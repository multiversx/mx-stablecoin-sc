mod contract_interactions;
mod contract_setup;
use contract_setup::*;

use elrond_wasm_debug::{rust_biguint, DebugApi};

#[test]
fn init_test() {
    let _ = StablecoinContractSetup::new(stablecoin_v3::contract_obj);
}

#[test]
fn simple_buy_stablecoin_test() {
    let rust_zero = rust_biguint!(0);
    let _ = DebugApi::dummy();
    let mut sa_setup = StablecoinContractSetup::new(stablecoin_v3::contract_obj);

    let first_user = sa_setup.b_mock.create_user_account(&rust_zero);
    sa_setup
        .b_mock
        .set_esdt_balance(&first_user, COLLATERAL_TOKEN_ID, &rust_biguint!(100));

    sa_setup.swap_stablecoin(&first_user, COLLATERAL_TOKEN_ID, 100u64, 9000u64);

    sa_setup.b_mock.check_esdt_balance(
        &first_user,
        STABLECOIN_TOKEN_ID,
        &rust_biguint!(9_950),
    );

    sa_setup.b_mock.check_esdt_balance(
        &sa_setup.owner_address,
        STABLECOIN_TOKEN_ID,
        &rust_biguint!(1_000_050),
    );
}
