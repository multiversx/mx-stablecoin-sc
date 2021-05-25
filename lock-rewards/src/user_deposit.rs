use elrond_wasm::api::BigUintApi;

elrond_wasm::derive_imports!();

#[derive(TypeAbi, TopEncode, TopDecode)]
pub struct UserDeposit<BigUint: BigUintApi> {
    pub amount: BigUint,

    // updated when savings rate is changed or user withdraws part or all of their deposit
    pub cummulated_rewards: BigUint,
    
    pub last_claim_block_nonce: u64,
}

impl<BigUint: BigUintApi> Default for UserDeposit<BigUint> {
    fn default() -> Self {
        UserDeposit {
            amount: BigUint::zero(),
            cummulated_rewards: BigUint::zero(),
            last_claim_block_nonce: 0,
        }
    }
}
