use crate::math::ONE;

elrond_wasm::imports!();

#[elrond_wasm::module]
pub trait LiquidityProvidersModule:
    crate::fees::FeesModule
    + crate::liquidity_token::LiquidityTokenModule
    + crate::math::MathModule
    + crate::pools::PoolsModule
    + price_aggregator_proxy::PriceAggregatorModule
    + crate::token_common::TokenCommonModule
{
    #[payable("*")]
    #[endpoint(addLiquidity)]
    fn add_liquidity(
        &self,
        #[payment_token] payment_token: TokenIdentifier,
        #[payment_amount] payment_amount: BigUint,
    ) -> SCResult<()> {
        self.require_collateral_in_whitelist(&payment_token)?;

        self.update_pool(&payment_token, |pool| {
            pool.collateral_reserves += &payment_amount;
        });

        let collateral_precision = self.get_collateral_precision(&payment_token);
        let amount_in_liq_tokens =
            self.collateral_to_liq_tokens(&payment_token, &payment_amount, &collateral_precision);
        let sft_nonce = self.create_or_mint_liq_tokens(&payment_token, &amount_in_liq_tokens);

        self.collateral_amount_for_liq_token(sft_nonce)
            .update(|collateral_amount| *collateral_amount += &payment_amount);

        let caller = self.blockchain().get_caller();
        self.send_liq_tokens(&caller, sft_nonce, &amount_in_liq_tokens);

        Ok(())
    }

    #[payable("*")]
    #[endpoint(removeLiquidity)]
    fn remove_liquidity(
        &self,
        #[payment_token] payment_token: TokenIdentifier,
        #[payment_nonce] payment_nonce: u64,
        #[payment_amount] payment_amount: BigUint,
    ) -> SCResult<()> {
        let liq_token_id = self.liquidity_token_id().get();
        require!(
            payment_token == liq_token_id,
            "May only pay with liquidity SFTs"
        );

        let collateral_id = self.collateral_for_liq_sft_nonce(payment_nonce).get();
        let collateral_precision = self.get_collateral_precision(&collateral_id);
        let amount_in_collateral =
            self.liq_tokens_to_collateral(&collateral_id, &payment_amount, &collateral_precision);

        let slippage_percentage = self.calculate_slippage_percentage(&collateral_id);
        let slippage_amount_in_collateral =
            self.calculate_percentage_of(&slippage_percentage, &amount_in_collateral);

        let collateral_amount_after_slippage =
            &amount_in_collateral - &slippage_amount_in_collateral;

        self.update_pool(&collateral_id, |pool| {
            require!(
                collateral_amount_after_slippage <= pool.collateral_reserves,
                "Not enough reserves in pool"
            );

            pool.collateral_reserves -= &collateral_amount_after_slippage;

            Ok(())
        })?;

        self.burn_liq_tokens(payment_nonce, &payment_amount);
        // have to deduct amount without slippage here to mantain the liq token price
        self.collateral_amount_for_liq_token(payment_nonce)
            .update(|collateral_amount| *collateral_amount -= &amount_in_collateral);

        let caller = self.blockchain().get_caller();
        self.send().direct(
            &caller,
            &collateral_id,
            0,
            &collateral_amount_after_slippage,
            &[],
        );

        Ok(())
    }

    #[view(getLiquidityTokenValueInCollateral)]
    fn get_liquidity_token_value_in_collateral_view(
        &self,
        collateral_id: TokenIdentifier,
    ) -> BigUint {
        let collateral_precision = self.get_collateral_precision(&collateral_id);
        self.get_liq_token_value_in_collateral(&collateral_id, &collateral_precision)
    }

    #[view(getSlippagePercentage)]
    fn calculate_slippage_percentage(&self, collateral_id: &TokenIdentifier) -> BigUint {
        let hedging_ratio = self.get_current_hedging_ratio(collateral_id);
        let one = BigUint::from(ONE);

        // no slippage if all collateral is covered
        if hedging_ratio >= one {
            return BigUint::zero();
        }

        let (min_slippage_percentage, max_slippage_percentage) =
            self.min_max_slippage_percentage(collateral_id).get();
        let percentage_diff = &max_slippage_percentage - &min_slippage_percentage;

        // min + (max - min) * hedging_ratio
        min_slippage_percentage + self.multiply(&hedging_ratio, &percentage_diff, &one)
    }

    #[storage_mapper("minMaxSlippagePercentage")]
    fn min_max_slippage_percentage(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SingleValueMapper<(BigUint, BigUint)>;

    #[storage_mapper("liquidityProviderFeeRewardPercentage")]
    fn liq_provider_fee_reward_percentage(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SingleValueMapper<BigUint>;

    #[storage_mapper("liqProviderLendRewardPercentage")]
    fn liq_provider_lend_reward_percentage(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SingleValueMapper<BigUint>;
}
