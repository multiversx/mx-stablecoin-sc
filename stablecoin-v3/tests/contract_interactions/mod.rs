use crate::contract_setup::{StablecoinContractSetup, COLLATERAL_TOKEN_ID, STABLECOIN_TOKEN_ID};
use elrond_wasm::types::Address;
use elrond_wasm_debug::{managed_biguint, rust_biguint, DebugApi};
use stablecoin_v3::StablecoinV3;
use stablecoin_v3::config::ConfigModule;

impl<StablecoinContractObjBuilder> StablecoinContractSetup<StablecoinContractObjBuilder>
where
    StablecoinContractObjBuilder: 'static + Copy + Fn() -> stablecoin_v3::ContractObj<DebugApi>,
{
    pub fn swap_stablecoin(
        &mut self,
        caller: &Address,
        payment_token: &[u8],
        payment_amount: u64,
        amount_out_min: u64,
    ) {
        self.b_mock
            .execute_esdt_transfer(
                caller,
                &self.sc_wrapper,
                payment_token,
                0,
                &rust_biguint!(payment_amount),
                |sc| {
                    sc.swap_stablecoin(managed_biguint!(amount_out_min));
                },
            )
            .assert_ok();
    }

    pub fn change_fee_spread_percentage(
        &mut self,
        fee_spread_percentage: u64,
    ) {
        let rust_zero = rust_biguint!(0);
        self.b_mock
            .execute_tx(&self.owner_address, &self.sc_wrapper, &rust_zero, |sc| {
                sc.set_spread_fee_min_percent(managed_biguint!(fee_spread_percentage))
            })
            .assert_ok();
    }

    pub fn setup_new_user(
        &mut self,
        collateral_token_amount: u64,
        stablecoin_token_amount: u64,
    ) -> Address {
        let rust_zero = rust_biguint!(0);

        let new_user = self.b_mock.create_user_account(&rust_zero);
        self.b_mock
            .set_esdt_balance(&new_user, COLLATERAL_TOKEN_ID, &rust_biguint!(collateral_token_amount));
        self.b_mock
            .set_esdt_balance(&new_user, STABLECOIN_TOKEN_ID, &rust_biguint!(stablecoin_token_amount));
        new_user
    }

    pub fn check_user_balance(
        &self,
        address: &Address,
        token_id: &[u8],
        token_balance: u64,
    ) {
        self
        .b_mock
        .check_esdt_balance(&address, token_id, &rust_biguint!(token_balance));
    }

}
