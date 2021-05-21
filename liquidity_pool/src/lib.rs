#![no_std]

pub mod models;
pub use models::*;

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

const STABLE_COIN_NAME: &[u8] = b"StableCoin";
const STABLE_COIN_TICKER: &[u8] = b"STCOIN";

const DEBT_TOKEN_NAME: &[u8] = b"DebtBearing";
const DEBT_TOKEN_TICKER: &[u8] = b"DEBT";

pub const BASE_PRECISION: u32 = 1_000_000_000;
pub const SECONDS_IN_YEAR: u32 = 31_556_926;

#[elrond_wasm_derive::contract(LiquidityPoolImpl)]
pub trait LiquidityPool {
    #[init]
    fn init(
        &self,
        asset: TokenIdentifier,
        borrow_rate: BigUint,
        health_factor_threshold: u32,
    ) -> SCResult<()> {
        require!(
            borrow_rate < BigUint::from(BASE_PRECISION),
            "Invalid borrow rate"
        );
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
        #[payment] collateral_amount: BigUint,
    ) -> SCResult<()> {
        sc_try!(self.require_debt_token_issued());
        sc_try!(self.require_stablecoin_issued());

        let debt_token_id = self.debt_token_id().get();

        require!(
            collateral_id == self.pool_asset_id().get(),
            "Token not supported as collateral"
        );
        require!(collateral_amount > 0, "amount must be bigger then 0");

        self.mint_debt(&collateral_amount);

        let caller = self.blockchain().get_caller();
        let nft_nonce = self.blockchain().get_current_esdt_nft_nonce(
            &self.blockchain().get_sc_address(),
            debt_token_id.as_esdt_identifier(),
        );

        // send debt position tokens
        // 1:1 ratio with collateral received

        match self.send().direct_esdt_nft_via_transfer_exec(
            &caller,
            debt_token_id.as_esdt_identifier(),
            nft_nonce,
            &collateral_amount,
            &[],
        ) {
            Result::Ok(()) => {}
            Result::Err(_) => {
                return sc_error!("Failed to send debt tokens");
            }
        };

        // send stablecoins to the user

        let stablecoin_token_id = self.stablecoin_token_id().get();
        let stablecoin_amount =
            self.compute_stablecoin_amount_to_send(&collateral_id, &collateral_amount);

        self.mint_stablecoin(&stablecoin_amount);
        self.send()
            .direct(&caller, &stablecoin_token_id, &stablecoin_amount, &[]);

        self.total_circulating_supply()
            .update(|total| *total += &stablecoin_amount);

        let current_health = self.compute_health_factor(&collateral_id);
        let debt_position = DebtPosition {
            health_factor: current_health,
            is_liquidated: false,
            collateral_timestamp: self.blockchain().get_block_timestamp(),
            collateral_amount,
            collateral_id,
        };
        self.debt_position(nft_nonce).set(&debt_position);

        Ok(())
    }

    #[payable("*")]
    #[endpoint(lockDebtTokens)]
    fn lock_debt_tokens(
        &self,
        #[payment_token] debt_token: TokenIdentifier,
        #[payment] amount: BigUint,
    ) -> SCResult<u64> {
        sc_try!(self.require_debt_token_issued());
        sc_try!(self.require_stablecoin_issued());
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
                debt_paid: BigUint::zero(),
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
        #[payment] amount: BigUint,
    ) -> SCResult<()> {
        sc_try!(self.require_debt_token_issued());
        sc_try!(self.require_stablecoin_issued());

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

        let collateral_value_in_dollars = self.compute_collateral_value_in_dollars(
            &debt_position.collateral_id,
            &repay_position.collateral_amount_to_withdraw,
        );
        let debt_interest = self.get_debt_interest(
            &collateral_value_in_dollars,
            debt_position.collateral_timestamp,
        );
        let total_owed = collateral_value_in_dollars + debt_interest;
        let total_debt_paid = &repay_position.debt_paid + &amount;

        if total_debt_paid < total_owed {
            self.repay_position(&caller, position_id)
                .update(|r| r.debt_paid = total_debt_paid);
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
                match self.send().direct_esdt_via_transf_exec(
                    &caller,
                    token_id.as_esdt_identifier(),
                    &extra_payment,
                    &[],
                ) {
                    Result::Ok(()) => {}
                    Result::Err(_) => return sc_error!("Failed refunding extra tokens"),
                };
            }

            // Send repaid collateral back to the caller
            self.send().direct(
                &caller,
                &debt_position.collateral_id,
                &repay_position.collateral_amount_to_withdraw,
                &[],
            );

            // burn locked debt tokens
            // debt tokens are 1:1 with the collateral_amount_to_withdraw
            self.burn(
                &repay_position.collateral_amount_to_withdraw,
                repay_position.nft_nonce,
                &self.debt_token_id().get(),
            );

            // burn received stablecoins
            self.burn_stablecoin(&amount);
        }

        // decrease circulating supply
        self.total_circulating_supply()
            .update(|circulating_supply| *circulating_supply -= &amount);

        Ok(())
    }

    #[payable("*")]
    #[endpoint]
    fn liquidate(
        &self,
        position_id: u64,
        #[payment_token] token_id: TokenIdentifier,
        #[payment] payment_amount: BigUint,
    ) -> SCResult<()> {
        require!(payment_amount > 0, "amount must be bigger then 0");
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
        let debt_interest = self.get_debt_interest(
            &debt_position.collateral_amount,
            debt_position.collateral_timestamp,
        );
        let total_owed = &debt_position.collateral_amount + &debt_interest;

        require!(
            payment_amount >= total_owed,
            "position can't be liquidated, not enough tokens sent"
        );

        // Refund extra tokens paid
        let extra_payment = &payment_amount - &total_owed;
        if extra_payment > 0 {
            match self.send().direct_esdt_via_transf_exec(
                &caller,
                token_id.as_esdt_identifier(),
                &extra_payment,
                &[],
            ) {
                Result::Ok(()) => {}
                Result::Err(_) => return sc_error!("Failed refunding extra tokens"),
            };
        }

        self.debt_position(position_id)
            .update(|d| d.is_liquidated = true);

        // decrease circulating supply
        self.total_circulating_supply()
            .update(|circulating_supply| *circulating_supply -= &payment_amount);

        Ok(())
    }

    #[payable("EGLD")]
    #[endpoint(issueStablecoinToken)]
    fn issue_stablecoin_token(
        &self,
        #[payment] issue_cost: BigUint,
    ) -> SCResult<AsyncCall<BigUint>> {
        only_owner!(self, "only owner can issue new tokens");
        require!(
            self.stablecoin_token_id().is_empty(),
            "Stablecoin already issued"
        );

        let token_display_name = BoxedBytes::from(STABLE_COIN_NAME);
        let token_ticker = BoxedBytes::from(STABLE_COIN_TICKER);
        let initial_supply = BigUint::from(1u32);

        Ok(ESDTSystemSmartContractProxy::new()
            .issue_fungible(
                issue_cost,
                &token_display_name,
                &token_ticker,
                &initial_supply,
                FungibleTokenProperties {
                    can_burn: true,
                    can_mint: true,
                    num_decimals: 0,
                    can_freeze: true,
                    can_wipe: true,
                    can_pause: true,
                    can_change_owner: true,
                    can_upgrade: true,
                    can_add_special_roles: true,
                },
            )
            .async_call()
            .with_callback(self.callbacks().issue_callback(token_ticker)))
    }

    #[payable("EGLD")]
    #[endpoint(issueDebtToken)]
    fn issue_debt_token(&self, #[payment] issue_cost: BigUint) -> SCResult<AsyncCall<BigUint>> {
        only_owner!(self, "only owner can issue new tokens");
        require!(self.debt_token_id().is_empty(), "Debt token already issued");

        let token_display_name = BoxedBytes::from(DEBT_TOKEN_NAME);
        let token_ticker = BoxedBytes::from(DEBT_TOKEN_TICKER);

        Ok(ESDTSystemSmartContractProxy::new()
            .issue_semi_fungible(
                issue_cost,
                &token_display_name,
                &token_ticker,
                SemiFungibleTokenProperties {
                    can_freeze: true,
                    can_wipe: true,
                    can_pause: true,
                    can_change_owner: true,
                    can_upgrade: true,
                    can_add_special_roles: true,
                },
            )
            .async_call()
            .with_callback(self.callbacks().issue_callback(token_ticker)))
    }

    fn set_roles(&self, token_id: TokenIdentifier, roles: &[EsdtLocalRole]) -> AsyncCall<BigUint> {
        ESDTSystemSmartContractProxy::new()
            .set_special_roles(
                &self.blockchain().get_sc_address(),
                token_id.as_esdt_identifier(),
                roles,
            )
            .async_call()
    }

    #[callback]
    fn issue_callback(
        &self,
        token_ticker: BoxedBytes,
        #[call_result] result: AsyncCallResult<TokenIdentifier>,
    ) -> OptionalResult<AsyncCall<BigUint>> {
        match result {
            AsyncCallResult::Ok(token_id) => {
                let mut roles = Vec::new();

                if token_ticker == BoxedBytes::from(STABLE_COIN_TICKER) {
                    self.stablecoin_token_id().set(&token_id);

                    roles.push(EsdtLocalRole::Mint);
                    roles.push(EsdtLocalRole::Burn);
                } else if token_ticker == BoxedBytes::from(DEBT_TOKEN_TICKER) {
                    self.debt_token_id().set(&token_id);

                    roles.push(EsdtLocalRole::NftAddQuantity);
                    roles.push(EsdtLocalRole::NftBurn);
                    roles.push(EsdtLocalRole::NftCreate);
                }

                if roles.is_empty() {
                    OptionalResult::None
                } else {
                    OptionalResult::Some(self.set_roles(token_id, roles.as_slice()))
                }
            }
            AsyncCallResult::Err(_) => {
                let caller = self.blockchain().get_owner_address();
                let (returned_tokens, token_id) = self.call_value().payment_token_pair();
                if token_id.is_egld() && returned_tokens > 0 {
                    self.send().direct_egld(&caller, &returned_tokens, &[]);
                }

                OptionalResult::None
            }
        }
    }

    fn mint_debt(&self, amount: &BigUint) {
        self.send().esdt_nft_create::<()>(
            self.blockchain().get_gas_left(),
            self.debt_token_id().get().as_esdt_identifier(),
            &amount,
            &BoxedBytes::empty(),
            &BigUint::zero(),
            &H256::zero(),
            &(),
            &[BoxedBytes::empty()],
        );
    }

    fn burn(&self, amount: &BigUint, nonce: u64, token_id: &TokenIdentifier) {
        self.send().esdt_nft_burn(
            self.blockchain().get_gas_left(),
            token_id.as_esdt_identifier(),
            nonce,
            &amount,
        );
    }

    fn mint_stablecoin(&self, amount: &BigUint) {
        self.send().esdt_local_mint(
            self.blockchain().get_gas_left(),
            self.stablecoin_token_id().get().as_esdt_identifier(),
            amount,
        );
    }

    fn burn_stablecoin(&self, amount: &BigUint) {
        self.send().esdt_local_burn(
            self.blockchain().get_gas_left(),
            self.stablecoin_token_id().get().as_esdt_identifier(),
            amount,
        );
    }

    /// VIEWS

    #[view(getDebtInterest)]
    fn get_debt_interest(&self, amount: &BigUint, timestamp: u64) -> BigUint {
        let now = self.blockchain().get_block_timestamp();
        let time_diff = BigUint::from(now - timestamp);
        let borrow_rate = self.borrow_rate().get();

        self.compute_debt(amount, &time_diff, &borrow_rate)
    }

    #[view(getTotalLockedPoolAsset)]
    fn get_total_locked_pool_asset(&self) -> BigUint {
        let pool_asset_id = self.pool_asset_id().get();

        if pool_asset_id.is_egld() {
            self.blockchain().get_sc_balance()
        } else {
            self.blockchain().get_esdt_balance(
                &self.blockchain().get_sc_address(),
                pool_asset_id.as_esdt_identifier(),
                0,
            )
        }
    }

    // UTILS

    fn require_debt_token_issued(&self) -> SCResult<()> {
        if self.debt_token_id().is_empty() {
            sc_error!("Debt token must be issued first")
        } else {
            Ok(())
        }
    }

    fn require_stablecoin_issued(&self) -> SCResult<()> {
        if self.stablecoin_token_id().is_empty() {
            sc_error!("Stablecoin token must be issued first")
        } else {
            Ok(())
        }
    }

    fn compute_health_factor(&self, _collateral_id: &TokenIdentifier) -> u32 {
        0
    }

    /// Ratio of 1:1 for the purpose of mocking
    fn get_collateral_to_dollar_ratio(&self, _collateral_id: &TokenIdentifier) -> BigUint {
        BigUint::from(BASE_PRECISION)
    }

    fn compute_collateral_value_in_dollars(
        &self,
        collateral_id: &TokenIdentifier,
        collateral_amount: &BigUint,
    ) -> BigUint {
        let collateral_to_dollar_ratio = self.get_collateral_to_dollar_ratio(collateral_id);
        let base_precision = BigUint::from(BASE_PRECISION);

        (collateral_amount * &collateral_to_dollar_ratio) / base_precision
    }

    fn compute_stablecoin_amount_to_send(
        &self,
        collateral_id: &TokenIdentifier,
        collateral_amount: &BigUint,
    ) -> BigUint {
        let borrow_rate = self.borrow_rate().get();
        let collateral_value_in_dollars =
            self.compute_collateral_value_in_dollars(collateral_id, collateral_amount);
        let base_precision = BigUint::from(BASE_PRECISION);

        (collateral_value_in_dollars * borrow_rate) / base_precision
    }

    fn compute_debt(
        &self,
        amount: &BigUint,
        time_diff: &BigUint,
        borrow_rate: &BigUint,
    ) -> BigUint {
        let base_precision = BigUint::from(BASE_PRECISION);
        let secs_year = BigUint::from(SECONDS_IN_YEAR);
        let time_unit_percentage = (time_diff * &base_precision) / secs_year;

        let debt_percentage = (&time_unit_percentage * borrow_rate) / base_precision.clone();

        if debt_percentage <= base_precision {
            let amount_diff =
                ((&base_precision - &debt_percentage) * amount.clone()) / base_precision;

            amount - &amount_diff
        } else {
            (&debt_percentage * amount) / base_precision
        }
    }

    fn clear_after_full_repay(
        &self,
        caller: &Address,
        position_id: u64,
        collateral_amount_full: &BigUint,
        collateral_amount_withdrawed: &BigUint,
    ) {
        self.repay_position(&caller, position_id).clear();

        if collateral_amount_full == collateral_amount_withdrawed {
            self.debt_position(position_id).clear();
        } else {
            self.debt_position(position_id)
                .update(|d| d.collateral_amount -= collateral_amount_withdrawed);
        }
    }

    #[view(getStablecoinTokenId)]
    #[storage_mapper("stablecoinTokenId")]
    fn stablecoin_token_id(&self) -> SingleValueMapper<Self::Storage, TokenIdentifier>;

    #[view(getPoolAssetId)]
    #[storage_mapper("poolAssetId")]
    fn pool_asset_id(&self) -> SingleValueMapper<Self::Storage, TokenIdentifier>;

    #[view(getDebtTokenId)]
    #[storage_mapper("debtTokenId")]
    fn debt_token_id(&self) -> SingleValueMapper<Self::Storage, TokenIdentifier>;

    #[storage_mapper("debtPosition")]
    fn debt_position(&self, id: u64) -> SingleValueMapper<Self::Storage, DebtPosition<BigUint>>;

    #[storage_mapper("repayPosition")]
    fn repay_position(
        &self,
        caller_address: &Address,
        id: u64,
    ) -> SingleValueMapper<Self::Storage, RepayPosition<BigUint>>;

    #[view(getHealthFactorThreshold)]
    #[storage_mapper("healthFactorThreshold")]
    fn health_factor_threshold(&self) -> SingleValueMapper<Self::Storage, u32>;

    #[view(getTotalCirculatingSupply)]
    #[storage_mapper("totalCirculatingSupply")]
    fn total_circulating_supply(&self) -> SingleValueMapper<Self::Storage, BigUint>;
    
    // Borrow rate of (0.5 * BASE_PRECISION) means only 50% of the amount calculated is sent
    #[view(getBorrowRate)]
    #[storage_mapper("borrowRate")]
    fn borrow_rate(&self) -> SingleValueMapper<Self::Storage, BigUint>;
}
