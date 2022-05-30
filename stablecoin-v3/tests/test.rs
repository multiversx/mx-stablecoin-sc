mod stablecoin_contract_setup;

use elrond_wasm_debug::{managed_biguint, rust_biguint, DebugApi};
use stablecoin_contract_setup::*;

#[test]
fn init_test() {
    let _ = StablecoinContractSetup::new(stablecoin_v3::contract_obj);
}