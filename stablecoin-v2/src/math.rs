elrond_wasm::imports!();

pub const PERCENTAGE_PRECISION: u64 = 1_000_000_000; // 100%
pub const ONE: u64 = PERCENTAGE_PRECISION / 100;

#[elrond_wasm::module]
pub trait MathModule {
    fn multiply(&self, first: &BigUint, second: &BigUint) -> BigUint {
        first * second / ONE
    }

    fn calculate_ratio(&self, first: &BigUint, second: &BigUint) -> BigUint {
        &(first * ONE) / second
    }

    fn calculate_percentage_of(&self, percentage: &BigUint, number: &BigUint) -> BigUint {
        number * percentage / PERCENTAGE_PRECISION
    }
}
