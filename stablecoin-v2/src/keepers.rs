elrond_wasm::imports!();

#[elrond_wasm::module]
pub trait KeepersModule:
    crate::fees::FeesModule
    + crate::hedging_agents::HedgingAgentsModule
    + crate::hedging_token::HedgingTokenModule
    + crate::math::MathModule
    + crate::pools::PoolsModule
    + price_aggregator_proxy::PriceAggregatorModule
{
    #[endpoint(forceCloseHedgingPosition)]
    fn force_close_hedging_position(&self, nft_nonce: u64) -> SCResult<()> {
        let mut hedging_position = self.hedging_position(nft_nonce).get();
        let pool = self.get_pool(&hedging_position.collateral_id);

        let limit_hedge_amount = self.calculate_limit_hedge_amount(&pool.collateral_amount);
        require!(
            pool.total_covered_value_in_stablecoin > limit_hedge_amount,
            "May only force close after limit hedge amount is passed"
        );

        hedging_position.withdraw_amount_after_force_close =
            Some(self.close_position(nft_nonce, &hedging_position, None)?);

        self.hedging_position(nft_nonce).set(&hedging_position);

        Ok(())
    }

    #[endpoint(rebalancePool)]
    fn rebalance_pool(&self, _collateral_id: TokenIdentifier) -> SCResult<()> {
        Ok(())
    }
}
