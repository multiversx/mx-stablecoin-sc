use elrond_wasm::api::BigUintApi;

elrond_wasm::derive_imports!();

// for consistency, we're using the same precision as the liquidity pool
pub const BASE_PRECISION: u64 = 1_000_000_000;

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

impl<BigUint: BigUintApi> UserDeposit<BigUint> {
    pub fn accummulate_rewards(
        &mut self,
        current_block_nonce: u64,
        percentage_reward_per_block: &BigUint,
    ) {
        if self.amount == 0 {
            return;
        }

        let amount_per_block =
            (self.amount.clone() * percentage_reward_per_block.clone()) / BASE_PRECISION.into();
        let blocks_waited = current_block_nonce - self.last_claim_block_nonce;
        let additional_cummulated_rewards = amount_per_block * blocks_waited.into();

        self.cummulated_rewards += additional_cummulated_rewards;
        self.last_claim_block_nonce = current_block_nonce;
    }
}
