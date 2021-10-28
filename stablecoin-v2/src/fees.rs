elrond_wasm::imports!();

use crate::math::ONE;

// TODO: calculate fees only when rebalancing pool by keepers
// store the value in storage instead of calculating everytime

#[elrond_wasm::module]
pub trait FeesModule:
    crate::math::MathModule + crate::pools::PoolsModule + price_aggregator_proxy::PriceAggregatorModule
{
    // mint fees decrease as coverage ratio increases
    #[view(getMintTransactionFeesPercentage)]
    fn get_mint_transaction_fees_percentage(&self, collateral_id: &TokenIdentifier) -> BigUint {
        let target_hedging_ratio = self.target_hedging_ratio().get();
        let current_hedging_ratio = self.calculate_current_hedging_ratio(collateral_id);
        let (min_fees_percentage, max_fees_percentage) =
            self.min_max_fees_percentage(collateral_id).get();
        let one = BigUint::from(ONE);

        if current_hedging_ratio == 0 {
            return max_fees_percentage;
        }
        if current_hedging_ratio >= target_hedging_ratio {
            return min_fees_percentage;
        }

        let percentage_diff = &max_fees_percentage - &min_fees_percentage;

        // max - (max - min) * hedging_ratio
        max_fees_percentage - self.multiply(&current_hedging_ratio, &percentage_diff, &one)
    }

    // burn fees decrease as coverage ratio decreases
    #[view(getBurnTransactionFeesPercentage)]
    fn get_burn_transaction_fees_percentage(&self, collateral_id: &TokenIdentifier) -> BigUint {
        let target_hedging_ratio = self.target_hedging_ratio().get();
        let current_hedging_ratio = self.calculate_current_hedging_ratio(collateral_id);
        let (min_fees_percentage, max_fees_percentage) =
            self.min_max_fees_percentage(collateral_id).get();
        let one = BigUint::from(ONE);

        if current_hedging_ratio == 0 {
            return min_fees_percentage;
        }
        if current_hedging_ratio >= target_hedging_ratio {
            return max_fees_percentage;
        }

        let percentage_diff = &max_fees_percentage - &min_fees_percentage;

        // min + (max - min) * hedging_ratio
        min_fees_percentage + self.multiply(&current_hedging_ratio, &percentage_diff, &one)
    }

    // The more collateral is covered, the more expensive it is to open a position
    // This scales the same way as the burn transaction fees, so we use the same formula
    #[view(getHedgingPositionOpenTransactionFeesPercentage)]
    fn get_hedging_position_open_transaction_fees_percentage(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> BigUint {
        self.get_burn_transaction_fees_percentage(collateral_id)
    }

    // The more collateral is covered, the less expensive it is to exit
    #[view(getHedgingPositionCloseTransactionFeesPercentage)]
    fn get_hedging_position_close_transaction_fees_percentage(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> BigUint {
        self.get_mint_transaction_fees_percentage(collateral_id)
    }

    #[view(getCurrentHedgingRatio)]
    fn calculate_current_hedging_ratio(&self, collateral_id: &TokenIdentifier) -> BigUint {
        let pool = self.get_pool(collateral_id);
        let target_hedge_amount = self.calculate_target_hedge_amount(&pool.collateral_amount);
        self.calculate_ratio(
            &pool.total_covered_value_in_stablecoin,
            &target_hedge_amount,
        )
    }

    #[view(getTargetHedgeAmount)]
    fn calculate_target_hedge_amount(&self, collateral_amount: &BigUint) -> BigUint {
        let target_hedging_ratio = self.target_hedging_ratio().get();
        self.calculate_percentage_of(&target_hedging_ratio, collateral_amount)
    }

    #[view(getLimitHedgeAmount)]
    fn calculate_limit_hedge_amount(&self, collateral_amount: &BigUint) -> BigUint {
        let hedging_ratio_limit = self.hedging_ratio_limit().get();
        self.calculate_percentage_of(&hedging_ratio_limit, collateral_amount)
    }

    fn split_fees(&self) {
        // TODO
    }

    // storage

    #[storage_mapper("minMaxFeesPercentage")]
    fn min_max_fees_percentage(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SingleValueMapper<(BigUint, BigUint)>;

    #[storage_mapper("accumulatedTxFees")]
    fn accumulated_tx_fees(&self, collateral_id: &TokenIdentifier) -> SingleValueMapper<BigUint>;

    #[view(getTargetHedgingRatio)]
    #[storage_mapper("targetHedgingRatio")]
    fn target_hedging_ratio(&self) -> SingleValueMapper<BigUint>;

    #[view(getHedgingRatioLimit)]
    #[storage_mapper("hedgingRatioLimit")]
    fn hedging_ratio_limit(&self) -> SingleValueMapper<BigUint>;
}
