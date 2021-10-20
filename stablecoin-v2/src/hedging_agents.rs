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
    pub withdraw_amount_after_force_close: Option<BigUint<M>>,
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
            pool.total_collateral_covered <= pool.collateral_amount,
            "Trying to cover too much collateral"
        );

        let transaction_fees_percentage =
            self.get_hedging_position_open_transaction_fees_percentage(&payment_token);
        let fees_amount_in_collateral =
            self.calculate_percentage_of(&transaction_fees_percentage, &payment_amount);
        let collateral_amount = &payment_amount - &fees_amount_in_collateral;

        let max_leverage = self.max_leverage(&payment_token).get();
        let position_leverage = self.calculate_leverage(&collateral_amount, &amount_to_cover);
        require!(position_leverage <= max_leverage, "Leverage too high");

        self.accumulated_tx_fees(&payment_token)
            .update(|accumulated_fees| *accumulated_fees += fees_amount_in_collateral);

        let caller = self.blockchain().get_caller();
        let nft_nonce = self.create_and_send_hedging_token(&caller);

        pool.hedging_positions.push(nft_nonce);

        self.set_pool(&payment_token, &pool);
        self.hedging_position(nft_nonce).set(&HedgingPosition {
            collateral_id: payment_token,
            deposit_amount: collateral_amount,
            covered_amount: amount_to_cover,
            oracle_value_at_deposit_time: collateral_value_in_dollars,
            creation_timestamp: self.blockchain().get_block_timestamp(),
            withdraw_amount_after_force_close: None,
        });

        Ok(())
    }

    #[payable("*")]
    #[endpoint(closeHedgingPosition)]
    fn close_hedging_position(
        &self,
        #[payment_token] payment_token: TokenIdentifier,
        #[payment_nonce] payment_nonce: u64,
        min_oracle_value: BigUint,
    ) -> SCResult<()> {
        let hedging_token_id = self.hedging_token_id().get();
        require!(
            payment_token == hedging_token_id,
            "May only pay with Hedging NFT"
        );

        let hedging_position = self.hedging_position(payment_nonce).get();
        let withdraw_amount = match hedging_position.withdraw_amount_after_force_close {
            Some(amt) => amt,
            None => {
                self.close_position(payment_nonce, &hedging_position, Some(min_oracle_value))?
            }
        };

        // TODO: Check if there is enough balance, otherwise, send some other redeemable token

        self.hedging_position(payment_nonce).clear();
        self.burn_hedging_token(payment_nonce);

        let caller = self.blockchain().get_caller();
        self.send().direct(
            &caller,
            &hedging_position.collateral_id,
            0,
            &withdraw_amount,
            &[],
        );

        Ok(())
    }

    #[endpoint(forceCloseHedgingPosition)]
    fn force_close_hedging_position(&self, nft_nonce: u64) -> SCResult<()> {
        let mut hedging_position = self.hedging_position(nft_nonce).get();
        let coverage_ratio = self.get_coverage_ratio(&hedging_position.collateral_id);
        require!(
            coverage_ratio > ONE,
            "Coverage ratio not high enough to force close"
        );

        hedging_position.withdraw_amount_after_force_close =
            Some(self.close_position(nft_nonce, &hedging_position, None)?);

        self.hedging_position(nft_nonce).set(&hedging_position);

        Ok(())
    }

    // private

    #[inline(always)]
    fn calculate_leverage(
        &self,
        collateral_amount: &BigUint,
        amount_to_cover: &BigUint,
    ) -> BigUint {
        self.calculate_ratio(&(collateral_amount + amount_to_cover), amount_to_cover)
    }

    // deduplicates code for close and force-close
    fn close_position(
        &self,
        nft_nonce: u64,
        hedging_position: &HedgingPosition<Self::Api>,
        opt_min_oracle_value: Option<BigUint>,
    ) -> SCResult<BigUint> {
        let mut pool = self.get_pool(&hedging_position.collateral_id);

        let current_time = self.blockchain().get_block_timestamp();
        let time_diff = current_time - hedging_position.creation_timestamp;
        let min_hedging_period_seconds = self.min_hedging_period_seconds().get();
        require!(
            time_diff >= min_hedging_period_seconds,
            "Trying to close too early"
        );

        let collateral_value_in_dollars =
            self.get_collateral_value_in_dollars(&hedging_position.collateral_id)?;
        if let Some(min_oracle_value) = opt_min_oracle_value {
            require!(
                collateral_value_in_dollars >= min_oracle_value,
                "Oracle value is lower than the provided min"
            );
        }

        let price_ratio = self.calculate_ratio(
            &hedging_position.oracle_value_at_deposit_time,
            &collateral_value_in_dollars,
        );

        // withdraw_amount = x + y * (1 - initial_oracle / current_oracle),
        // where x is deposit_amount and y is amount_to_cover
        let one = BigUint::from(ONE);
        let base_withdraw_amount = if price_ratio <= one {
            let factor = one - price_ratio;
            let extra_amount = self.multiply(&factor, &hedging_position.covered_amount);

            &hedging_position.deposit_amount + &extra_amount
        } else {
            let factor = price_ratio - one;
            let deducted_amount = self.multiply(&factor, &hedging_position.covered_amount);

            &hedging_position.deposit_amount - &deducted_amount
        };

        let transaction_fees_percentage = self
            .get_hedging_position_close_transaction_fees_percentage(
                &hedging_position.collateral_id,
            );
        let fees_amount_in_collateral =
            self.calculate_percentage_of(&transaction_fees_percentage, &base_withdraw_amount);
        let withdraw_amount = &base_withdraw_amount - &fees_amount_in_collateral;

        self.accumulated_tx_fees(&hedging_position.collateral_id)
            .update(|accumulated_fees| *accumulated_fees += fees_amount_in_collateral);

        let pos_index = pool
            .hedging_positions
            .iter()
            .position(|nonce| *nonce == nft_nonce)
            .ok_or("Could not close position")?;
        let _ = pool.hedging_positions.swap_remove(pos_index);

        self.set_pool(&hedging_position.collateral_id, &pool);

        Ok(withdraw_amount)
    }

    // storage

    #[storage_mapper("hedgingPosition")]
    fn hedging_position(&self, nft_nonce: u64) -> SingleValueMapper<HedgingPosition<Self::Api>>;

    #[view(getMinHedgingPeriodSeconds)]
    #[storage_mapper("minHedgingPeriodSeconds")]
    fn min_hedging_period_seconds(&self) -> SingleValueMapper<u64>;
}
