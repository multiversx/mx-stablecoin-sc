elrond_wasm::imports!();

use crate::{hedging_agents::HedgingPosition, math::ONE};

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
        self.require_not_liquidated(nft_nonce)?;

        let mut hedging_position = self.hedging_position(nft_nonce).get();
        let pool = self.get_pool(&hedging_position.collateral_id);

        let limit_hedge_amount = self.calculate_limit_hedge_amount(&pool.collateral_amount);
        require!(
            pool.total_covered_value_in_stablecoin > limit_hedge_amount,
            "May only force close after limit hedge amount is passed"
        );

        self.close_position(nft_nonce, &hedging_position)?;
        hedging_position.withdraw_amount_after_force_close =
            Some(self.get_withdraw_amount_and_update_fees(&hedging_position, None)?);

        self.hedging_position(nft_nonce).set(&hedging_position);

        Ok(())
    }

    #[endpoint(liquidateHedgingPosition)]
    fn liquidate_hedging_position(&self, nft_nonce: u64) -> SCResult<()> {
        self.require_not_liquidated(nft_nonce)?;

        let hedging_position = self.hedging_position(nft_nonce).get();
        self.require_not_closed(&hedging_position)?;

        let margin_ratio = self.calculate_margin_ratio(&hedging_position)?;
        let hedging_maintenance_ratio = self.hedging_maintenance_ratio().get();
        require!(
            margin_ratio <= hedging_maintenance_ratio,
            "Can only liquidate if margin ratio is below expected amount"
        );

        self.update_pool(&hedging_position.collateral_id, |pool| {
            pool.collateral_amount += &hedging_position.deposit_amount
        });
        self.close_position(nft_nonce, &hedging_position)?;
        self.hedging_position(nft_nonce).clear();

        Ok(())
    }

    #[endpoint(rebalancePool)]
    fn rebalance_pool(&self, _collateral_id: TokenIdentifier) -> SCResult<()> {
        Ok(())
    }

    fn calculate_margin_ratio(
        &self,
        hedging_position: &HedgingPosition<Self::Api>,
    ) -> SCResult<BigUint> {
        let collateral_value_in_dollars =
            self.get_collateral_value_in_dollars(&hedging_position.collateral_id)?;

        // margin = x / y + (1 - initial_oracle / current_oracle)
        // where x is deposit_amount and y is amount_to_cover
        let amount_ratio = self.calculate_ratio(
            &hedging_position.deposit_amount,
            &hedging_position.covered_amount,
        );
        let price_ratio = self.calculate_ratio(
            &hedging_position.oracle_value_at_deposit_time,
            &collateral_value_in_dollars,
        );

        let one = BigUint::from(ONE);
        let result = if price_ratio <= one {
            let diff = one - price_ratio;
            amount_ratio + diff
        } else {
            let diff = price_ratio - one;
            amount_ratio - diff
        };

        Ok(result)
    }
}
