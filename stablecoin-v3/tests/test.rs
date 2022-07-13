mod contract_interactions;
mod contract_setup;
use std::ops::Mul;

use contract_setup::*;

use elrond_wasm_debug::{DebugApi, rust_biguint};

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
    sc_setup.check_user_balance(&sc_setup.owner_address, STABLECOIN_TOKEN_ID, 100_000_000);
    sc_setup.check_user_balance(&first_user, STABLECOIN_TOKEN_ID, 19_900);
    sc_setup.check_user_balance(&first_user, COLLATERAL_TOKEN_ID, 0);
    sc_setup.check_user_balance(&sc_setup.sc_wrapper.address_ref(), STABLECOIN_TOKEN_ID, 100);
}

#[test]
fn stablecoin_simple_sell_test() {
    let _ = DebugApi::dummy();
    let mut sc_setup = StablecoinContractSetup::new(stablecoin_v3::contract_obj);

    let first_user = sc_setup.setup_new_user(100u64, 10_000u64);
    sc_setup.swap_stablecoin(&first_user, STABLECOIN_TOKEN_ID, 10_000u64, 99u64);
    sc_setup.check_user_balance(&first_user, COLLATERAL_TOKEN_ID, 199);
    sc_setup.check_user_balance(&sc_setup.owner_address, COLLATERAL_TOKEN_ID, 0);
    sc_setup.check_user_balance(&sc_setup.owner_address, STABLECOIN_TOKEN_ID, 100_000_000);
    // sc_setup.check_user_balance(&sc_setup.sc_wrapper.address_ref(), STABLECOIN_TOKEN_ID, 100);
}

#[test]
fn stablecoin_multiple_buy_test() {
    let _ = DebugApi::dummy();
    let mut sc_setup = StablecoinContractSetup::new(stablecoin_v3::contract_obj);

    sc_setup.change_fee_spread_percentage(100); // 0.1%

    let first_user = sc_setup.setup_new_user(1_000u64, 100_000u64);
    let second_user = sc_setup.setup_new_user(1_000u64, 100_000u64);
    let third_user = sc_setup.setup_new_user(1_000u64, 100_000u64);

    //Default spread fee
    sc_setup.swap_stablecoin(&first_user, COLLATERAL_TOKEN_ID, 200u64, 18_000u64);
    sc_setup.check_user_balance(&first_user, COLLATERAL_TOKEN_ID, 800);
    sc_setup.check_user_balance(&first_user, STABLECOIN_TOKEN_ID, 119_980);
    sc_setup.swap_stablecoin(&first_user, COLLATERAL_TOKEN_ID, 200u64, 18_000u64);
    sc_setup.check_user_balance(&first_user, COLLATERAL_TOKEN_ID, 600);
    sc_setup.check_user_balance(&first_user, STABLECOIN_TOKEN_ID, 139_960);
    sc_setup.swap_stablecoin(&first_user, COLLATERAL_TOKEN_ID, 200u64, 18_000u64);
    sc_setup.check_user_balance(&first_user, COLLATERAL_TOKEN_ID, 400);
    sc_setup.check_user_balance(&first_user, STABLECOIN_TOKEN_ID, 159_940);
    sc_setup.swap_stablecoin(&first_user, COLLATERAL_TOKEN_ID, 200u64, 17_000u64);

    //Bigger than default spread fee
    sc_setup.check_user_balance(&first_user, COLLATERAL_TOKEN_ID, 200);
    sc_setup.check_user_balance_denominated(&first_user, STABLECOIN_TOKEN_ID, exp17(1_799_122));

    sc_setup.b_mock.set_block_nonce(2u64);

    sc_setup.swap_stablecoin(&second_user, COLLATERAL_TOKEN_ID, 500u64, 18_000u64);
    sc_setup.check_user_balance(&second_user, COLLATERAL_TOKEN_ID, 500);
    sc_setup.check_user_balance(&second_user, STABLECOIN_TOKEN_ID, 149_897);

    sc_setup.b_mock.set_block_nonce(10u64);

    sc_setup.swap_stablecoin(&third_user, STABLECOIN_TOKEN_ID, 30_000u64, 100u64);
    sc_setup.check_user_balance_denominated(&third_user, COLLATERAL_TOKEN_ID, exp15(1_299_295));
    sc_setup.check_user_balance(&third_user, STABLECOIN_TOKEN_ID, 70_000);
}

#[test]
fn stablecoin_simple_collateral_provision_test() {
    let _ = DebugApi::dummy();
    let mut sc_setup = StablecoinContractSetup::new(stablecoin_v3::contract_obj);

    let first_user = sc_setup.setup_new_user_with_overcollateral(100u64, 10_000u64, 1_000u64);
    sc_setup.provide_collateral(&first_user, OVERCOLLATERAL_TOKEN_ID, 850u64);
    sc_setup.check_user_balance(&first_user, OVERCOLLATERAL_TOKEN_ID, 150);
    sc_setup.check_user_nft_balance(&first_user, CP_TOKEN_ID, 1, 25500);
    sc_setup.check_user_balance(&sc_setup.sc_wrapper.address_ref(), OVERCOLLATERAL_TOKEN_ID, 850);
    sc_setup.check_user_balance(&sc_setup.owner_address, STABLECOIN_TOKEN_ID, 100_000_000);
}

#[test]
fn stablecoin_collateral_provision_with_claim_rewards_test() {
    let _ = DebugApi::dummy();
    let mut sc_setup = StablecoinContractSetup::new(stablecoin_v3::contract_obj);

    let first_user = sc_setup.setup_new_user_with_overcollateral(100u64, 10_000u64, 1_000u64);
    let second_user = sc_setup.setup_new_user(1_000u64, 100_000u64);

    sc_setup.provide_collateral(&first_user, OVERCOLLATERAL_TOKEN_ID, 850u64);
    sc_setup.check_user_balance(&first_user, OVERCOLLATERAL_TOKEN_ID, 150);
    sc_setup.check_user_nft_balance(&first_user, CP_TOKEN_ID, 1, 25500);
    sc_setup.check_user_balance(&sc_setup.sc_wrapper.address_ref(), OVERCOLLATERAL_TOKEN_ID, 850);
    sc_setup.check_user_balance(&sc_setup.owner_address, STABLECOIN_TOKEN_ID, 100_000_000);

    sc_setup.swap_stablecoin(&first_user, COLLATERAL_TOKEN_ID, 100u64, 9_000u64);
    sc_setup.check_user_balance(&sc_setup.owner_address, STABLECOIN_TOKEN_ID, 100_000_000);
    sc_setup.check_user_balance(&first_user, STABLECOIN_TOKEN_ID, 19_900);
    sc_setup.check_user_balance(&first_user, COLLATERAL_TOKEN_ID, 0);
    sc_setup.check_user_balance(&sc_setup.sc_wrapper.address_ref(), STABLECOIN_TOKEN_ID, 100);

    sc_setup.swap_stablecoin(&second_user, COLLATERAL_TOKEN_ID, 500u64, 18_000u64);
    sc_setup.check_user_balance(&second_user, COLLATERAL_TOKEN_ID, 500);
    sc_setup.check_user_balance(&second_user, STABLECOIN_TOKEN_ID, 149_500);

    sc_setup.claim_fee_rewards(&first_user, CP_TOKEN_ID, 1, 25500);
    sc_setup.check_user_nft_balance(&first_user, CP_TOKEN_ID, 2, 25500);
    sc_setup.check_user_balance_denominated(&first_user, STABLECOIN_TOKEN_ID, exp9(20_499_999_999_982)); // 19_900 + 599.999999982

    // Returns 0 as user already claimed with this position
    sc_setup.claim_fee_rewards(&first_user, CP_TOKEN_ID, 2, 25500);
    sc_setup.check_user_nft_balance(&first_user, CP_TOKEN_ID, 3, 25500);
    sc_setup.check_user_balance_denominated(&first_user, STABLECOIN_TOKEN_ID, exp9(20_499_999_999_982)); // (19_900 + 599.999999982) + 0

}

pub fn exp15(value: u64) -> num_bigint::BigUint {
    value.mul(rust_biguint!(10).pow(15))
}

pub fn exp17(value: u64) -> num_bigint::BigUint {
    value.mul(rust_biguint!(10).pow(17))
}

pub fn exp9(value: u64) -> num_bigint::BigUint {
    value.mul(rust_biguint!(10).pow(9))
}
