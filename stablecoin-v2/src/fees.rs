elrond_wasm::imports!();

use crate::math::ONE;

#[elrond_wasm::module]
pub trait FeesModule:
    crate::math::MathModule + crate::pools::PoolsModule + price_aggregator_proxy::PriceAggregatorModule
{
    // mint fees decrease as coverage ratio increases
    #[view(getMintTransactionFeesPercentage)]
    fn get_mint_transaction_fees_percentage(&self, collateral_id: &TokenIdentifier) -> BigUint {
        let coverage_ratio = self.get_coverage_ratio(collateral_id);
        let (min_fees_percentage, max_fees_percentage) =
            self.min_max_fees_percentage(collateral_id).get();

        if coverage_ratio == 0 {
            return max_fees_percentage;
        }
        if coverage_ratio >= ONE {
            return min_fees_percentage;
        }

        let percentage_diff = &max_fees_percentage - &min_fees_percentage;

        // max - (max - min) * coverage_ratio
        max_fees_percentage - self.multiply(&coverage_ratio, &percentage_diff)
    }

    // burn fees decrease as coverage ratio decreases
    #[view(getBurnTransactionFeesPercentage)]
    fn get_burn_transaction_fees_percentage(&self, collateral_id: &TokenIdentifier) -> BigUint {
        let coverage_ratio = self.get_coverage_ratio(collateral_id);
        let (min_fees_percentage, max_fees_percentage) =
            self.min_max_fees_percentage(collateral_id).get();

        if coverage_ratio == 0 {
            return min_fees_percentage;
        }
        if coverage_ratio >= ONE {
            return max_fees_percentage;
        }

        let percentage_diff = &max_fees_percentage - &min_fees_percentage;

        // min + (max - min) * coverage_ratio
        min_fees_percentage + self.multiply(&coverage_ratio, &percentage_diff)
    }

    // The more collateral is covered, the more expensive it is to open a position
    // This scales the same way as the burn transaction fees, so we use the same formula
    #[view(getHedgingPositionOpenTransactionFeesPercentage)]
    fn get_hedging_position_open_transaction_fees_percentage(&self, collateral_id: &TokenIdentifier) -> BigUint {
        self.get_burn_transaction_fees_percentage(collateral_id)
    }

    // The more collateral is covered, the less expensive it is to exit
    #[view(getHedgingPositionCloseTransactionFeesPercentage)]
    fn get_hedging_position_close_transaction_fees_percentage(&self, collateral_id: &TokenIdentifier) -> BigUint {
        self.get_mint_transaction_fees_percentage(collateral_id)
    }

    // storage

    #[storage_mapper("minMaxFeesPercentage")]
    fn min_max_fees_percentage(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SingleValueMapper<(BigUint, BigUint)>;

    // TODO: Don't accumulate here, but rather split the fees when the transaction is processed
    // These fees will go to the liquidity providers
    #[storage_mapper("accumulatedTxFees")]
    fn accumulated_tx_fees(&self, collateral_id: &TokenIdentifier) -> SingleValueMapper<BigUint>;
}
