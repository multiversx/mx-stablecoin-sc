elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use crate::math::ONE;

pub struct WithdrawAmountFeeSplit<M: ManagedTypeApi> {
    pub withdraw_amount: BigUint<M>,
    pub fees_amount: BigUint<M>,
}

#[derive(TypeAbi, TopEncode, TopDecode)]
pub struct HedgingPosition<M: ManagedTypeApi> {
    pub collateral_id: TokenIdentifier<M>,
    pub deposit_amount: BigUint<M>,
    pub covered_amount: BigUint<M>,
    pub oracle_value_at_deposit_time: BigUint<M>,
    pub creation_timestamp: u64,
    pub withdraw_amount_after_force_close: Option<BigUint<M>>,
}

impl<M: ManagedTypeApi> HedgingPosition<M> {
    #[inline(always)]
    pub fn is_closed(&self) -> bool {
        self.withdraw_amount_after_force_close.is_some()
    }
}

#[elrond_wasm::module]
pub trait HedgingAgentsModule:
    crate::fees::FeesModule
    + crate::hedging_token::HedgingTokenModule
    + crate::math::MathModule
    + crate::pools::PoolsModule
    + price_aggregator_proxy::PriceAggregatorModule
    + crate::token_common::TokenCommonModule
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
        let target_hedge_amount = self.calculate_target_hedge_amount(&pool.collateral_amount);
        require!(
            pool.total_collateral_covered <= target_hedge_amount,
            "Over target hedge amount, no new positions may be opened"
        );

        pool.total_collateral_covered += &amount_to_cover;
        require!(
            pool.total_collateral_covered <= pool.collateral_amount,
            "Trying to cover too much collateral"
        );
        require!(
            pool.total_collateral_covered <= target_hedge_amount,
            "Position would go over target hedge amount"
        );

        let collateral_precision = self.get_collateral_precision(&payment_token);
        let amount_to_cover_in_stablecoin = self.multiply(
            &collateral_value_in_dollars,
            &amount_to_cover,
            &collateral_precision,
        );
        pool.total_covered_value_in_stablecoin += amount_to_cover_in_stablecoin;

        let transaction_fees_percentage =
            self.get_hedging_position_open_transaction_fees_percentage(&payment_token);
        let fees_amount_in_collateral =
            self.calculate_percentage_of(&transaction_fees_percentage, &payment_amount);
        let collateral_amount = &payment_amount - &fees_amount_in_collateral;

        pool.total_hedging_agents_deposit += &collateral_amount;

        let hedging_position = HedgingPosition {
            collateral_id: payment_token.clone(),
            deposit_amount: collateral_amount,
            covered_amount: amount_to_cover,
            oracle_value_at_deposit_time: collateral_value_in_dollars,
            creation_timestamp: self.blockchain().get_block_timestamp(),
            withdraw_amount_after_force_close: None,
        };
        self.require_under_max_leverage(&hedging_position)?;

        self.accumulated_tx_fees(&payment_token)
            .update(|accumulated_fees| *accumulated_fees += fees_amount_in_collateral);

        let caller = self.blockchain().get_caller();
        let nft_nonce = self.create_hedging_token();
        self.send_hedging_token(&caller, nft_nonce);

        self.set_pool(&payment_token, &pool);
        self.hedging_position(nft_nonce).set(&hedging_position);

        Ok(())
    }

    #[payable("*")]
    #[endpoint(addMargin)]
    fn add_margin(&self) -> SCResult<()> {
        let nr_required_transfers = 2;
        let transfers: Vec<EsdtTokenPayment<Self::Api>> =
            self.call_value().all_esdt_transfers().into_iter().collect();
        require!(
            transfers.len() == nr_required_transfers,
            "Invalid number of transfers"
        );

        let first_transfer = &transfers[0];
        let second_transfer = &transfers[1];

        let hedging_token_id = self.hedging_token_id().get();
        require!(
            first_transfer.token_identifier == hedging_token_id,
            "First token should be the hedging NFT"
        );

        let nft_nonce = first_transfer.token_nonce;
        self.hedging_position(nft_nonce).update(|hedging_pos| {
            require!(
                second_transfer.token_identifier == hedging_pos.collateral_id,
                "Second token should be the collateral for the position"
            );
            self.require_not_closed(hedging_pos)?;

            hedging_pos.deposit_amount += &second_transfer.amount;
            self.require_under_max_leverage(hedging_pos)?;

            Ok(())
        })?;
        self.update_pool(&second_transfer.token_identifier, |pool| {
            pool.total_hedging_agents_deposit += &second_transfer.amount;
        });

        // return the nft
        let caller = self.blockchain().get_caller();
        self.send_hedging_token(&caller, nft_nonce);

        Ok(())
    }

    #[payable("*")]
    #[endpoint(removeMargin)]
    fn remove_margin(
        &self,
        #[payment_token] payment_token: TokenIdentifier,
        #[payment_nonce] payment_nonce: u64,
        amount_to_remove: BigUint,
    ) -> SCResult<()> {
        let hedging_token_id = self.hedging_token_id().get();
        require!(
            payment_token == hedging_token_id,
            "Token should be the hedging NFT"
        );

        self.hedging_position(payment_nonce).update(|hedging_pos| {
            require!(
                amount_to_remove < hedging_pos.deposit_amount,
                "Remove amount higher than total deposit"
            );
            self.require_not_closed(hedging_pos)?;

            hedging_pos.deposit_amount -= &amount_to_remove;
            self.require_under_max_leverage(hedging_pos)?;

            Ok(())
        })?;
        self.update_pool(&payment_token, |pool| {
            pool.total_hedging_agents_deposit -= amount_to_remove;
        });

        // return the nft
        let caller = self.blockchain().get_caller();
        self.send_hedging_token(&caller, payment_nonce);

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
        self.require_not_liquidated(payment_nonce)?;

        let hedging_position = self.hedging_position(payment_nonce).get();
        let withdraw_amount = match hedging_position.withdraw_amount_after_force_close {
            Some(amt) => amt,
            None => {
                self.close_position(&hedging_position)?;

                let amt = self.get_withdraw_amount_and_update_fees(
                    &hedging_position,
                    Some(min_oracle_value),
                )?;
                self.update_pool_after_closed_position(
                    &hedging_position.collateral_id,
                    &hedging_position.deposit_amount,
                    &amt,
                );

                amt
            }
        };

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

    // private

    // deduplicates code for close, force-close and liquidate
    fn close_position(&self, hedging_position: &HedgingPosition<Self::Api>) -> SCResult<()> {
        self.require_not_closed(hedging_position)?;

        let mut pool = self.get_pool(&hedging_position.collateral_id);

        let current_time = self.blockchain().get_block_timestamp();
        let time_diff = current_time - hedging_position.creation_timestamp;
        let min_hedging_period_seconds = self.min_hedging_period_seconds().get();
        require!(
            time_diff >= min_hedging_period_seconds,
            "Trying to close too early"
        );

        let collateral_precision = self.get_collateral_precision(&hedging_position.collateral_id);
        let amount_to_cover_in_stablecoin = self.multiply(
            &hedging_position.oracle_value_at_deposit_time,
            &hedging_position.covered_amount,
            &collateral_precision,
        );
        pool.total_covered_value_in_stablecoin -= amount_to_cover_in_stablecoin;
        pool.total_collateral_covered -= &hedging_position.covered_amount;

        self.set_pool(&hedging_position.collateral_id, &pool);

        Ok(())
    }

    fn get_withdraw_amount_and_update_fees(
        &self,
        hedging_position: &HedgingPosition<Self::Api>,
        opt_min_oracle_value: Option<BigUint>,
    ) -> SCResult<BigUint> {
        let withdraw_amount_fees_pair =
            self.calculate_withdraw_and_fee_amount(hedging_position, opt_min_oracle_value)?;

        self.accumulated_tx_fees(&hedging_position.collateral_id)
            .update(|accumulated_fees| *accumulated_fees += &withdraw_amount_fees_pair.fees_amount);

        Ok(withdraw_amount_fees_pair.withdraw_amount)
    }

    fn calculate_withdraw_and_fee_amount(
        &self,
        hedging_position: &HedgingPosition<Self::Api>,
        opt_min_oracle_value: Option<BigUint>,
    ) -> SCResult<WithdrawAmountFeeSplit<Self::Api>> {
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
            let factor = &one - &price_ratio;
            let extra_amount = self.multiply(&factor, &hedging_position.covered_amount, &one);

            &hedging_position.deposit_amount + &extra_amount
        } else {
            let factor = &price_ratio - &one;
            let deducted_amount = self.multiply(&factor, &hedging_position.covered_amount, &one);

            &hedging_position.deposit_amount - &deducted_amount
        };

        let transaction_fees_percentage = self
            .get_hedging_position_close_transaction_fees_percentage(
                &hedging_position.collateral_id,
            );
        let fees_amount =
            self.calculate_percentage_of(&transaction_fees_percentage, &base_withdraw_amount);
        let withdraw_amount = &base_withdraw_amount - &fees_amount;

        Ok(WithdrawAmountFeeSplit {
            withdraw_amount,
            fees_amount,
        })
    }

    fn update_pool_after_closed_position(
        &self,
        collateral_id: &TokenIdentifier,
        deposit_amount: &BigUint,
        withdraw_amount: &BigUint,
    ) {
        if withdraw_amount > deposit_amount {
            let hedger_reward = withdraw_amount - deposit_amount;
            self.update_pool(collateral_id, |pool| {
                pool.total_hedging_agents_deposit -= deposit_amount;
                pool.collateral_amount -= hedger_reward
            });
        } else {
            let hedger_penalty = deposit_amount - withdraw_amount;
            self.update_pool(collateral_id, |pool| {
                pool.total_hedging_agents_deposit -= deposit_amount;
                pool.collateral_amount += hedger_penalty
            });
        }
    }

    #[inline(always)]
    fn calculate_leverage(
        &self,
        collateral_amount: &BigUint,
        amount_to_cover: &BigUint,
    ) -> BigUint {
        // x + y / x
        self.calculate_ratio(&(collateral_amount + amount_to_cover), collateral_amount)
    }

    fn require_under_max_leverage(
        &self,
        hedging_position: &HedgingPosition<Self::Api>,
    ) -> SCResult<()> {
        let max_leverage = self.max_leverage(&hedging_position.collateral_id).get();
        let position_leverage = self.calculate_leverage(
            &hedging_position.deposit_amount,
            &hedging_position.covered_amount,
        );
        require!(position_leverage <= max_leverage, "Leverage too high");

        Ok(())
    }

    fn require_not_closed(&self, hedging_position: &HedgingPosition<Self::Api>) -> SCResult<()> {
        require!(!hedging_position.is_closed(), "Position already closed");
        Ok(())
    }

    fn require_not_liquidated(&self, nft_nonce: u64) -> SCResult<()> {
        require!(
            !self.hedging_position(nft_nonce).is_empty(),
            "Position liquidated"
        );
        Ok(())
    }

    // storage

    #[storage_mapper("hedgingPosition")]
    fn hedging_position(&self, nft_nonce: u64) -> SingleValueMapper<HedgingPosition<Self::Api>>;

    #[view(getMinHedgingPeriodSeconds)]
    #[storage_mapper("minHedgingPeriodSeconds")]
    fn min_hedging_period_seconds(&self) -> SingleValueMapper<u64>;

    #[view(getMaxLeverage)]
    #[storage_mapper("maxLeverage")]
    fn max_leverage(&self, collateral_id: &TokenIdentifier) -> SingleValueMapper<BigUint>;

    #[view(getHedgingMaintenanceRatio)]
    #[storage_mapper("hedgingMaintenanceRatio")]
    fn hedging_maintenance_ratio(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SingleValueMapper<BigUint>;
}
