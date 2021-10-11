elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use crate::math::ONE;

#[derive(TypeAbi, TopEncode, TopDecode)]
pub struct HedgingPosition<M: ManagedTypeApi> {
    pub collateral_id: TokenIdentifier<M>,
    pub deposit_amount: BigUint<M>,
    pub covered_amount: BigUint<M>,
    pub oracle_value_at_deposit_time: BigUint<M>,
    pub creation_timestamp: u64,
}

#[elrond_wasm::module]
pub trait HedgingAgentsModule:
    crate::fees::FeesModule
    + crate::hedging_token::HedgingTokenModule
    + crate::math::MathModule
    + crate::pools::PoolsModule
    + price_aggregator_proxy::PriceAggregatorModule
{
    #[payable("*")]
    #[endpoint(openHedgingPosition)]
    fn open_hedging_position(
        &self,
        #[payment_token] payment_token: TokenIdentifier,
        #[payment_amount] payment_amount: BigUint,
        amount_to_cover: BigUint,
        max_oracle_value: BigUint,
    ) -> SCResult<()> {
        self.require_collateral_in_whitelist(&payment_token)?;

        let collateral_value_in_dollars = self.get_collateral_value_in_dollars(&payment_token)?;
        require!(
            collateral_value_in_dollars <= max_oracle_value,
            "Oracle value is higher than the provided max"
        );

        let mut pool = self.get_pool(&payment_token);
        pool.total_collateral_covered += &amount_to_cover;
        require!(
            pool.total_collateral_covered <= ONE,
            "Trying to cover too much collateral"
        );

        let transaction_fees_percentage =
            self.get_hedging_position_open_transaction_fees_percentage(&payment_token);
        let fees_amount_in_collateral =
            self.calculate_percentage_of(&transaction_fees_percentage, &payment_amount);
        let collateral_amount = &payment_amount - &fees_amount_in_collateral;

        let max_leverage = self.max_leverage(&payment_token).get();
        let position_leverage = &amount_to_cover / &collateral_amount;
        require!(position_leverage <= max_leverage, "Leverage too high");

        self.accumulated_tx_fees(&payment_token)
            .update(|accumulated_fees| *accumulated_fees += fees_amount_in_collateral);

        let caller = self.blockchain().get_caller();
        let nft_nonce = self.create_and_send_hedging_token(&caller);

        self.set_pool(&payment_token, &pool);
        self.hedging_position(nft_nonce).set(&HedgingPosition {
            collateral_id: payment_token,
            deposit_amount: collateral_amount,
            covered_amount: amount_to_cover,
            oracle_value_at_deposit_time: collateral_value_in_dollars,
            creation_timestamp: self.blockchain().get_block_timestamp(),
        });

        Ok(())
    }

    #[payable("*")]
    #[endpoint(closeHedgingPosition)]
    fn close_hedging_position(&self) -> SCResult<()> {
        Ok(())
    }

    // --- force close position if coverage_ratio is >=1

    // -- Slippage prot for hedgers: max oracle value for hedging, min value for exit

    // storage

    #[storage_mapper("hedgingPosition")]
    fn hedging_position(&self, nft_nonce: u64) -> SingleValueMapper<HedgingPosition<Self::Api>>;

    #[view(getMinHedgingPeriod)]
    #[storage_mapper("minHedgingPeriod")]
    fn min_hedging_period(&self) -> SingleValueMapper<u64>;
}
