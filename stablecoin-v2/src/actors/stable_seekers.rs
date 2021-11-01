elrond_wasm::imports!();

#[elrond_wasm::module]
pub trait StableSeekers:
    crate::fees::FeesModule
    + crate::math::MathModule
    + crate::pools::PoolsModule
    + crate::pool_events::PoolEventsModule
    + price_aggregator_proxy::PriceAggregatorModule
    + crate::stablecoin_token::StablecoinTokenModule
    + crate::token_common::TokenCommonModule
{
    #[payable("*")]
    #[endpoint(sellCollateral)]
    fn sell_collateral(
        &self,
        #[payment_token] payment_token: TokenIdentifier,
        #[payment_amount] payment_amount: BigUint,
        min_amount_out: BigUint,
    ) -> SCResult<()> {
        self.require_collateral_in_whitelist(&payment_token)?;

        let collateral_value_in_dollars = self.get_collateral_value_in_dollars(&payment_token)?;
        let transaction_fees_percentage = self.get_mint_transaction_fees_percentage(&payment_token);
        let fees_amount_in_collateral =
            self.calculate_percentage_of(&transaction_fees_percentage, &payment_amount);
        let collateral_amount = &payment_amount - &fees_amount_in_collateral;

        let collateral_precision = self.get_collateral_precision(&payment_token);
        let stablecoin_amount = self.multiply(
            &collateral_value_in_dollars,
            &collateral_amount,
            &collateral_precision,
        );

        require!(stablecoin_amount >= min_amount_out, "Below min amount");

        self.update_pool(&payment_token, |pool| {
            pool.collateral_amount += &collateral_amount;
            pool.stablecoin_amount += &stablecoin_amount;
        });
        self.accumulated_tx_fees(&payment_token)
            .update(|accumulated_fees| *accumulated_fees += fees_amount_in_collateral);

        let caller = self.blockchain().get_caller();
        self.mint_and_send_stablecoin(&caller, &stablecoin_amount);

        let stablecoin_token_id = self.stablecoin_token_id().get();
        self.swap_event(
            &caller,
            &payment_token,
            &stablecoin_token_id,
            &payment_amount,
            &stablecoin_amount,
        );

        Ok(())
    }

    #[payable("*")]
    #[endpoint(buyCollateral)]
    fn buy_collateral(
        &self,
        #[payment_token] payment_token: TokenIdentifier,
        #[payment_amount] payment_amount: BigUint,
        collateral_id: TokenIdentifier,
        min_amount_out: BigUint,
    ) -> SCResult<()> {
        let stablecoin_token_id = self.stablecoin_token_id().get();
        require!(
            payment_token == stablecoin_token_id,
            "May only pay with stablecoins"
        );
        self.require_collateral_in_whitelist(&collateral_id)?;

        let collateral_value_in_dollars = self.get_collateral_value_in_dollars(&collateral_id)?;
        let collateral_precision = self.get_collateral_precision(&collateral_id);
        let total_value_in_collateral = self.divide(
            &payment_amount,
            &collateral_value_in_dollars,
            &collateral_precision,
        );

        let transaction_fees_percentage = self.get_burn_transaction_fees_percentage(&collateral_id);
        let fees_amount_in_collateral =
            self.calculate_percentage_of(&transaction_fees_percentage, &total_value_in_collateral);

        let collateral_amount = &total_value_in_collateral - &fees_amount_in_collateral;
        require!(collateral_amount >= min_amount_out, "Below min amount");

        self.update_pool(&collateral_id, |pool| {
            require!(
                pool.collateral_amount >= collateral_amount,
                "Insufficient funds for swap"
            );
            require!(
                pool.stablecoin_amount >= payment_amount,
                "Too many stablecoins paid"
            );

            pool.collateral_amount -= &total_value_in_collateral;
            pool.stablecoin_amount -= &payment_amount;

            Ok(())
        })?;
        self.accumulated_tx_fees(&collateral_id)
            .update(|accumulated_fees| *accumulated_fees += fees_amount_in_collateral);

        self.burn_stablecoin(&payment_amount);

        let caller = self.blockchain().get_caller();
        self.send()
            .direct(&caller, &collateral_id, 0, &collateral_amount, &[]);

        self.swap_event(
            &caller,
            &stablecoin_token_id,
            &collateral_id,
            &payment_amount,
            &collateral_amount,
        );

        Ok(())
    }
}
