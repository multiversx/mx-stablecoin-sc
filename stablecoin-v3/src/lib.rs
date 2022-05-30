#![no_std]

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

pub mod aggregator_proxy;
pub mod config;
pub mod errors;
pub mod events;
pub mod virtual_liquidity_pools;

use crate::{
    aggregator_proxy::*,
    config::State,
    errors::{
        ERROR_ACTIVE, ERROR_BAD_PAYMENT_TOKENS, ERROR_NOT_AN_ESDT, ERROR_SAME_TOKENS,
        ERROR_SLIPPAGE_EXCEEDED, ERROR_PRICE_AGGREGATOR_WRONG_ADDRESS, ERROR_SWAP_NOT_ENABLED,
    },
    events::SwapEvent,
};

const MEDIAN_POOL_DELTA: u64 = 100_000_000;

#[elrond_wasm::contract]
pub trait StablecoinV3:
    virtual_liquidity_pools::VLPModule + config::ConfigModule + events::EventsModule
{
    #[init]
    fn init(&self, price_aggregator_address: ManagedAddress) {
        require!(!price_aggregator_address.is_zero(), ERROR_PRICE_AGGREGATOR_WRONG_ADDRESS);
        self.price_aggregator_address()
            .set(&price_aggregator_address);
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
        stablecoin_token_id: TokenIdentifier,
        spread_fee_min_percent: BigUint,
    ) -> BigUint {
        let (payment_amount, token_in) = self.call_value().payment_token_pair();
        
        require!(collateral_token_id.is_esdt(), ERROR_NOT_AN_ESDT);
        require!(stablecoin_token_id.is_esdt(), ERROR_NOT_AN_ESDT);
        require!(
            collateral_token_id != stablecoin_token_id,
            ERROR_SAME_TOKENS
        );
        require!(token_in == collateral_token_id, ERROR_BAD_PAYMENT_TOKENS);

        self.spread_fee_min_percent().set(spread_fee_min_percent);
        self.stablecoin().set_token_id(&stablecoin_token_id);
        self.collateral_token_id().set(&collateral_token_id);
        self.pool_delta().set(&BigUint::from(MEDIAN_POOL_DELTA));

        let caller = self.blockchain().get_caller();
        let collateral_price = self.get_exchange_rate(&collateral_token_id, &stablecoin_token_id);
        let payment_value_denominated = &payment_amount * &collateral_price;
        let user_payment = self.mint_stablecoins(payment_value_denominated.clone());
        self.base_pool().update(|total| *total = payment_value_denominated.clone());
        self.collateral_token_supply()
            .update(|total| *total += &payment_amount);

        let mut user_payments = ManagedVec::new();
        if user_payment.amount > BigUint::zero() {
            user_payments.push(user_payment)
        }

        self.send().direct_multi(&caller, &user_payments, &[]);

        payment_value_denominated
    }

    #[payable("*")]
    #[endpoint(swapStablecoin)]
    fn swap_stablecoin(&self, amount_out_min: BigUint) {

        require!(self.can_swap(), ERROR_SWAP_NOT_ENABLED);

        let (amount_in, token_in) = self.call_value().payment_token_pair();
        let collateral_token_id = self.collateral_token_id().get();
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
        let mut pool_delta = self.pool_delta().get();
        let stablecoin_pool = &base_pool + &pool_delta;
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
        require!(
            amount_out_min >= amount_out_optimal,
            ERROR_SLIPPAGE_EXCEEDED
        );

        // Constant-product based swap amount
        let demand_base_amount = &demand_pool - &(&cp / &(&offer_pool + &amount_out_optimal));

        // Calculate spread
        let mut spread = (&amount_out_optimal - &demand_base_amount) / &amount_out_optimal;
        if spread < min_swap_spread {
            spread = min_swap_spread;
        }

        let spread_fee = &spread * &amount_out_optimal;
        let amount_out_after_fee = &amount_out_optimal - &spread_fee;

        // Update pool delta & supplies
        if stablecoin_buy {
            pool_delta -= &amount_out_optimal;
        } else {
            pool_delta += &amount_in;
        }
        self.pool_delta().set(pool_delta);

        let mut user_payments: ManagedVec<EsdtTokenPayment<Self::Api>> = ManagedVec::new();
        let mut fee_payments: ManagedVec<EsdtTokenPayment<Self::Api>> = ManagedVec::new();

        if stablecoin_buy {
            self.collateral_token_supply()
                .update(|total| *total += amount_in.clone());
            user_payments.push(self.mint_stablecoins(amount_out_after_fee.clone()));
            fee_payments.push(EsdtTokenPayment::new(
                stablecoin_token_id.clone(),
                0,
                spread_fee.clone(),
            ));
        } else {
            self.collateral_token_supply()
                .update(|total| *total -= &amount_out_optimal.clone());
            self.burn_stablecoins(amount_in.clone());
            user_payments.push(EsdtTokenPayment::new(
                collateral_token_id.clone(),
                0,
                amount_out_after_fee.clone(),
            ));
            fee_payments.push(EsdtTokenPayment::new(
                collateral_token_id.clone(),
                0,
                spread_fee.clone(),
            ));
        }

        // Send tokens to caller
        self.send().direct_multi(&caller, &user_payments, &[]);

        // TODO - change address from owner to oracle SC
        // Send swap fees to oracle SC
        let owner = self.blockchain().get_owner_address();
        self.send().direct_multi(&owner, &fee_payments, &[]);

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

    // proxy

    #[proxy]
    fn aggregator_proxy(&self, sc_address: ManagedAddress) -> aggregator_proxy::Proxy<Self::Api>;

    fn get_exchange_rate(&self, from: &TokenIdentifier, to: &TokenIdentifier) -> BigUint {
        let price_aggregator_address = self.price_aggregator_address().get();

        let from_ticker = b"EGLD"; //self.token_ticker(from).get();
        let to_ticker = b"USD"; //self.token_ticker(to).get();

        let result: AggregatorResultAsMultiValue<Self::Api> = self
            .aggregator_proxy(price_aggregator_address)
            .latest_price_feed(from_ticker, to_ticker)
            .execute_on_dest_context();

        AggregatorResult::from(result).price
    }
}
