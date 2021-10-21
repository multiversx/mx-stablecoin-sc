elrond_wasm::imports!();

const HEDGING_TOKEN_NAME: &[u8] = b"HedgingToken";
const HEDGING_TOKEN_TICKER: &[u8] = b"HEDGE";
pub const NFT_AMOUNT: u32 = 1;

#[elrond_wasm::module]
pub trait HedgingTokenModule: crate::token_common::TokenCommonModule {
    #[only_owner]
    #[payable("EGLD")]
    #[endpoint(issueHedgingToken)]
    fn issue_hedging_token(&self, #[payment] issue_cost: BigUint) -> SCResult<AsyncCall> {
        require!(
            self.hedging_token_id().is_empty(),
            "Hedging token already issued"
        );

        let token_display_name = ManagedBuffer::from(HEDGING_TOKEN_NAME);
        let token_ticker = ManagedBuffer::from(HEDGING_TOKEN_TICKER);

        Ok(self
            .send()
            .esdt_system_sc_proxy()
            .issue_non_fungible(
                issue_cost,
                &token_display_name,
                &token_ticker,
                NonFungibleTokenProperties {
                    can_freeze: true,
                    can_wipe: true,
                    can_pause: true,
                    can_change_owner: true,
                    can_upgrade: true,
                    can_add_special_roles: true,
                },
            )
            .async_call()
            .with_callback(self.callbacks().hedging_token_issue_callback()))
    }

    #[endpoint(setHedgingTokenRoles)]
    fn set_hedging_token_roles(&self) -> AsyncCall {
        let token_id = self.hedging_token_id().get();
        let roles = [EsdtLocalRole::NftCreate, EsdtLocalRole::NftBurn];

        self.set_local_roles(&token_id, &roles)
    }

    fn create_hedging_token(&self) -> u64 {
        let token_id = self.hedging_token_id().get();
        let amount = BigUint::from(NFT_AMOUNT);

        self.create_nft(&token_id, &amount)
    }

    fn send_hedging_token(&self, to: &ManagedAddress, nft_nonce: u64) {
        let token_id = self.hedging_token_id().get();
        let amount = BigUint::from(NFT_AMOUNT);

        self.send().direct(to, &token_id, nft_nonce, &amount, &[]);
    }

    fn burn_hedging_token(&self, nft_nonce: u64) {
        let token_id = self.hedging_token_id().get();
        let amount = BigUint::from(NFT_AMOUNT);

        self.send().esdt_local_burn(&token_id, nft_nonce, &amount);
    }

    #[callback]
    fn hedging_token_issue_callback(
        &self,
        #[call_result] result: ManagedAsyncCallResult<TokenIdentifier>,
    ) -> OptionalResult<AsyncCall> {
        match result {
            ManagedAsyncCallResult::Ok(token_id) => {
                self.hedging_token_id().set(&token_id);

                OptionalResult::Some(self.set_hedging_token_roles())
            }
            ManagedAsyncCallResult::Err(_) => {
                self.refund_owner_failed_issue();

                OptionalResult::None
            }
        }
    }

    // storage

    #[view(getHedgingTokenId)]
    #[storage_mapper("hedgingTokenId")]
    fn hedging_token_id(&self) -> SingleValueMapper<TokenIdentifier>;
}
