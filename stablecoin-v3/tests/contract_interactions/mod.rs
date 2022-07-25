use crate::contract_setup::{
    StablecoinContractSetup, COLLATERAL_TOKEN_ID, OVERCOLLATERAL_TOKEN_ID, STABLECOIN_TOKEN_ID,
};
use elrond_wasm::types::Address;
use elrond_wasm_debug::{managed_biguint, rust_biguint, DebugApi};
use stablecoin_v3::collateral_provision::CpTokenAttributes;
use stablecoin_v3::config::ConfigModule;
use stablecoin_v3::StablecoinV3;

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
                &&Self::exp18(payment_amount),
                |sc| {
                    sc.swap_stablecoin(Self::to_managed_biguint(Self::exp18(amount_out_min)));
                },
            )
            .assert_ok();
    }

    pub fn provide_collateral(
        &mut self,
        caller: &Address,
        payment_token: &[u8],
        payment_amount: u64,
    ) {
        self.b_mock
            .execute_esdt_transfer(
                caller,
                &self.sc_wrapper,
                payment_token,
                0,
                &&Self::exp18(payment_amount),
                |sc| {
                    sc.provide_collateral();
                },
            )
            .assert_ok();
    }

    pub fn claim_fee_rewards(
        &mut self,
        caller: &Address,
        payment_token: &[u8],
        payment_nonce: u64,
        payment_amount: u64,
    ) {
        self.b_mock
            .execute_esdt_transfer(
                caller,
                &self.sc_wrapper,
                payment_token,
                payment_nonce,
                &&Self::exp18(payment_amount),
                |sc| {
                    sc.claim_fee_rewards();
                },
            )
            .assert_ok();
    }

    pub fn change_fee_spread_percentage(&mut self, fee_spread_percentage: u64) {
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
        self.b_mock.set_esdt_balance(
            &new_user,
            COLLATERAL_TOKEN_ID,
            &&Self::exp18(collateral_token_amount),
        );
        self.b_mock.set_esdt_balance(
            &new_user,
            STABLECOIN_TOKEN_ID,
            &&Self::exp18(stablecoin_token_amount),
        );
        new_user
    }

    pub fn setup_new_user_with_overcollateral(
        &mut self,
        collateral_token_amount: u64,
        stablecoin_token_amount: u64,
        overcollateral_token_amount: u64,
    ) -> Address {
        let new_user = self.setup_new_user(collateral_token_amount, stablecoin_token_amount);
        self.b_mock.set_esdt_balance(
            &new_user,
            OVERCOLLATERAL_TOKEN_ID,
            &&Self::exp18(overcollateral_token_amount),
        );
        new_user
    }

    pub fn check_user_balance(&self, address: &Address, token_id: &[u8], token_balance: u64) {
        self.b_mock
            .check_esdt_balance(&address, token_id, &&Self::exp18(token_balance));
    }

    pub fn check_user_balance_denominated(
        &self,
        address: &Address,
        token_id: &[u8],
        token_balance: num_bigint::BigUint,
    ) {
        self.b_mock
            .check_esdt_balance(&address, token_id, &token_balance);
    }

    pub fn check_user_nft_balance(
        &self,
        address: &Address,
        token_id: &[u8],
        token_nonce: u64,
        token_balance: u64,
    ) {
        self.b_mock.check_nft_balance::<CpTokenAttributes::<DebugApi>>(
            &address,
            token_id,
            token_nonce,
            &&Self::exp18(token_balance),
            None,
        );
    }
}
