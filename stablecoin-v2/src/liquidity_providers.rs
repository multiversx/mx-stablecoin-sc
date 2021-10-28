elrond_wasm::imports!();

// TODO: Pay some part of the hedging position open/close fees to liquidity providers as rewards

#[elrond_wasm::module]
pub trait LiquidityProvidersModule:
    crate::liquidity_token::LiquidityTokenModule
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

        let caller = self.blockchain().get_caller();
        let amount_in_liq_tokens = self.collateral_to_liq_tokens(&payment_token, &payment_amount);
        self.create_and_send_liq_tokens(&caller, &payment_token, &amount_in_liq_tokens);

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
        let amount_in_collateral = self.liq_tokens_to_collateral(&collateral_id, &payment_amount);

        self.update_pool(&collateral_id, |pool| {
            require!(
                amount_in_collateral <= pool.collateral_reserves,
                "Not enough reserves in pool"
            );

            pool.collateral_reserves -= &amount_in_collateral;

            Ok(())
        })?;

        let caller = self.blockchain().get_caller();
        self.send()
            .direct(&caller, &collateral_id, 0, &amount_in_collateral, &[]);

        Ok(())
    }
}
