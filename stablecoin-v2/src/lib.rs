#![no_std]

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

mod actors;
mod economics;
mod tokens;

use actors::*;
use economics::*;
use tokens::*;

// TODO: Add events

#[elrond_wasm::contract]
pub trait StablecoinV2:
    fees::FeesModule
    + hedging_agents::HedgingAgentsModule
    + hedging_token::HedgingTokenModule
    + keepers::KeepersModule
    + liquidity_providers::LiquidityProvidersModule
    + liquidity_token::LiquidityTokenModule
    + math::MathModule
    + pools::PoolsModule
    + price_aggregator_proxy::PriceAggregatorModule
    + stablecoin_token::StablecoinTokenModule
    + stable_seekers::StableSeekers
    + token_common::TokenCommonModule
{
    #[init]
    fn init(
        &self,
        price_aggregator_address: ManagedAddress,
        min_hedging_period_seconds: u64,
        target_hedging_ratio: BigUint,
        hedging_ratio_limit: BigUint,
    ) -> SCResult<()> {
        require!(
            self.blockchain()
                .is_smart_contract(&price_aggregator_address),
            "Price aggregator address is not a smart contract"
        );

        self.price_aggregator_address()
            .set(&price_aggregator_address);

        self.min_hedging_period_seconds()
            .set(&min_hedging_period_seconds);
        self.target_hedging_ratio().set(&target_hedging_ratio);
        self.hedging_ratio_limit().set(&hedging_ratio_limit);

        Ok(())
    }

    #[only_owner]
    #[endpoint(addCollateralToWhitelist)]
    fn add_collateral_to_whitelist(
        &self,
        collateral_id: TokenIdentifier,
        collateral_ticker: ManagedBuffer,
        collateral_num_decimals: u32,
        max_leverage: BigUint,
        min_fees_percentage: BigUint,
        max_fees_percentage: BigUint,
        hedging_maintenance_ratio: BigUint,
        liq_provider_fee_reward_percentage: BigUint,
        min_slippage_percentage: BigUint,
        max_slippage_percentage: BigUint,
    ) -> SCResult<()> {
        require!(
            min_fees_percentage <= max_fees_percentage
                && max_fees_percentage < math::PERCENTAGE_PRECISION,
            "Invalid fees percentages"
        );
        require!(
            min_slippage_percentage <= max_slippage_percentage
                && max_slippage_percentage < math::PERCENTAGE_PRECISION,
            "Invalid slippage percentages"
        );

        self.collateral_ticker(&collateral_id)
            .set(&collateral_ticker);
        self.collateral_num_decimals(&collateral_id)
            .set(&collateral_num_decimals);
        self.max_leverage(&collateral_id).set(&max_leverage);
        self.min_max_fees_percentage(&collateral_id)
            .set(&(min_fees_percentage, max_fees_percentage));
        self.hedging_maintenance_ratio(&collateral_id)
            .set(&hedging_maintenance_ratio);
        self.liq_provider_fee_reward_percentage(&collateral_id)
            .set(&liq_provider_fee_reward_percentage);
        self.min_max_slippage_percentage(&collateral_id)
            .set(&(min_slippage_percentage, max_slippage_percentage));
        self.collateral_whitelisted(&collateral_id).set(&true);

        // preserve the pool info if it was added, removed, and then added again
        self.pool_for_collateral(&collateral_id)
            .set_if_empty(&pools::Pool::new(self.raw_vm_api()));

        Ok(())
    }

    #[only_owner]
    #[endpoint(removeCollateralFromWhitelist)]
    fn remove_collateral_from_whitelist(&self, collateral_id: TokenIdentifier) {
        self.collateral_ticker(&collateral_id).clear();
        self.collateral_num_decimals(&collateral_id).clear();
        self.max_leverage(&collateral_id).clear();
        self.min_max_fees_percentage(&collateral_id).clear();
        self.hedging_maintenance_ratio(&collateral_id).clear();
        self.liq_provider_fee_reward_percentage(&collateral_id)
            .clear();
        self.min_max_slippage_percentage(&collateral_id).clear();
        self.collateral_whitelisted(&collateral_id).clear();
    }
}
