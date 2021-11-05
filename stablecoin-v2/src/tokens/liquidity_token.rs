elrond_wasm::imports!();

const LIQUIDITY_TOKEN_NAME: &[u8] = b"LiquidityToken";
const LIQUIDITY_TOKEN_TICKER: &[u8] = b"LIQ";

#[elrond_wasm::module]
pub trait LiquidityTokenModule:
    crate::math::MathModule + crate::token_common::TokenCommonModule
{
    #[only_owner]
    #[payable("EGLD")]
    #[endpoint(issueLiquidityToken)]
    fn issue_liquidity_token(&self, #[payment] issue_cost: BigUint) -> SCResult<AsyncCall> {
        require!(
            self.liquidity_token_id().is_empty(),
            "Liquidity token already issued"
        );

        let token_display_name = ManagedBuffer::from(LIQUIDITY_TOKEN_NAME);
        let token_ticker = ManagedBuffer::from(LIQUIDITY_TOKEN_TICKER);

        Ok(self
            .send()
            .esdt_system_sc_proxy()
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
            .with_callback(self.callbacks().liquidity_token_issue_callback()))
    }

    #[only_owner]
    #[endpoint(setLiquidityTokenRoles)]
    fn set_liquidity_token_roles(&self) -> AsyncCall {
        let token_id = self.liquidity_token_id().get();
        let roles = [
            EsdtLocalRole::NftCreate,
            EsdtLocalRole::NftAddQuantity,
            EsdtLocalRole::NftBurn,
        ];

        self.set_local_roles(&token_id, &roles)
    }

    fn create_or_mint_liq_tokens(&self, collateral_id: &TokenIdentifier, amount: &BigUint) -> u64 {
        let token_id = self.liquidity_token_id().get();

        let existing_sft_nonce = self.liq_sft_nonce_for_collateral(collateral_id).get();
        if existing_sft_nonce > 0 {
            self.send()
                .esdt_local_mint(&token_id, existing_sft_nonce, amount);

            return existing_sft_nonce;
        }

        // must keep at least 1 in SC's balance for NFTAddQuantity
        // ESDT metadata is deleted if the balance is 0
        let amount_plus_leftover = amount + 1u32;
        let new_sft_nonce = self.create_nft(&token_id, &amount_plus_leftover);
        self.liq_sft_nonce_for_collateral(collateral_id)
            .set(&new_sft_nonce);
        self.collateral_for_liq_sft_nonce(new_sft_nonce)
            .set(collateral_id);

        new_sft_nonce
    }

    fn send_liq_tokens(&self, to: &ManagedAddress, sft_nonce: u64, amount: &BigUint) {
        self.liq_token_amount_in_circulation(sft_nonce)
            .update(|amount_in_circulation| *amount_in_circulation += amount);

        let token_id = self.liquidity_token_id().get();
        self.send().direct(to, &token_id, sft_nonce, amount, &[]);
    }

    fn create_and_send_liq_tokens(
        &self,
        to: &ManagedAddress,
        collateral_id: &TokenIdentifier,
        amount: &BigUint,
    ) {
        let sft_nonce = self.create_or_mint_liq_tokens(collateral_id, amount);
        self.send_liq_tokens(to, sft_nonce, amount);
    }

    fn burn_liq_tokens(&self, sft_nonce: u64, amount: &BigUint) {
        self.liq_token_amount_in_circulation(sft_nonce)
            .update(|amount_in_circulation| *amount_in_circulation -= amount);

        let token_id = self.liquidity_token_id().get();
        self.send().esdt_local_burn(&token_id, sft_nonce, amount);
    }

    // if no liq tokens exist, the value is 1:1
    fn get_liq_token_value_in_collateral(
        &self,
        collateral_id: &TokenIdentifier,
        collateral_precision: &BigUint,
    ) -> BigUint {
        let sft_nonce = self.liq_sft_nonce_for_collateral(collateral_id).get();
        if sft_nonce == 0 {
            return collateral_precision.clone();
        }

        let liq_tokens_amount = self.liq_token_amount_in_circulation(sft_nonce).get();
        if liq_tokens_amount == 0 {
            return collateral_precision.clone();
        }

        let collateral_amount = self.collateral_amount_for_liq_token(sft_nonce).get();
        self.divide(&collateral_amount, &liq_tokens_amount, collateral_precision)
    }

    fn collateral_to_liq_tokens(
        &self,
        collateral_id: &TokenIdentifier,
        collateral_amount: &BigUint,
        collateral_precision: &BigUint,
    ) -> BigUint {
        let liq_token_value_in_collateral =
            self.get_liq_token_value_in_collateral(collateral_id, collateral_precision);
        self.divide(
            collateral_amount,
            &liq_token_value_in_collateral,
            collateral_precision,
        )
    }

    fn liq_tokens_to_collateral(
        &self,
        collateral_id: &TokenIdentifier,
        liq_token_amount: &BigUint,
        collateral_precision: &BigUint,
    ) -> BigUint {
        let liq_token_value_in_collateral =
            self.get_liq_token_value_in_collateral(collateral_id, collateral_precision);
        self.multiply(
            liq_token_amount,
            &liq_token_value_in_collateral,
            collateral_precision,
        )
    }

    #[callback]
    fn liquidity_token_issue_callback(
        &self,
        #[call_result] result: ManagedAsyncCallResult<TokenIdentifier>,
    ) -> OptionalResult<AsyncCall> {
        match result {
            ManagedAsyncCallResult::Ok(token_id) => {
                self.liquidity_token_id().set(&token_id);

                OptionalResult::Some(self.set_liquidity_token_roles())
            }
            ManagedAsyncCallResult::Err(_) => {
                self.refund_owner_failed_issue();

                OptionalResult::None
            }
        }
    }

    // storage

    #[view(getLiquidityTokenId)]
    #[storage_mapper("liquidityTokenId")]
    fn liquidity_token_id(&self) -> SingleValueMapper<TokenIdentifier>;

    #[view(getLiquidityTokenSftNonceForCollateral)]
    #[storage_mapper("liqSftNonceForCollateral")]
    fn liq_sft_nonce_for_collateral(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SingleValueMapper<u64>;

    #[view(getCollateralForLiquidityTokenSftNonce)]
    #[storage_mapper("collateralForLiqSftNonce")]
    fn collateral_for_liq_sft_nonce(&self, sft_nonce: u64) -> SingleValueMapper<TokenIdentifier>;

    #[storage_mapper("liqTokenAmountInCirculation")]
    fn liq_token_amount_in_circulation(&self, sft_nonce: u64) -> SingleValueMapper<BigUint>;

    #[storage_mapper("collateralAmountForLiqToken")]
    fn collateral_amount_for_liq_token(&self, sft_nonce: u64) -> SingleValueMapper<BigUint>;
}
