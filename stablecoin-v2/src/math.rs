elrond_wasm::imports!();

pub const PERCENTAGE_PRECISION: u64 = 1_000_000_000; // 100%
pub const ONE: u64 = PERCENTAGE_PRECISION / 100;

#[elrond_wasm::module]
pub trait MathModule {
    #[inline(always)]
    fn multiply(&self, first: &BigUint, second: &BigUint, precision_to_remove: &BigUint) -> BigUint {
        first * second / precision_to_remove
    }

    #[inline(always)]
    fn calculate_ratio(&self, first: &BigUint, second: &BigUint) -> BigUint {
        &(first * ONE) / second
    }

    #[inline(always)]
    fn calculate_percentage_of(&self, percentage: &BigUint, number: &BigUint) -> BigUint {
        number * percentage / PERCENTAGE_PRECISION
    }

    #[inline(always)]
    fn create_precision_biguint(&self, nr_decimals: u32) -> BigUint {
        let base = BigUint::from(10u64);
        base.pow(nr_decimals)
    }
}
