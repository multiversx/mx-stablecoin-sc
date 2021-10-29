elrond_wasm::imports!();

use crate::{fees::CurrentFeeConfiguration, hedging_agents::HedgingPosition, math::ONE};

#[elrond_wasm::module]
pub trait KeepersModule:
    crate::fees::FeesModule
    + crate::hedging_agents_events::HedgingAgentsEventsModule
    + crate::hedging_agents::HedgingAgentsModule
    + crate::hedging_token::HedgingTokenModule
    + crate::lending_events::LendingEventsModule
    + crate::lending::LendingModule
    + crate::liquidity_providers_events::LiquidityProvidersEventsModule
    + crate::liquidity_providers::LiquidityProvidersModule
    + crate::liquidity_token::LiquidityTokenModule
    + crate::math::MathModule
    + crate::pool_events::PoolEventsModule
    + crate::pools::PoolsModule
    + price_aggregator_proxy::PriceAggregatorModule
    + crate::token_common::TokenCommonModule
{
    #[endpoint(rebalancePool)]
    fn rebalance_pool(&self, collateral_id: TokenIdentifier) -> SCResult<()> {
        self.require_collateral_in_whitelist(&collateral_id)?;

        let collateral_value_in_dollars = self.get_collateral_value_in_dollars(&collateral_id)?;
        let collateral_precision = self.get_collateral_precision(&collateral_id);

        self.update_pool(&collateral_id, |pool| {
            let pool_value_in_dollars = self.multiply(
                &pool.collateral_amount,
                &collateral_value_in_dollars,
                &collateral_precision,
            );
            let old_collateral_amount = pool.collateral_amount.clone();

            // collateral value increased, so we move the extra to reserves
            if pool_value_in_dollars > pool.stablecoin_amount {
                let extra_collateral_in_dollars = &pool_value_in_dollars - &pool.stablecoin_amount;
                let extra_collateral_amount = self.divide(
                    &extra_collateral_in_dollars,
                    &collateral_value_in_dollars,
                    &collateral_precision,
                );

                pool.collateral_reserves += extra_collateral_amount;
            }
            // collateral value decreased, so we take collateral from the reserves to rebalance the pool
            else {
                let missing_collateral_in_dollars =
                    &pool.stablecoin_amount - &pool_value_in_dollars;
                let missing_collateral_amount = self.divide(
                    &missing_collateral_in_dollars,
                    &collateral_value_in_dollars,
                    &collateral_precision,
                );

                require!(
                    missing_collateral_amount <= pool.collateral_reserves,
                    "Not enough reserves to rebalance pool"
                );

                pool.collateral_reserves -= missing_collateral_amount;
            }

            self.pool_rebalanced_event(
                &collateral_id,
                &old_collateral_amount,
                &pool.stablecoin_amount,
                &pool.collateral_amount,
                &pool_value_in_dollars,
            );

            pool.stablecoin_amount = pool_value_in_dollars;

            Ok(())
        })
    }

    #[endpoint(updateFeesPercentage)]
    fn update_fees_percentage(&self, collateral_id: TokenIdentifier) {
        let hedging_ratio = self.calculate_current_hedging_ratio(&collateral_id);
        let mint_fee_percentage = self.calculate_mint_transaction_fees_percentage(&collateral_id);
        let burn_fee_percentage = self.calculate_burn_transaction_fees_percentage(&collateral_id);

        self.fees_updated_event(
            &collateral_id,
            &hedging_ratio,
            &mint_fee_percentage,
            &burn_fee_percentage,
        );

        self.current_fee_configuration(&collateral_id)
            .set(&CurrentFeeConfiguration {
                hedging_ratio,
                mint_fee_percentage,
                burn_fee_percentage,
            });
    }

    #[endpoint(splitFees)]
    fn split_fees(&self, collateral_id: TokenIdentifier) {
        let accumulated_fees = self.accumulated_tx_fees(&collateral_id).get();
        if accumulated_fees == 0u32 {
            return;
        }

        let liq_provider_fee_reward_percentage = self
            .liq_provider_fee_reward_percentage(&collateral_id)
            .get();
        let liq_provider_reward =
            self.calculate_percentage_of(&liq_provider_fee_reward_percentage, &accumulated_fees);
        let leftover = &accumulated_fees - &liq_provider_reward;

        let sft_nonce = self.liq_sft_nonce_for_collateral(&collateral_id).get();
        self.collateral_amount_for_liq_token(sft_nonce)
            .update(|amt| *amt += &liq_provider_reward);
        self.update_pool(&collateral_id, |pool| {
            pool.collateral_reserves += &leftover;
        });

        self.accumulated_tx_fees(&collateral_id).clear();

        self.fees_split_event(&collateral_id, &liq_provider_reward, &leftover);
    }

    #[endpoint(lendReserves)]
    fn lend_reserves(&self, collateral_id: TokenIdentifier) -> SCResult<AsyncCall> {
        self.lend(collateral_id)
    }

    #[endpoint(withdrawLendedReserves)]
    fn withdraw_lended_reserves(&self, collateral_id: TokenIdentifier) -> SCResult<AsyncCall> {
        self.withdraw(collateral_id)
    }

    #[endpoint(splitLendRewards)]
    fn split_lend_rewards(&self, collateral_id: TokenIdentifier) {
        let accumulated_rewards = self.accumulated_lend_rewards(&collateral_id).get();
        if accumulated_rewards == 0u32 {
            return;
        }

        let liq_provider_lend_reward_percentage = self
            .liq_provider_lend_reward_percentage(&collateral_id)
            .get();

        let liq_provider_reward = self
            .calculate_percentage_of(&liq_provider_lend_reward_percentage, &accumulated_rewards);
        let leftover = &accumulated_rewards - &liq_provider_reward;

        let sft_nonce = self.liq_sft_nonce_for_collateral(&collateral_id).get();
        self.collateral_amount_for_liq_token(sft_nonce)
            .update(|amt| *amt += &liq_provider_reward);
        self.update_pool(&collateral_id, |pool| {
            pool.collateral_reserves += &leftover;
        });

        self.accumulated_lend_rewards(&collateral_id).clear();

        self.lend_rewards_split_event(&collateral_id, &liq_provider_reward, &leftover);
    }

    #[endpoint(forceCloseHedgingPosition)]
    fn force_close_hedging_position(&self, nft_nonce: u64) -> SCResult<()> {
        self.require_not_liquidated(nft_nonce)?;

        let mut hedging_position = self.hedging_position(nft_nonce).get();
        let pool = self.get_pool(&hedging_position.collateral_id);

        let limit_hedge_amount = self.calculate_limit_hedge_amount(&pool.collateral_amount);
        require!(
            pool.total_covered_value_in_stablecoin > limit_hedge_amount,
            "May only force close after limit hedge amount is passed"
        );

        self.close_position(&hedging_position)?;

        let withdraw_amount = self.get_withdraw_amount_and_update_fees(&hedging_position, None)?;
        hedging_position.withdraw_amount_after_force_close = Some(withdraw_amount.clone());
        self.hedging_position(nft_nonce).set(&hedging_position);

        self.hedging_position_force_closed_event(nft_nonce, &withdraw_amount);

        Ok(())
    }

    #[endpoint(liquidateHedgingPosition)]
    fn liquidate_hedging_position(&self, nft_nonce: u64) -> SCResult<()> {
        self.require_not_liquidated(nft_nonce)?;

        let hedging_position = self.hedging_position(nft_nonce).get();
        self.require_not_closed(&hedging_position)?;

        let margin_ratio = self.calculate_margin_ratio(&hedging_position)?;
        let hedging_maintenance_ratio = self
            .hedging_maintenance_ratio(&hedging_position.collateral_id)
            .get();
        require!(
            margin_ratio <= hedging_maintenance_ratio,
            "Can only liquidate if margin ratio is below expected amount"
        );

        self.close_position(&hedging_position)?;
        self.update_pool(&hedging_position.collateral_id, |pool| {
            pool.collateral_reserves += &hedging_position.deposit_amount;
        });

        self.hedging_position(nft_nonce).clear();

        self.hedging_position_liquidated_event(nft_nonce, &hedging_position.deposit_amount);

        Ok(())
    }

    fn calculate_margin_ratio(
        &self,
        hedging_position: &HedgingPosition<Self::Api>,
    ) -> SCResult<BigUint> {
        let collateral_value_in_dollars =
            self.get_collateral_value_in_dollars(&hedging_position.collateral_id)?;

        // margin = x / y + (1 - initial_oracle / current_oracle)
        // where x is deposit_amount and y is amount_to_cover
        let amount_ratio = self.calculate_ratio(
            &hedging_position.deposit_amount,
            &hedging_position.covered_amount,
        );
        let price_ratio = self.calculate_ratio(
            &hedging_position.oracle_value_at_deposit_time,
            &collateral_value_in_dollars,
        );

        let one = BigUint::from(ONE);
        let result = if price_ratio <= one {
            let diff = one - price_ratio;
            amount_ratio + diff
        } else {
            let diff = price_ratio - one;
            amount_ratio - diff
        };

        Ok(result)
    }
}
