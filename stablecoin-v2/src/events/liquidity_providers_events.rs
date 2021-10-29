elrond_wasm::imports!();

#[elrond_wasm::module]
pub trait LiquidityProvidersEventsModule {
    #[event("addedLiquidity")]
    fn added_liquidity_event(
        &self,
        #[indexed] sft_nonce: u64,
        #[indexed] collateral_id: &TokenIdentifier,
        #[indexed] collateral_amount: &BigUint,
        #[indexed] liq_tokens_amount_received: &BigUint,
    );

    #[event("removedLiquidity")]
    fn removed_liquidity_event(
        &self,
        #[indexed] sft_nonce: u64,
        #[indexed] collateral_id: &TokenIdentifier,
        #[indexed] liq_tokens_sent: &BigUint,
        #[indexed] collateral_amount_received: &BigUint,
    );
}
