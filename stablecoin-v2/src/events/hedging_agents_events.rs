elrond_wasm::imports!();

#[elrond_wasm::module]
pub trait HedgingAgentsEventsModule {
    #[event("hedgingPositionOpened")]
    fn hedging_position_opened_event(
        &self,
        #[indexed] nft_nonce: u64,
        #[indexed] collateral_id: &TokenIdentifier,
        #[indexed] deposit_amount: &BigUint,
        #[indexed] covered_amount: &BigUint,
        #[indexed] oracle_value: &BigUint,
        #[indexed] timestamp: u64,
    );

    #[event("hedgingPositionClosed")]
    fn hedging_position_closed_event(
        &self,
        #[indexed] nft_nonce: u64,
        #[indexed] withdraw_amount_collateral: &BigUint,
        #[indexed] withdraw_amount_liq_tokens: &BigUint,
    );

    #[event("hedgingPositionForceClosed")]
    fn hedging_position_force_closed_event(
        &self,
        #[indexed] nft_nonce: u64,
        #[indexed] withdraw_amount: &BigUint,
    );

    #[event("hedgingPositionLiquidated")]
    fn hedging_position_liquidated_event(
        &self,
        #[indexed] nft_nonce: u64,
        #[indexed] reserves_added: &BigUint,
    );

    #[event("hedgingPositionAddedMargin")]
    fn hedging_position_added_margin_event(
        &self,
        #[indexed] nft_nonce: u64,
        #[indexed] margin_added: &BigUint,
    );

    #[event("hedgingPositionRemovedMargin")]
    fn hedging_position_removed_margin_event(
        &self,
        #[indexed] nft_nonce: u64,
        #[indexed] margin_removed: &BigUint,
    );
}
