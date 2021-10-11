elrond_wasm::imports!();

const HEDGING_TOKEN_NAME: &[u8] = b"HedgingToken";
const HEDGING_TOKEN_TICKER: &[u8] = b"HEDGE";
const NFT_AMOUNT: u32 = 1;

#[elrond_wasm::module]
pub trait HedgingTokenModule {
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
        let own_sc_address = self.blockchain().get_sc_address();
        let token_id = self.hedging_token_id().get();
        let roles = [EsdtLocalRole::NftCreate, EsdtLocalRole::NftBurn];

        self.send()
            .esdt_system_sc_proxy()
            .set_special_roles(
                &own_sc_address,
                &token_id,
                (&roles[..]).into_iter().cloned(),
            )
            .async_call()
    }

    fn create_and_send_hedging_token(&self, to: &ManagedAddress) -> u64 {
        let token_id = self.hedging_token_id().get();
        let amount = BigUint::from(NFT_AMOUNT);
        let mut uris = ManagedVec::new();
        uris.push(ManagedBuffer::new());

        let nft_nonce = self.send().esdt_nft_create(
            &token_id,
            &amount,
            &ManagedBuffer::new(),
            &BigUint::zero(),
            &ManagedBuffer::new(),
            &(),
            &uris
        );
        self.send()
            .direct(to, &token_id, nft_nonce, &amount, &[]);

        nft_nonce
    }

    fn burn_hedging_token(&self, nft_nonce: u64) {
        let token_id = self.hedging_token_id().get();
        self.send()
            .esdt_local_burn(&token_id, nft_nonce, &BigUint::from(NFT_AMOUNT));
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
                let initial_caller = self.blockchain().get_owner_address();
                let egld_returned = self.call_value().egld_value();
                if egld_returned > 0 {
                    self.send()
                        .direct_egld(&initial_caller, &egld_returned, &[]);
                }

                OptionalResult::None
            }
        }
    }

    // storage

    #[view(getHedgingTokenId)]
    #[storage_mapper("hedgingTokenId")]
    fn hedging_token_id(&self) -> SingleValueMapper<TokenIdentifier>;
}
