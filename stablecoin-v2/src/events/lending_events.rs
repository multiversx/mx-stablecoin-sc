elrond_wasm::imports!();

#[elrond_wasm::module]
pub trait LendingEventsModule {
    #[event("reservesLended")]
    fn reserves_lended_event(
        &self,
        #[indexed] collateral_id: &TokenIdentifier,
        #[indexed] lend_epoch: u64,
        #[indexed] lend_token_nonce: u64,
        #[indexed] lended_amount: &BigUint,
    );

    #[event("lendedReservesWithdrawn")]
    fn lended_reserves_withdrawn_event(
        &self,
        #[indexed] collateral_id: &TokenIdentifier,
        #[indexed] withdraw_amount: &BigUint,
    );

    #[event("lendRewardsSplit")]
    fn lend_rewards_split_event(
        &self,
        #[indexed] collateral_id: &TokenIdentifier,
        #[indexed] liq_providers_amount: &BigUint,
        #[indexed] reserves_amount: &BigUint,
    );
}
