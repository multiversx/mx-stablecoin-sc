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
    fn init(&self, asset: TokenIdentifier, borrow_rate: BigUint) -> SCResult<()> {
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
        self.debt_nonce().set(&1u64);

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

        let debt_token_id = self.debt_token_id().get();

        require!(
            collateral_id == self.pool_asset_id().get(),
            "Token not supported as collateral"
        );
        require!(collateral_amount > 0, "amount must be bigger then 0");

        let position_id = self.get_nft_hash();
        let debt_metadata = DebtMetadata {
            collateral_id: collateral_id.clone(),
            collateral_timestamp: self.blockchain().get_block_timestamp(),
        };

        self.mint_debt(&collateral_amount, &debt_metadata, &position_id);

        let caller = self.blockchain().get_caller();
        let nonce = self.blockchain().get_current_esdt_nft_nonce(
            &self.blockchain().get_sc_address(),
            debt_token_id.as_esdt_identifier(),
        );

        // send debt position tokens
        // 1:1 ratio with collateral received

        match self.send().direct_esdt_nft_via_transfer_exec(
            &caller,
            debt_token_id.as_esdt_identifier(),
            nonce,
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

        self.total_borrow()
            .update(|total| *total += &stablecoin_amount);

        let current_health = self.compute_health_factor();
        let debt_position = DebtPosition {
            initial_amount: collateral_amount.clone(),
            health_factor: current_health,
            is_liquidated: false,
            collateral_timestamp: debt_metadata.collateral_timestamp,
            collateral_amount,
            collateral_id,
        };
        self.debt_position(&position_id).set(&debt_position);

        Ok(())
    }

    /*
    #[payable("*")]
    #[endpoint(lockDebtTokens)]
    fn lock_debt_tokens(
        &self,
        #[payment_token] debt_token: TokenIdentifier,
        #[payment] amount: BigUint,
    ) -> SCResult<H256> {
        require!(amount > 0, "amount must be greater then 0");
        require!(
            debt_token == self.debt_token_id().get(),
            "debt token not supported by this pool"
        );

        let nft_nonce = self.call_value().esdt_token_nonce();
        let esdt_nft_data = self.blockchain().get_esdt_token_data(
            &self.blockchain().get_sc_address(),
            debt_token.as_esdt_identifier(),
            nft_nonce,
        );

        let debt_position_id = &esdt_nft_data.hash;
        let debt_position = match self.debt_positions().get(debt_position_id) {
            Some(pos) => pos,
            None => return sc_error!("invalid debt position"),
        };

        require!(!debt_position.is_liquidated, "position is liquidated");

        let metadata = match esdt_nft_data.decode_attributes::<DebtMetadata>() {
            Result::Ok(decoded) => decoded,
            Result::Err(_) => {
                return sc_error!("could not parse token metadata");
            }
        };
        let data = [
            debt_token.as_esdt_identifier(),
            amount.to_bytes_be().as_slice(),
            &nft_nonce.to_be_bytes()[..],
        ]
        .concat();

        let unique_repay_id = self.crypto().keccak256(&data);
        let repay_position = RepayPostion {
            identifier: debt_token,
            amount,
            nonce: nft_nonce,
            collateral_id: metadata.collateral_id,
            collateral_amount: metadata.collateral_amount,
            collateral_timestamp: metadata.collateral_timestamp,
        };
        self.repay_position()
            .insert(unique_repay_id.clone(), repay_position);

        Ok(unique_repay_id)
    }
    */

    #[payable("*")]
    #[endpoint(repay)]
    fn repay(
        &self,
        unique_id: H256,
        #[payment_token] asset: TokenIdentifier,
        #[payment] amount: BigUint,
    ) -> SCResult<RepayPostion<BigUint>> {
        require!(amount > 0, "amount must be greater then 0");
        require!(
            asset == self.pool_asset_id().get(),
            "asset is not supported by this pool"
        );
        require!(
            self.repay_position().contains_key(&unique_id),
            "there are no locked borrowed token for this id, lock b tokens first"
        );

        let mut repay_position = self.repay_position().get(&unique_id).unwrap_or_default();

        require!(
            repay_position.amount >= amount,
            "b tokens amount locked must be equal with the amount of asset token send"
        );

        let esdt_nft_data = self.blockchain().get_esdt_token_data(
            &self.blockchain().get_sc_address(),
            repay_position.identifier.as_esdt_identifier(),
            repay_position.nonce,
        );

        let debt_position_id = esdt_nft_data.hash;

        require!(
            !self.debt_position(&debt_position_id).is_empty(),
            "invalid debt position id"
        );
        let debt_position = self.debt_position(&debt_position_id).get();

        require!(!debt_position.is_liquidated, "position is liquidated");

        let interest =
            self.get_debt_interest(&repay_position.amount, repay_position.collateral_timestamp);

        if repay_position.amount.clone() + interest == amount {
            self.repay_position().remove(&unique_id);
        } else if repay_position.amount > amount {
            repay_position.amount -= amount.clone();
            self.repay_position()
                .insert(unique_id, repay_position.clone());
        }

        self.burn(&amount, repay_position.nonce, &repay_position.identifier);

        repay_position.amount = amount;

        Ok(repay_position)
    }

    /*
    // Might remove or merge with the repay function
    #[payable("*")]
    #[endpoint]
    fn withdraw(
        &self,
        #[payment_token] lend_token: TokenIdentifier,
        #[payment] amount: BigUint,
    ) -> SCResult<()> {
        let caller = self.blockchain().get_caller();

        /*require!(
            lend_token == self.get_lend_token(),
            "lend token is not supported by this pool"
        );*/
        require!(amount > 0, "amount must be bigger then 0");

        let pool_asset = self.pool_asset_id().get();
        let mut asset_reserve = self
            .reserves()
            .get(&pool_asset)
            .unwrap_or_else(BigUint::zero);

        require!(asset_reserve != BigUint::zero(), "asset reserve is empty");

        let nonce = self.call_value().esdt_token_nonce();
        self.burn(&amount, nonce, &lend_token);

        self.send().direct(&caller, &pool_asset, &amount, &[]);

        asset_reserve -= amount;
        self.reserves().insert(pool_asset, asset_reserve);

        Ok(())
    }
    */

    #[payable("*")]
    #[endpoint(liquidate)]
    fn liquidate(
        &self,
        position_id: H256,
        #[payment_token] token: TokenIdentifier,
        #[payment] amount: BigUint,
    ) -> SCResult<LiquidateData<BigUint>> {
        require!(amount > 0, "amount must be bigger then 0");
        require!(
            token == self.pool_asset_id().get(),
            "asset is not supported by this pool"
        );
        require!(
            !self.debt_position(&position_id).is_empty(),
            "invalid debt position id"
        );

        let mut debt_position = self.debt_position(&position_id).get();

        require!(
            !debt_position.is_liquidated,
            "position is already liquidated"
        );
        require!(
            debt_position.health_factor < self.get_health_factor_threshold(),
            "the health factor is not low enough"
        );

        let interest = self.get_debt_interest(
            &debt_position.initial_amount,
            debt_position.collateral_timestamp,
        );

        require!(
            debt_position.initial_amount.clone() + interest == amount,
            "position can't be liquidated, not enough or to much tokens send"
        );

        debt_position.is_liquidated = true;

        self.debt_position(&position_id).set(&debt_position);

        let liquidate_data = LiquidateData {
            collateral_token: debt_position.collateral_id,
            amount,
        };

        Ok(liquidate_data)
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

    #[endpoint(setStablecoinRoles)]
    fn set_stablecoin_roles(
        &self,
        #[var_args] roles: VarArgs<EsdtLocalRole>,
    ) -> SCResult<AsyncCall<BigUint>> {
        only_owner!(self, "only owner can set roles");
        sc_try!(self.require_stablecoin_issued());

        Ok(self.set_roles(self.stablecoin_token_id().get(), roles.as_slice()))
    }

    #[endpoint(setDebtTokenRoles)]
    fn set_debt_token_roles(
        &self,
        #[var_args] roles: VarArgs<EsdtLocalRole>,
    ) -> SCResult<AsyncCall<BigUint>> {
        only_owner!(self, "only owner can set roles");
        sc_try!(self.require_debt_token_issued());

        Ok(self.set_roles(self.debt_token_id().get(), roles.as_slice()))
    }

    fn issue(
        &self,
        token_display_name: BoxedBytes,
        token_ticker: BoxedBytes,
        issue_cost: BigUint,
    ) -> SCResult<AsyncCall<BigUint>> {
        only_owner!(self, "only owner can issue new tokens");

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

    fn set_roles(&self, token: TokenIdentifier, roles: &[EsdtLocalRole]) -> AsyncCall<BigUint> {
        ESDTSystemSmartContractProxy::new()
            .set_special_roles(
                &self.blockchain().get_sc_address(),
                token.as_esdt_identifier(),
                roles,
            )
            .async_call()
    }

    #[callback]
    fn issue_callback(
        &self,
        token_ticker: BoxedBytes,
        #[payment_token] token_id: TokenIdentifier,
        #[payment] returned_tokens: BigUint,
        #[call_result] result: AsyncCallResult<()>,
    ) {
        match result {
            AsyncCallResult::Ok(()) => {
                if token_ticker == BoxedBytes::from(STABLE_COIN_TICKER) {
                    self.stablecoin_token_id().set(&token_id);
                } else if token_ticker == BoxedBytes::from(DEBT_TOKEN_TICKER) {
                    self.debt_token_id().set(&token_id);
                }
            }
            AsyncCallResult::Err(_) => {
                let caller = self.blockchain().get_owner_address();
                if token_id.is_egld() && returned_tokens > 0 {
                    self.send().direct_egld(&caller, &returned_tokens, &[]);
                }
            }
        }
    }

    fn mint_debt(&self, amount: &BigUint, metadata: &DebtMetadata, position_id: &H256) {
        self.send().esdt_nft_create::<DebtMetadata>(
            self.blockchain().get_gas_left(),
            self.debt_token_id().get().as_esdt_identifier(),
            &amount,
            &BoxedBytes::empty(),
            &BigUint::zero(),
            &position_id,
            &metadata,
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

    fn get_nft_hash(&self) -> H256 {
        let debt_nonce = self.debt_nonce().get();
        let hash = self.crypto().keccak256(&debt_nonce.to_be_bytes()[..]);
        self.debt_nonce().set(&(debt_nonce + 1));
        hash
    }

    fn compute_health_factor(&self) -> u32 {
        0
    }

    fn get_health_factor_threshold(&self) -> u32 {
        0
    }

    /// Ratio of 1:1 for the purpose of mocking
    fn get_collateral_to_dollar_ratio(&self, _collateral_id: &TokenIdentifier) -> BigUint {
        BigUint::from(BASE_PRECISION)
    }

    fn compute_stablecoin_amount_to_send(
        &self,
        collateral_id: &TokenIdentifier,
        collateral_amount: &BigUint,
    ) -> BigUint {
        let borrow_rate = self.borrow_rate().get();
        let collateral_to_dollar_ratio = self.get_collateral_to_dollar_ratio(collateral_id);
        let base_precision = BigUint::from(BASE_PRECISION);

        let raw_amount = &(collateral_amount * &collateral_to_dollar_ratio) / &base_precision;

        (raw_amount * borrow_rate) / base_precision
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

    //
    /// stablecoin token id
    #[view(getStablecoinTokenId)]
    #[storage_mapper("stablecoinTokenId")]
    fn stablecoin_token_id(&self) -> SingleValueMapper<Self::Storage, TokenIdentifier>;

    //
    /// pool asset
    #[view(getPoolAssetId)]
    #[storage_mapper("poolAssetId")]
    fn pool_asset_id(&self) -> SingleValueMapper<Self::Storage, TokenIdentifier>;

    //
    /// debt token supported for collateral
    #[view(getDebtTokenId)]
    #[storage_mapper("debtTokenId")]
    fn debt_token_id(&self) -> SingleValueMapper<Self::Storage, TokenIdentifier>;

    //
    /// debt positions
    #[storage_mapper("debtPosition")]
    fn debt_position(&self, id: &H256) -> SingleValueMapper<Self::Storage, DebtPosition<BigUint>>;

    //
    /// debt nonce
    #[storage_mapper("debtNonce")]
    fn debt_nonce(&self) -> SingleValueMapper<Self::Storage, u64>;

    //
    /// repay position
    #[storage_mapper("repayPosition")]
    fn repay_position(&self) -> MapMapper<Self::Storage, H256, RepayPostion<BigUint>>;

    //
    /// health factor threshold
    #[view(getHealthFactorThreshold)]
    #[storage_mapper("healthFactorThreshold")]
    fn health_factor_threshold(&self) -> SingleValueMapper<Self::Storage, u32>;

    //
    // total borrowing from pool
    #[view(getTotalBorrow)]
    #[storage_mapper("totalBorrow")]
    fn total_borrow(&self) -> SingleValueMapper<Self::Storage, BigUint>;

    //
    // Borrow rate of (0.5 * BASE_PRECISION) means only 50% of the amount calculated is sent
    #[view(getBorrowRate)]
    #[storage_mapper("borrowRate")]
    fn borrow_rate(&self) -> SingleValueMapper<Self::Storage, BigUint>;
}
