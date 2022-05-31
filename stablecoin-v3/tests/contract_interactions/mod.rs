use crate::contract_setup::{
    StablecoinContractSetup,
};
use elrond_wasm::{types::Address};
use elrond_wasm_debug::{
    managed_biguint, rust_biguint, DebugApi,
};
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
        self.b_mock.execute_esdt_transfer(
            caller,
            &self.sc_wrapper,
            payment_token,
            0,
            &rust_biguint!(payment_amount),
            |sc| {
                sc.swap_stablecoin(
                    managed_biguint!(amount_out_min),
                );
            },
        ).assert_ok();
    }
}