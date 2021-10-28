elrond_wasm::imports!();

pub const PERCENTAGE_PRECISION: u64 = 1_000_000_000; // 100%
pub const ONE: u64 = PERCENTAGE_PRECISION / 100;

// this is the most common case, and it's more efficient to have a constant instead of manually calculating 10^18 everytime
const DEFAULT_TOKEN_NUM_DECIMALS: u32 = 18;
const DEFAULT_TOKEN_DECIMALS_VALUE: u64 = 1_000_000_000_000_000_000;

#[elrond_wasm::module]
pub trait MathModule {
    #[inline(always)]
    fn multiply(&self, first: &BigUint, second: &BigUint, precision_to_remove: &BigUint) -> BigUint {
        first * second / precision_to_remove
    }

    #[inline(always)]
    fn divide(&self, first: &BigUint, second: &BigUint, result_precision: &BigUint) -> BigUint {
        &(first * result_precision) / second
    }

    #[inline(always)]
    fn calculate_ratio(&self, first: &BigUint, second: &BigUint) -> BigUint {
        &(first * ONE) / second
    }

    #[inline(always)]
    fn calculate_percentage_of(&self, percentage: &BigUint, number: &BigUint) -> BigUint {
        number * percentage / PERCENTAGE_PRECISION
    }

    fn create_precision_biguint(&self, nr_decimals: u32) -> BigUint {
        if nr_decimals == DEFAULT_TOKEN_NUM_DECIMALS {
            return BigUint::from(DEFAULT_TOKEN_DECIMALS_VALUE);
        }

        let base = BigUint::from(10u64);
        base.pow(nr_decimals)
    }
}
