#![no_std]

elrond_wasm::imports!();

pub mod user_deposit;
use user_deposit::*;

// for consistency, we're using the same precision as the liquidity pool
pub const BASE_PRECISION: u64 = 1_000_000_000;

#[elrond_wasm_derive::contract]
pub trait LockRewards {
    #[init]
    fn init(
        &self,
        stablecoin_token_id: TokenIdentifier,
        percentage_reward_per_block: Self::BigUint,
    ) -> SCResult<()> {
        require!(
            stablecoin_token_id.is_valid_esdt_identifier(),
            "invalid stablecoin token id"
        );

        self.try_set_percentage_rewards_per_block(&percentage_reward_per_block)
    }

    // endpoints - owner-only

    fn set_percentage_reward_per_block(
        &self,
        percentage_reward_per_block: Self::BigUint,
    ) -> SCResult<()> {
        only_owner!(self, "only owner may call this function");

        let old_percentage = self.percentage_reward_per_block().get();
        self.try_set_percentage_rewards_per_block(&percentage_reward_per_block)?;

        let current_block_nonce = self.blockchain().get_block_nonce();
        for address in self.user_deposits().keys() {
            let mut user_deposit = self.get_user_deposit_or_default(&address);
            let additional_cummulated_rewards = self.calculate_cumulated_rewards(
                &user_deposit,
                current_block_nonce,
                &old_percentage,
            );

            user_deposit.cummulated_rewards += additional_cummulated_rewards;
            user_deposit.last_claim_block_nonce = current_block_nonce;

            self.user_deposits().insert(address, user_deposit);
        }

        Ok(())
    }

    // endpoints

    #[payable("*")]
    #[endpoint]
    fn deposit(
        &self,
        #[payment_token] token_id: TokenIdentifier,
        #[payment] amount: Self::BigUint,
    ) -> SCResult<()> {
        require!(
            token_id == self.stablecoin_token_id().get(),
            "Wrong payment token"
        );
        require!(amount > 0, "Must deposit more than 0");

        let caller = self.blockchain().get_caller();
        let current_block_nonce = self.blockchain().get_block_nonce();
        let mut user_deposit = self.get_user_deposit_or_default(&caller);

        if user_deposit.amount > 0 {
            let additional_cummulated_rewards = self.calculate_cumulated_rewards(
                &user_deposit,
                current_block_nonce,
                &self.percentage_reward_per_block().get(),
            );

            user_deposit.cummulated_rewards += additional_cummulated_rewards;
        }

        user_deposit.last_claim_block_nonce = current_block_nonce;
        user_deposit.amount += amount;
        self.user_deposits().insert(caller, user_deposit);

        Ok(())
    }

    // private

    fn try_set_percentage_rewards_per_block(
        &self,
        percentage_reward_per_block: &Self::BigUint,
    ) -> SCResult<()> {
        require!(
            *percentage_reward_per_block > 0 && *percentage_reward_per_block <= BASE_PRECISION,
            "Invalid percentage"
        );

        self.percentage_reward_per_block()
            .set(percentage_reward_per_block);

        Ok(())
    }

    fn calculate_cumulated_rewards(
        &self,
        user_deposit: &UserDeposit<Self::BigUint>,
        current_block_nonce: u64,
        percentage_reward_per_block: &Self::BigUint,
    ) -> Self::BigUint {
        let amount_per_block =
            (&user_deposit.amount * percentage_reward_per_block) / BASE_PRECISION.into();
        let blocks_waited = current_block_nonce - user_deposit.last_claim_block_nonce;

        amount_per_block * blocks_waited.into()
    }

    fn get_user_deposit_or_default(&self, address: &Address) -> UserDeposit<Self::BigUint> {
        match self.user_deposits().get(address) {
            Some(dep) => dep,
            None => UserDeposit::default(),
        }
    }

    // storage

    #[storage_mapper("stablecoinTokenId")]
    fn stablecoin_token_id(&self) -> SingleValueMapper<Self::Storage, TokenIdentifier>;

    #[storage_mapper("percentageRewardPerBlock")]
    fn percentage_reward_per_block(&self) -> SingleValueMapper<Self::Storage, Self::BigUint>;

    #[storage_mapper("userDeposits")]
    fn user_deposits(&self) -> MapMapper<Self::Storage, Address, UserDeposit<Self::BigUint>>;
}
