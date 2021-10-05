elrond_wasm::imports!();

use crate::math::ONE;

const MIN_FEES_PERCENTAGE: u64 = ONE * 2 / 10; // 0.2%
const MAX_FEES_PERCENTAGE: u64 = ONE * 4 / 10; // 0.4%

#[elrond_wasm::module]
pub trait FeesModule: crate::math::MathModule {
    fn get_coverage_ratio(&self, collateral_id: &TokenIdentifier) -> BigUint {
        let reserves = self.reserves(collateral_id).get();
        let total_covered = self.total_covered(collateral_id).get();

        self.calculate_ratio(&total_covered, &reserves)
    }

    // mint fees decrease as coverage ratio increases
    fn get_mint_transaction_fees_percentage(&self, collateral_id: &TokenIdentifier) -> BigUint {
        let coverage_ratio = self.get_coverage_ratio(collateral_id);

        if coverage_ratio == 0 {
            return BigUint::from(MAX_FEES_PERCENTAGE);
        }
        if coverage_ratio >= ONE {
            return BigUint::from(MIN_FEES_PERCENTAGE);
        }

        let percentage_diff = BigUint::from(MAX_FEES_PERCENTAGE - MIN_FEES_PERCENTAGE);

        // max - (max - min) * coverage_ratio
        BigUint::from(MAX_FEES_PERCENTAGE) - self.multiply(&coverage_ratio, &percentage_diff)
    }

    // burn fees decrease as coverage ratio decreases
    fn get_burn_transaction_fees_percentage(&self, collateral_id: &TokenIdentifier) -> BigUint {
        let coverage_ratio = self.get_coverage_ratio(collateral_id);

        if coverage_ratio == 0 {
            return BigUint::from(MIN_FEES_PERCENTAGE);
        }
        if coverage_ratio >= ONE {
            return BigUint::from(MAX_FEES_PERCENTAGE);
        }

        let percentage_diff = BigUint::from(MAX_FEES_PERCENTAGE - MIN_FEES_PERCENTAGE);

        // min + (max - min) * coverage_ratio
        BigUint::from(MIN_FEES_PERCENTAGE) + self.multiply(&coverage_ratio, &percentage_diff)
    }

    // storage

    #[storage_mapper("reserves")]
    fn reserves(&self, collateral_id: &TokenIdentifier) -> SingleValueMapper<BigUint>;

    #[storage_mapper("totalCovered")]
    fn total_covered(&self, collateral_id: &TokenIdentifier) -> SingleValueMapper<BigUint>;

    #[storage_mapper("accumulatedTxFees")]
    fn accumulated_tx_fees(&self, collateral_id: &TokenIdentifier) -> SingleValueMapper<BigUint>;
}
