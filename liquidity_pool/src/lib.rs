#![no_std]

pub mod models;
pub use models::*;

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

mod debt_token;
mod stablecoin_token;

pub const BASE_PRECISION: u64 = 1_000_000_000;
pub const SECONDS_IN_YEAR: u64 = 31_556_926;

#[elrond_wasm::contract]
pub trait LiquidityPool:
    stablecoin_token::StablecoinTokenModule + debt_token::DebtTokenModule
{
    #[init]
    fn init(
        &self,
        asset: TokenIdentifier,
        borrow_rate: Self::BigUint,
        health_factor_threshold: u32,
    ) -> SCResult<()> {
        require!(borrow_rate < BASE_PRECISION, "Invalid borrow rate");
        require!(
            asset.is_egld() || asset.is_valid_esdt_identifier(),
            "Invalid asset"
        );

        self.pool_asset_id().set(&asset);
        self.borrow_rate().set(&borrow_rate);
        self.health_factor_threshold().set(&health_factor_threshold);

        Ok(())
    }

    #[payable("*")]
    #[endpoint]
    fn borrow(
        &self,
        #[payment_token] collateral_id: TokenIdentifier,
        #[payment] collateral_amount: Self::BigUint,
    ) -> SCResult<()> {
        self.require_debt_token_issued()?;
        self.require_stablecoin_issued()?;
        require!(
            collateral_id == self.pool_asset_id().get(),
            "Token not supported as collateral"
        );
        require!(collateral_amount > 0, "amount must be bigger then 0");

        let caller = self.blockchain().get_caller();

        // send debt position tokens
        // 1:1 ratio with collateral received

        let debt_nonce = self.create_and_send_debt(&caller, &collateral_amount);

        // send stablecoins to the user

        let stablecoin_amount =
            self.compute_stablecoin_amount_to_send(&collateral_id, &collateral_amount);

        self.mint_and_send_stablecoin(&caller, &stablecoin_amount);

        let current_health = self.compute_health_factor(&collateral_id);
        let debt_position = DebtPosition {
            health_factor: current_health,
            is_liquidated: false,
            collateral_timestamp: self.blockchain().get_block_timestamp(),
            collateral_amount,
            collateral_id,
        };
        self.debt_position(debt_nonce).set(&debt_position);

        Ok(())
    }

    #[payable("*")]
    #[endpoint(lockDebtTokens)]
    fn lock_debt_tokens(
        &self,
        #[payment_token] debt_token: TokenIdentifier,
        #[payment] amount: Self::BigUint,
    ) -> SCResult<u64> {
        self.require_debt_token_issued()?;
        self.require_stablecoin_issued()?;
        require!(amount > 0, "amount must be greater then 0");
        require!(
            debt_token == self.debt_token_id().get(),
            "debt token not supported by this pool"
        );

        let position_id = self.call_value().esdt_token_nonce();

        require!(
            !self.debt_position(position_id).is_empty(),
            "invalid debt position"
        );

        let debt_position = self.debt_position(position_id).get();
        require!(!debt_position.is_liquidated, "position is liquidated");

        let caller = self.blockchain().get_caller();

        if !self.repay_position(&caller, position_id).is_empty() {
            self.repay_position(&caller, position_id)
                .update(|repay_position| {
                    repay_position.collateral_amount_to_withdraw += amount;
                });
        } else {
            let repay_position = RepayPosition {
                collateral_amount_to_withdraw: amount,
                nft_nonce: position_id,
                debt_paid: Self::BigUint::zero(),
            };
            self.repay_position(&caller, position_id)
                .set(&repay_position);
        }

        Ok(position_id)
    }

    #[payable("*")]
    #[endpoint]
    fn repay(
        &self,
        position_id: u64,
        #[payment_token] token_id: TokenIdentifier,
        #[payment] amount: Self::BigUint,
    ) -> SCResult<()> {
        self.require_debt_token_issued()?;
        self.require_stablecoin_issued()?;

        let caller = self.blockchain().get_caller();

        require!(amount > 0, "amount must be greater then 0");
        require!(
            token_id == self.stablecoin_token_id().get(),
            "can only repay with stablecoin"
        );
        require!(
            !self.repay_position(&caller, position_id).is_empty(),
            "there are no locked debt tokens for this id"
        );
        require!(
            !self.debt_position(position_id).is_empty(),
            "invalid debt position id"
        );

        let repay_position = self.repay_position(&caller, position_id).get();
        let debt_position = self.debt_position(position_id).get();

        require!(!debt_position.is_liquidated, "position is liquidated");

        let total_owed = self.calculate_total_owed(
            &debt_position.collateral_id,
            &repay_position.collateral_amount_to_withdraw,
            debt_position.collateral_timestamp,
        );
        let total_debt_paid = &repay_position.debt_paid + &amount;

        if total_debt_paid < total_owed {
            self.repay_position(&caller, position_id)
                .update(|r| r.debt_paid = total_debt_paid);

            self.burn_stablecoin(&amount);
        } else {
            self.clear_after_full_repay(
                &caller,
                position_id,
                &debt_position.collateral_amount,
                &repay_position.collateral_amount_to_withdraw,
            );

            // Refund extra tokens paid
            let extra_payment = &total_debt_paid - &total_owed;
            if extra_payment > 0 {
                self.send_stablecoin(&caller, &extra_payment);
            }

            // Send repaid collateral back to the caller
            self.send().direct(
                &caller,
                &debt_position.collateral_id,
                0,
                &repay_position.collateral_amount_to_withdraw,
                &[],
            );

            // burn locked debt tokens
            // debt tokens are 1:1 with the collateral_amount_to_withdraw
            self.burn_debt(
                repay_position.nft_nonce,
                &repay_position.collateral_amount_to_withdraw,
            );

            // burn received stablecoins
            let amount_after_refund = amount - extra_payment;
            self.burn_stablecoin(&amount_after_refund);
        }

        Ok(())
    }

    #[payable("*")]
    #[endpoint]
    fn liquidate(
        &self,
        position_id: u64,
        #[payment_token] token_id: TokenIdentifier,
        #[payment] amount: Self::BigUint,
    ) -> SCResult<()> {
        require!(
            token_id == self.stablecoin_token_id().get(),
            "can only pay with stablecoins"
        );
        require!(
            !self.debt_position(position_id).is_empty(),
            "invalid debt position id"
        );

        let debt_position = self.debt_position(position_id).get();

        require!(
            !debt_position.is_liquidated,
            "position is already liquidated"
        );
        require!(
            debt_position.health_factor < self.health_factor_threshold().get(),
            "the health factor is not low enough"
        );

        let caller = self.blockchain().get_caller();
        let total_owed = self.calculate_total_owed(
            &debt_position.collateral_id,
            &debt_position.collateral_amount,
            debt_position.collateral_timestamp,
        );

        require!(
            amount >= total_owed,
            "position can't be liquidated, not enough tokens sent"
        );

        // Refund extra tokens paid
        let extra_payment = &amount - &total_owed;
        if extra_payment > 0 {
            self.send_stablecoin(&caller, &extra_payment);
        }

        self.debt_position(position_id)
            .update(|d| d.is_liquidated = true);

        // send collateral to liquidator
        self.send().direct(
            &caller,
            &debt_position.collateral_id,
            0,
            &debt_position.collateral_amount,
            &[],
        );

        // burn received stablecoins
        let amount_after_refund = amount - extra_payment;
        self.burn_stablecoin(&amount_after_refund);

        Ok(())
    }

    /// VIEWS

    #[view(getDebtInterest)]
    fn get_debt_interest(&self, value_in_dollars: &Self::BigUint, timestamp: u64) -> Self::BigUint {
        let now = self.blockchain().get_block_timestamp();
        let time_diff = Self::BigUint::from(now - timestamp);
        let borrow_rate = self.borrow_rate().get();

        self.compute_debt(value_in_dollars, &time_diff, &borrow_rate)
    }

    #[view(getTotalLockedPoolAsset)]
    fn get_total_locked_pool_asset(&self) -> Self::BigUint {
        let pool_asset_id = self.pool_asset_id().get();
        self.blockchain().get_sc_balance(&pool_asset_id, 0)
    }

    // UTILS

    fn compute_health_factor(&self, _collateral_id: &TokenIdentifier) -> u32 {
        0
    }

    /// Ratio of 1:1 for the purpose of mocking
    fn get_collateral_to_dollar_ratio(&self, _collateral_id: &TokenIdentifier) -> Self::BigUint {
        BASE_PRECISION.into()
    }

    fn compute_collateral_value_in_dollars(
        &self,
        collateral_id: &TokenIdentifier,
        collateral_amount: &Self::BigUint,
    ) -> Self::BigUint {
        let collateral_to_dollar_ratio = self.get_collateral_to_dollar_ratio(collateral_id);

        (collateral_amount * &collateral_to_dollar_ratio) / BASE_PRECISION.into()
    }

    fn compute_stablecoin_amount_to_send(
        &self,
        collateral_id: &TokenIdentifier,
        collateral_amount: &Self::BigUint,
    ) -> Self::BigUint {
        let borrow_rate = self.borrow_rate().get();
        let collateral_value_in_dollars =
            self.compute_collateral_value_in_dollars(collateral_id, collateral_amount);

        (collateral_value_in_dollars * borrow_rate) / BASE_PRECISION.into()
    }

    fn compute_debt(
        &self,
        amount: &Self::BigUint,
        time_diff: &Self::BigUint,
        borrow_rate: &Self::BigUint,
    ) -> Self::BigUint {
        let base_precision = Self::BigUint::from(BASE_PRECISION);
        let secs_year = Self::BigUint::from(SECONDS_IN_YEAR);
        let time_unit_percentage = (time_diff * &base_precision) / secs_year;
        let debt_percentage = &(&time_unit_percentage * borrow_rate) / &base_precision;

        (&debt_percentage * amount) / base_precision
    }

    fn calculate_total_owed(
        &self,
        collateral_id: &TokenIdentifier,
        collateral_amount: &Self::BigUint,
        collateral_timestamp: u64,
    ) -> Self::BigUint {
        let collateral_value_in_dollars =
            self.compute_collateral_value_in_dollars(collateral_id, collateral_amount);
        let debt_interest =
            self.get_debt_interest(&collateral_value_in_dollars, collateral_timestamp);

        collateral_value_in_dollars + debt_interest
    }

    fn clear_after_full_repay(
        &self,
        caller: &Address,
        position_id: u64,
        collateral_amount_full: &Self::BigUint,
        collateral_amount_withdrawed: &Self::BigUint,
    ) {
        self.repay_position(caller, position_id).clear();

        if collateral_amount_full == collateral_amount_withdrawed {
            self.debt_position(position_id).clear();
        } else {
            self.debt_position(position_id)
                .update(|d| d.collateral_amount -= collateral_amount_withdrawed);
        }
    }

    // storage

    #[view(getPoolAssetId)]
    #[storage_mapper("poolAssetId")]
    fn pool_asset_id(&self) -> SingleValueMapper<Self::Storage, TokenIdentifier>;

    #[storage_mapper("debtPosition")]
    fn debt_position(
        &self,
        id: u64,
    ) -> SingleValueMapper<Self::Storage, DebtPosition<Self::BigUint>>;

    #[storage_mapper("repayPosition")]
    fn repay_position(
        &self,
        caller_address: &Address,
        id: u64,
    ) -> SingleValueMapper<Self::Storage, RepayPosition<Self::BigUint>>;

    #[view(getHealthFactorThreshold)]
    #[storage_mapper("healthFactorThreshold")]
    fn health_factor_threshold(&self) -> SingleValueMapper<Self::Storage, u32>;

    // Borrow rate of (0.5 * BASE_PRECISION) means only 50% of the amount calculated is sent
    #[view(getBorrowRate)]
    #[storage_mapper("borrowRate")]
    fn borrow_rate(&self) -> SingleValueMapper<Self::Storage, Self::BigUint>;
}
