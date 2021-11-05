elrond_wasm::imports!();

#[elrond_wasm::module]
pub trait PoolEventsModule {
    #[event("collateralAddedToWhitelist")]
    fn collateral_added_to_whitelist_event(
        &self,
        #[indexed] collateral_id: &TokenIdentifier,
        #[indexed] collateral_ticker: &ManagedBuffer,
        #[indexed] collateral_num_decimals: u32,
        #[indexed] max_leverage: &BigUint,
        #[indexed] min_fees_percentage: &BigUint,
        #[indexed] max_fees_percentage: &BigUint,
        #[indexed] hedging_maintenance_ratio: &BigUint,
        #[indexed] min_leftover_reserves_after_lend: &BigUint,
        #[indexed] reserves_lend_percentage: &BigUint,
        #[indexed] liq_provider_lend_reward_percentage: &BigUint,
        #[indexed] liq_provider_fee_reward_percentage: &BigUint,
        #[indexed] min_slippage_percentage: &BigUint,
        #[indexed] max_slippage_percentage: &BigUint,
    );

    #[event("collateralRemovedFromWhitelist")]
    fn collateral_removed_from_whitelist_event(&self, #[indexed] collateral_id: &TokenIdentifier);

    #[event("swap")]
    fn swap_event(
        &self,
        #[indexed] sender: &ManagedAddress,
        #[indexed] from_token: &TokenIdentifier,
        #[indexed] to_token: &TokenIdentifier,
        #[indexed] amount_in: &BigUint,
        #[indexed] amount_out: &BigUint,
    );

    #[event("poolRebalanced")]
    fn pool_rebalanced_event(
        &self,
        #[indexed] collateral_id: &TokenIdentifier,
        #[indexed] stablecoin_amount: &BigUint,
        #[indexed] old_collateral_amount: &BigUint,
        #[indexed] new_collateral_amount: &BigUint,
    );

    #[event("feesUpdated")]
    fn fees_updated_event(
        &self,
        #[indexed] collateral_id: &TokenIdentifier,
        #[indexed] hedging_ratio: &BigUint,
        #[indexed] mint_fee_percentage: &BigUint,
        #[indexed] burn_fee_percentage: &BigUint,
        #[indexed] slippage_percentage: &BigUint,
    );

    #[event("feesSplit")]
    fn fees_split_event(
        &self,
        #[indexed] collateral_id: &TokenIdentifier,
        #[indexed] liq_providers_amount: &BigUint,
        #[indexed] reserves_amount: &BigUint,
    );
}
