#![no_std]

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

pub mod aggregator_proxy;
pub mod collateral_provision;
pub mod config;
pub mod errors;
pub mod events;
pub mod virtual_liquidity_pools;

use crate::{
    aggregator_proxy::*,
    collateral_provision::CpTokenAttributes,
    config::State,
    errors::{
        ERROR_ACTIVE, ERROR_ALREADY_DEPLOYED, ERROR_BAD_PAYMENT_TOKENS, ERROR_CP_TOKEN_UNDEFINED,
        ERROR_DIVISION_SAFETY_CONSTANT_ZERO, ERROR_INVALID_AMOUNT, ERROR_NOT_ACTIVE,
        ERROR_NOT_AN_ESDT, ERROR_PRICE_AGGREGATOR_WRONG_ADDRESS, ERROR_SAME_TOKENS,
        ERROR_SLIPPAGE_EXCEEDED, ERROR_STABLECOIN_TOKEN_NOT_ISSUED, ERROR_SWAP_NOT_ENABLED,
        ERROR_UNLISTED_COLLATERAL,
    },
    events::{ProvisionEvent, SwapEvent},
};

const PERCENTAGE: u64 = 100_000;

#[elrond_wasm::contract]
pub trait StablecoinV3:
    virtual_liquidity_pools::VLPModule
    + config::ConfigModule
    + events::EventsModule
    + collateral_provision::CollateralProvisionModule
{
    #[init]
    fn init(
        &self,
        price_aggregator_address: ManagedAddress,
        pool_recovery_period: u64,
        division_safety_constant: BigUint,
    ) {
        require!(
            !price_aggregator_address.is_zero(),
            ERROR_PRICE_AGGREGATOR_WRONG_ADDRESS
        );
        require!(
            division_safety_constant > 0u64,
            ERROR_DIVISION_SAFETY_CONSTANT_ZERO
        );
        self.price_aggregator_address()
            .set(&price_aggregator_address);
        self.pool_recovery_period().set(pool_recovery_period);
        self.division_safety_constant()
            .set(division_safety_constant);
        self.state().set(&State::Inactive);
    }

    #[only_owner]
    #[payable("EGLD")]
    #[endpoint(registerStablecoin)]
    fn register_stablecoin(
        &self,
        token_display_name: ManagedBuffer,
        token_ticker: ManagedBuffer,
        num_decimals: usize,
    ) {
        require!(!self.is_state_active(), ERROR_ACTIVE);
        let payment_amount = self.call_value().egld_value();
        self.stablecoin().issue_and_set_all_roles(
            payment_amount,
            token_display_name,
            token_ticker,
            num_decimals,
            None,
        );
    }

    #[only_owner]
    #[payable("*")]
    #[endpoint(deployStablecoin)]
    fn deploy_stablecoin(
        &self,
        collateral_token_id: TokenIdentifier,
        collateral_token_ticker: ManagedBuffer,
        stablecoin_token_ticker: ManagedBuffer,
        initial_stablecoin_token_id: TokenIdentifier,
        initial_stablecoin_token_ticker: ManagedBuffer,
        spread_fee_min_percent: BigUint,
    ) -> EsdtTokenPayment<Self::Api> {
        let (payment_token, payment_amount) = self.call_value().single_fungible_esdt();

        require!(
            !self.stablecoin().is_empty(),
            ERROR_STABLECOIN_TOKEN_NOT_ISSUED
        );

        let stablecoin_token_id = self.stablecoin().get_token_id();
        require!(
            collateral_token_id.is_valid_esdt_identifier(),
            ERROR_NOT_AN_ESDT
        );
        require!(
            stablecoin_token_id.is_valid_esdt_identifier(),
            ERROR_NOT_AN_ESDT
        );
        require!(
            collateral_token_id != stablecoin_token_id,
            ERROR_SAME_TOKENS
        );
        require!(
            payment_token == collateral_token_id,
            ERROR_BAD_PAYMENT_TOKENS
        );
        require!(self.base_pool().is_empty(), ERROR_ALREADY_DEPLOYED);

        self.spread_fee_min_percent().set(spread_fee_min_percent);

        self.base_collateral_token_id().set(&collateral_token_id);
        self.token_ticker(&collateral_token_id)
            .set(collateral_token_ticker);
        self.token_ticker(&stablecoin_token_id)
            .set(stablecoin_token_ticker);
        self.token_ticker(&initial_stablecoin_token_id)
            .set(initial_stablecoin_token_ticker);

        let caller = self.blockchain().get_caller();
        let collateral_price =
            self.get_exchange_rate(&collateral_token_id, &initial_stablecoin_token_id);

        let payment_value_denominated = (&payment_amount) * (&collateral_price);

        self.median_pool_delta()
            .set(payment_value_denominated.clone());
        self.pool_delta().set(payment_value_denominated.clone());

        let user_payment = self.mint_stablecoins(payment_value_denominated.clone());
        self.base_pool()
            .update(|total| *total = payment_value_denominated.clone());
        self.collateral_supply()
            .update(|total| *total += &payment_amount);

        if user_payment.amount > BigUint::zero() {
            self.send().direct_esdt(
                &caller,
                &user_payment.token_identifier,
                user_payment.token_nonce,
                &user_payment.amount,
            );
        }

        user_payment
    }

    #[payable("*")]
    #[endpoint(swapStablecoin)]
    fn swap_stablecoin(&self, amount_out_min: BigUint) {
        require!(self.can_swap(), ERROR_SWAP_NOT_ENABLED);

        let (token_in, amount_in) = self.call_value().single_fungible_esdt();
        let collateral_token_id = self.base_collateral_token_id().get();
        let stablecoin_token_id = self.stablecoin().get_token_id();

        require!(
            token_in == collateral_token_id || token_in == stablecoin_token_id,
            ERROR_BAD_PAYMENT_TOKENS
        );

        // Sets the swap type (stablecoin buy/sell)
        let mut stablecoin_buy = true;
        let mut token_out = stablecoin_token_id.clone();
        if token_in == stablecoin_token_id {
            stablecoin_buy = false;
            token_out = collateral_token_id.clone();
        }

        let caller = self.blockchain().get_caller();

        // Replenish delta at start of each block
        self.update_pool_delta();

        // Compute swap

        // Oracle prices
        let collateral_price = self.get_exchange_rate(&collateral_token_id, &stablecoin_token_id);
        let stablecoin_price = self.get_exchange_rate(&stablecoin_token_id, &stablecoin_token_id);

        let base_pool = self.base_pool().get();
        let min_swap_spread = BigUint::from(self.spread_fee_min_percent().get());
        let cp = &base_pool * &base_pool;
        let pool_delta = self.pool_delta().get();
        let median_pool_delta = self.median_pool_delta().get();

        // stablecoin_pool = base_pool + pool_delta;
        let mut stablecoin_pool = base_pool.clone();
        if pool_delta > median_pool_delta {
            stablecoin_pool += &pool_delta - &median_pool_delta;
        } else if median_pool_delta > pool_delta {
            stablecoin_pool -= &median_pool_delta - &pool_delta;
        }

        let collateral_pool = &cp / &stablecoin_pool;

        let offer_rate;
        let demand_rate;
        let offer_pool;
        let demand_pool;
        if stablecoin_buy {
            offer_rate = collateral_price.clone();
            demand_rate = stablecoin_price.clone();
            offer_pool = collateral_pool.clone();
            demand_pool = stablecoin_pool.clone();
        } else {
            offer_rate = stablecoin_price.clone();
            demand_rate = collateral_price.clone();
            offer_pool = stablecoin_pool.clone();
            demand_pool = collateral_pool.clone();
        }

        // Calculate optimal value of amount_out
        let amount_out_optimal = &amount_in * &offer_rate / &demand_rate;

        // Constant-product based swap amount
        let demand_base_amount = &demand_pool - &(&cp / &(&offer_pool + &amount_out_optimal));

        // Calculate spread
        let mut spread;
        if amount_out_optimal > demand_base_amount {
            spread = (&amount_out_optimal - &demand_base_amount) * PERCENTAGE / &amount_out_optimal;
        } else if amount_out_optimal < demand_base_amount {
            spread = (&demand_base_amount - &amount_out_optimal) * PERCENTAGE / &demand_base_amount;
        } else {
            spread = BigUint::zero();
        }

        if spread < min_swap_spread {
            spread = min_swap_spread;
        }

        let spread_fee = &amount_out_optimal * &spread / BigUint::from(PERCENTAGE);

        let amount_out_after_fee = &amount_out_optimal - &spread_fee;

        require!(
            amount_out_after_fee >= amount_out_min,
            ERROR_SLIPPAGE_EXCEEDED
        );

        let user_payment: EsdtTokenPayment<Self::Api>;

        if stablecoin_buy {
            self.pool_delta()
                .update(|total| *total -= amount_out_optimal.clone());
            self.collateral_supply()
                .update(|total| *total += amount_in.clone());
            self.mint_stablecoins(amount_out_optimal.clone());

            self.update_rewards(&stablecoin_token_id, &spread_fee);

            user_payment =
                EsdtTokenPayment::new(stablecoin_token_id.clone(), 0, amount_out_after_fee.clone());
        } else {
            self.pool_delta()
                .update(|total| *total += amount_in.clone());
            self.collateral_supply()
                .update(|total| *total -= &amount_out_optimal.clone());
            self.burn_stablecoins(amount_in.clone());

            self.update_rewards(&collateral_token_id, &spread_fee);

            user_payment =
                EsdtTokenPayment::new(collateral_token_id.clone(), 0, amount_out_after_fee.clone());
        }

        // Send tokens to caller
        self.send().direct_esdt(
            &caller,
            &user_payment.token_identifier,
            user_payment.token_nonce,
            &user_payment.amount,
        );

        // Emit event
        let swap_event = SwapEvent {
            caller: caller,
            token_id_in: token_in,
            token_amount_in: amount_in,
            token_id_out: token_out,
            token_amount_out: amount_out_after_fee,
            fee_amount: spread_fee,
            block: self.blockchain().get_block_nonce(),
            epoch: self.blockchain().get_block_epoch(),
            timestamp: self.blockchain().get_block_timestamp(),
        };
        self.emit_swap_event(&swap_event);
    }

    #[payable("*")]
    #[endpoint(provideCollateral)]
    fn provide_collateral(&self) {
        let (collateral_token_id, payment_amount) = self.call_value().single_fungible_esdt();

        require!(self.is_state_active(), ERROR_NOT_ACTIVE);
        require!(
            self.collateral_tokens().contains(&collateral_token_id),
            ERROR_UNLISTED_COLLATERAL
        );
        require!(
            !self.stablecoin().is_empty(),
            ERROR_STABLECOIN_TOKEN_NOT_ISSUED
        );
        require!(payment_amount > BigUint::zero(), ERROR_INVALID_AMOUNT);

        let cp_token_id = self.cp_token().get_token_id();
        let stablecoin_token_id = self.stablecoin().get_token_id();

        let collateral_price = self.get_exchange_rate(&collateral_token_id, &stablecoin_token_id);

        let cp_token_amount = &payment_amount * &collateral_price;

        let caller = self.blockchain().get_caller();
        let current_epoch = self.blockchain().get_block_epoch();

        let virtual_position = CpTokenAttributes {
            stablecoin_reward_per_share: self
                .reward_per_share(&self.stablecoin().get_token_id())
                .get(),
            collateral_reward_per_share: self
                .reward_per_share(&self.base_collateral_token_id().get())
                .get(),
            entering_epoch: current_epoch,
        };

        let user_payment = self.mint_cp_tokens(cp_token_id, cp_token_amount, &virtual_position);
        self.send().direct_esdt(
            &caller,
            &user_payment.token_identifier,
            user_payment.token_nonce,
            &user_payment.amount,
        );

        // Emit collateral provision event
        let provision_event = ProvisionEvent {
            caller: caller,
            token_id_in: collateral_token_id,
            token_amount_in: payment_amount,
            block: self.blockchain().get_block_nonce(),
            epoch: self.blockchain().get_block_epoch(),
            timestamp: self.blockchain().get_block_timestamp(),
        };
        self.emit_provide_collateral_event(&provision_event);
    }

    #[payable("*")]
    #[endpoint(claimFeeRewards)]
    fn claim_fee_rewards(&self) {
        let (cp_token_id, payment_nonce, payment_amount) =
            self.call_value().single_esdt().into_tuple();

        require!(self.is_state_active(), ERROR_NOT_ACTIVE);
        require!(!self.cp_token().is_empty(), ERROR_CP_TOKEN_UNDEFINED);
        require!(
            !self.stablecoin().is_empty(),
            ERROR_STABLECOIN_TOKEN_NOT_ISSUED
        );
        require!(
            cp_token_id == self.cp_token().get_token_id(),
            ERROR_BAD_PAYMENT_TOKENS
        );
        require!(payment_amount > BigUint::zero(), ERROR_INVALID_AMOUNT);

        let stablecoin_token_id = self.stablecoin().get_token_id();
        let base_collateral_token_id = self.base_collateral_token_id().get();

        let (stablecoin_rewards, collateral_rewards) =
            self.calculate_fee_rewards(&cp_token_id, payment_nonce, &payment_amount);
        self.reward_reserve(&stablecoin_token_id)
            .update(|x| *x -= &stablecoin_rewards);
        self.reward_reserve(&base_collateral_token_id)
            .update(|x| *x -= &collateral_rewards);
        self.burn_cp_tokens(&cp_token_id, payment_nonce, &payment_amount);

        let current_epoch = self.blockchain().get_block_epoch();

        let virtual_position = CpTokenAttributes {
            stablecoin_reward_per_share: self.reward_per_share(&stablecoin_token_id).get(),
            collateral_reward_per_share: self.reward_per_share(&base_collateral_token_id).get(),
            entering_epoch: current_epoch,
        };

        let user_cp_payment = self.mint_cp_tokens(cp_token_id, payment_amount, &virtual_position);

        let caller = self.blockchain().get_caller();

        let mut payments = ManagedVec::new();

        payments.push(EsdtTokenPayment::new(
            stablecoin_token_id,
            0,
            stablecoin_rewards,
        ));
        payments.push(EsdtTokenPayment::new(
            base_collateral_token_id,
            0,
            collateral_rewards,
        ));
        payments.push(user_cp_payment);

        self.send().direct_multi(&caller, &payments);
    }

    #[view(calculateFeeRewards)]
    fn calculate_fee_rewards(
        &self,
        cp_token_id: &TokenIdentifier,
        nonce: u64,
        amount: &BigUint,
    ) -> (BigUint, BigUint) {
        let cp_token_attributes =
            self.get_cp_token_attributes::<CpTokenAttributes<Self::Api>>(cp_token_id, nonce);
        let stablecoin_rps = self
            .reward_per_share(&self.stablecoin().get_token_id())
            .get();
        let collateral_rps = self
            .reward_per_share(&self.base_collateral_token_id().get())
            .get();
        let stablecoin_rewards = self.compute_rewards(
            amount,
            &stablecoin_rps,
            &cp_token_attributes.stablecoin_reward_per_share,
        );
        let collateral_rewards = self.compute_rewards(
            amount,
            &collateral_rps,
            &cp_token_attributes.collateral_reward_per_share,
        );

        (stablecoin_rewards, collateral_rewards)
    }

    // proxy

    #[proxy]
    fn aggregator_proxy(&self, sc_address: ManagedAddress) -> aggregator_proxy::Proxy<Self::Api>;

    fn get_exchange_rate(&self, from: &TokenIdentifier, to: &TokenIdentifier) -> BigUint {
        let price_aggregator_address = self.price_aggregator_address().get();

        let from_ticker = self.token_ticker(from).get(); // b"EGLD";
        let to_ticker = self.token_ticker(to).get(); // b"USD";

        let result: AggregatorResultAsMultiValue<Self::Api> = self
            .aggregator_proxy(price_aggregator_address)
            .latest_price_feed(from_ticker, to_ticker)
            .execute_on_dest_context();

        AggregatorResult::from(result).price
    }
}
