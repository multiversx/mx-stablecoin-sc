elrond_wasm::imports!();

use crate::math::ONE;

#[elrond_wasm::module]
pub trait FeesModule: crate::math::MathModule {
    #[view(getCoverageRatio)]
    fn get_coverage_ratio(&self, collateral_id: &TokenIdentifier) -> BigUint {
        let reserves = self.reserves(collateral_id).get();
        let total_covered = self.total_covered(collateral_id).get();

        self.calculate_ratio(&total_covered, &reserves)
    }

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

    // storage

    #[storage_mapper("reserves")]
    fn reserves(&self, collateral_id: &TokenIdentifier) -> SingleValueMapper<BigUint>;

    #[view(getTotalCovered)]
    #[storage_mapper("totalCovered")]
    fn total_covered(&self, collateral_id: &TokenIdentifier) -> SingleValueMapper<BigUint>;

    #[storage_mapper("minMaxFeesPercentage")]
    fn min_max_fees_percentage(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SingleValueMapper<(BigUint, BigUint)>;

    #[storage_mapper("accumulatedTxFees")]
    fn accumulated_tx_fees(&self, collateral_id: &TokenIdentifier) -> SingleValueMapper<BigUint>;
}
