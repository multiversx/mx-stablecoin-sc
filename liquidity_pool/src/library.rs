elrond_wasm::imports!();

const BASE_PRECISION: u32 = 1000000000;
const SECONDS_IN_YEAR: u32 = 31556926;

#[elrond_wasm_derive::module(LibraryModuleImpl)]
pub trait LibraryModule {
    fn init(&self) {}

    fn compute_borrow_rate(
        &self,
        r_base: BigUint,
        r_slope1: BigUint,
        r_slope2: BigUint,
        u_optimal: BigUint,
        u_current: BigUint,
    ) -> BigUint {
        let base_precision = BigUint::from(BASE_PRECISION);

        if u_current < u_optimal {
            let utilisation_ratio = (u_current * r_slope1) / u_optimal;

            r_base + utilisation_ratio
        } else {
            let denominator = base_precision - u_optimal.clone();
            let numerator = (u_current - u_optimal) * r_slope2;

            (r_base + r_slope1) + numerator / denominator
        }
    }

    fn compute_deposit_rate(
        &self,
        u_current: BigUint,
        borrow_rate: BigUint,
        reserve_factor: BigUint,
    ) -> BigUint {
        let base_precision = BigUint::from(BASE_PRECISION);
        let loan_ratio = u_current.clone() * borrow_rate;
        let deposit_rate = u_current * loan_ratio * (base_precision.clone() - reserve_factor);

        deposit_rate / (&base_precision * &base_precision * base_precision)
    }

    fn compute_capital_utilisation(
        &self,
        borrowed_amount: BigUint,
        total_pool_reserves: BigUint,
    ) -> BigUint {
        (borrowed_amount * BigUint::from(BASE_PRECISION)) / total_pool_reserves
    }

    fn compute_debt(&self, amount: BigUint, time_diff: BigUint, borrow_rate: BigUint) -> BigUint {
        let base_precision = BigUint::from(BASE_PRECISION);
        let secs_year = BigUint::from(SECONDS_IN_YEAR);
        let time_unit_percentage = (time_diff * base_precision.clone()) / secs_year;

        let debt_percetange = (time_unit_percentage * borrow_rate) / base_precision.clone();

        if debt_percetange <= base_precision {
            let amount_diff =
                ((base_precision.clone() - debt_percetange) * amount.clone()) / base_precision;

            amount - amount_diff
        } else {
            (debt_percetange * amount) / base_precision
        }
    }
}
