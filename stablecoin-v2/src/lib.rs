#![no_std]

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

mod fees;
mod hedging_agents;
mod hedging_token;
mod keepers;
mod math;
mod pools;
mod stablecoin_token;

#[elrond_wasm::contract]
pub trait StablecoinV2:
    fees::FeesModule
    + hedging_agents::HedgingAgentsModule
    + hedging_token::HedgingTokenModule
    + keepers::KeepersModule
    + math::MathModule
    + pools::PoolsModule
    + price_aggregator_proxy::PriceAggregatorModule
    + stablecoin_token::StablecoinTokenModule
{
    #[init]
    fn init(
        &self,
        price_aggregator_address: ManagedAddress,
        min_hedging_period_seconds: u64,
        target_hedging_ratio: BigUint,
        hedging_ratio_limit: BigUint,
    ) -> SCResult<()> {
        require!(
            self.blockchain()
                .is_smart_contract(&price_aggregator_address),
            "Price aggregator address is not a smart contract"
        );

        self.price_aggregator_address()
            .set(&price_aggregator_address);

        self.min_hedging_period_seconds()
            .set(&min_hedging_period_seconds);
        self.target_hedging_ratio().set(&target_hedging_ratio);
        self.hedging_ratio_limit().set(&hedging_ratio_limit);

        Ok(())
    }

    // endpoints - owner-only

    #[only_owner]
    #[endpoint(addCollateralToWhitelist)]
    fn add_collateral_to_whitelist(
        &self,
        collateral_id: TokenIdentifier,
        collateral_ticker: ManagedBuffer,
        max_leverage: BigUint,
        min_fees_percentage: BigUint,
        max_fees_percentage: BigUint,
    ) -> SCResult<()> {
        require!(
            min_fees_percentage <= max_fees_percentage
                && max_fees_percentage < math::PERCENTAGE_PRECISION,
            "Invalid fees percentages"
        );

        self.collateral_ticker(&collateral_id)
            .set(&collateral_ticker);
        self.max_leverage(&collateral_id).set(&max_leverage);
        self.min_max_fees_percentage(&collateral_id)
            .set(&(min_fees_percentage, max_fees_percentage));
        let _ = self.collateral_whitelist().insert(collateral_id);

        Ok(())
    }

    #[only_owner]
    #[endpoint(removeCollateralFromWhitelist)]
    fn remove_collateral_from_whitelist(&self, collateral_id: TokenIdentifier) {
        self.collateral_ticker(&collateral_id).clear();
        self.max_leverage(&collateral_id).clear();
        self.min_max_fees_percentage(&collateral_id).clear();
        let _ = self.collateral_whitelist().remove(&collateral_id);
    }

    // endpoints

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

        let stablecoin_amount = &collateral_value_in_dollars * &collateral_amount;
        require!(stablecoin_amount >= min_amount_out, "Below min amount");

        self.update_pool(&payment_token, |pool| {
            pool.collateral_amount += &collateral_amount;
            pool.stablecoin_amount += &stablecoin_amount;
        });
        self.accumulated_tx_fees(&payment_token)
            .update(|accumulated_fees| *accumulated_fees += fees_amount_in_collateral);

        let caller = self.blockchain().get_caller();
        self.mint_and_send_stablecoin(&caller, &stablecoin_amount);

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
        let total_value_in_collateral = &payment_amount / &collateral_value_in_dollars;
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

            pool.collateral_amount -= &collateral_amount;
            pool.stablecoin_amount -= &payment_amount;

            Ok(())
        })?;
        self.accumulated_tx_fees(&collateral_id)
            .update(|accumulated_fees| *accumulated_fees += fees_amount_in_collateral);

        self.burn_stablecoin(&payment_amount);

        let caller = self.blockchain().get_caller();
        self.send()
            .direct(&caller, &collateral_id, 0, &collateral_amount, &[]);

        Ok(())
    }
}
