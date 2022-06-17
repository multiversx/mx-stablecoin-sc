mod contract_interactions;
mod contract_setup;
use contract_setup::*;

use elrond_wasm_debug::{DebugApi};

#[test]
fn init_test() {
    let _ = StablecoinContractSetup::new(stablecoin_v3::contract_obj);
}

#[test]
fn stablecoin_simple_buy_test() {
    let _ = DebugApi::dummy();
    let mut sc_setup = StablecoinContractSetup::new(stablecoin_v3::contract_obj);

    let first_user = sc_setup.setup_new_user(100u64, 10_000u64);
    sc_setup.swap_stablecoin(&first_user, COLLATERAL_TOKEN_ID, 100u64, 9_000u64);
    sc_setup.check_user_balance(&sc_setup.owner_address, STABLECOIN_TOKEN_ID, 1_000_100);
    sc_setup.check_user_balance(&first_user, STABLECOIN_TOKEN_ID, 19_900);
    sc_setup.check_user_balance(&first_user, COLLATERAL_TOKEN_ID, 0);
}

#[test]
fn stablecoin_simple_sell_test() {
    let _ = DebugApi::dummy();
    let mut sc_setup = StablecoinContractSetup::new(stablecoin_v3::contract_obj);

    let first_user = sc_setup.setup_new_user(100u64, 10_000u64);
    sc_setup.swap_stablecoin(&first_user, STABLECOIN_TOKEN_ID, 10_000u64, 99u64);
    sc_setup.check_user_balance(&first_user, COLLATERAL_TOKEN_ID, 199);
    sc_setup.check_user_balance(&sc_setup.owner_address, COLLATERAL_TOKEN_ID, 1);
    sc_setup.check_user_balance(&sc_setup.owner_address, STABLECOIN_TOKEN_ID, 1_000_000);
}

#[test]
fn stablecoin_multiple_buy_test() {
    let _ = DebugApi::dummy();
    let mut sc_setup = StablecoinContractSetup::new(stablecoin_v3::contract_obj);

    sc_setup.change_fee_spread_percentage(2_000); // 2%

    let first_user = sc_setup.setup_new_user(1_000u64, 100_000u64);
    let second_user = sc_setup.setup_new_user(1_000u64, 100_000u64);
    let third_user = sc_setup.setup_new_user(1_000u64, 100_000u64);

    sc_setup.swap_stablecoin(&first_user, COLLATERAL_TOKEN_ID, 500u64, 45_000u64);
    sc_setup.check_user_balance(&first_user, COLLATERAL_TOKEN_ID, 500);
    sc_setup.check_user_balance(&first_user, STABLECOIN_TOKEN_ID, 149_000);

    sc_setup.swap_stablecoin(&first_user, COLLATERAL_TOKEN_ID, 200u64, 18_000u64);
    sc_setup.check_user_balance(&first_user, COLLATERAL_TOKEN_ID, 300);
    sc_setup.check_user_balance(&first_user, STABLECOIN_TOKEN_ID, 168_600);

    sc_setup.b_mock.set_block_nonce(2u64);

    sc_setup.swap_stablecoin(&second_user, COLLATERAL_TOKEN_ID, 500u64, 18_000u64);
    sc_setup.check_user_balance(&second_user, COLLATERAL_TOKEN_ID, 500);
    sc_setup.check_user_balance(&second_user, STABLECOIN_TOKEN_ID, 149_000);

    sc_setup.b_mock.set_block_nonce(10u64);

    sc_setup.swap_stablecoin(&third_user, STABLECOIN_TOKEN_ID, 30_000u64, 100u64);
    sc_setup.check_user_balance(&third_user, COLLATERAL_TOKEN_ID, 1_294);
    sc_setup.check_user_balance(&third_user, STABLECOIN_TOKEN_ID, 70_000);
}
