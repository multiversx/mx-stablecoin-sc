elrond_wasm::imports!();

const STABLE_COIN_NAME: &[u8] = b"StableCoin";
const STABLE_COIN_TICKER: &[u8] = b"STCOIN";

#[elrond_wasm::module]
pub trait StablecoinTokenModule {
    #[payable("EGLD")]
    #[endpoint(issueStablecoinToken)]
    fn issue_stablecoin_token(
        &self,
        #[payment] issue_cost: Self::BigUint,
    ) -> SCResult<AsyncCall<Self::SendApi>> {
        only_owner!(self, "only owner can issue new tokens");
        require!(
            self.stablecoin_token_id().is_empty(),
            "Stablecoin already issued"
        );

        let token_display_name = BoxedBytes::from(STABLE_COIN_NAME);
        let token_ticker = BoxedBytes::from(STABLE_COIN_TICKER);
        let initial_supply = Self::BigUint::from(1u32);

        Ok(ESDTSystemSmartContractProxy::new_proxy_obj(self.send())
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
            .with_callback(self.callbacks().stablecoin_issue_callback()))
    }

    fn mint_stablecoin(&self, amount: &Self::BigUint) {
        self.send()
            .esdt_local_mint(&self.stablecoin_token_id().get(), 0, amount);

        self.total_circulating_supply()
            .update(|total| *total += amount);
    }

    fn burn_stablecoin(&self, amount: &Self::BigUint) {
        self.send()
            .esdt_local_burn(&self.stablecoin_token_id().get(), 0, amount);

        self.total_circulating_supply()
            .update(|total| *total -= amount);
    }

    fn send_stablecoin(&self, to: &Address, amount: &Self::BigUint) {
        self.send()
            .direct(to, &self.stablecoin_token_id().get(), 0, amount, &[]);
    }

    fn mint_and_send_stablecoin(&self, to: &Address, amount: &Self::BigUint) {
        self.mint_stablecoin(amount);
        self.send_stablecoin(to, amount);
    }

    fn require_stablecoin_issued(&self) -> SCResult<()> {
        require!(
            !self.stablecoin_token_id().is_empty(),
            "Stablecoin token must be issued first"
        );
        Ok(())
    }

    fn set_stablecoin_roles(&self) -> AsyncCall<Self::SendApi> {
        let own_sc_address = self.blockchain().get_sc_address();
        let token_id = self.stablecoin_token_id().get();
        let roles = [EsdtLocalRole::Mint, EsdtLocalRole::Burn];

        ESDTSystemSmartContractProxy::new_proxy_obj(self.send())
            .set_special_roles(&own_sc_address, &token_id, &roles)
            .async_call()
    }

    #[callback]
    fn stablecoin_issue_callback(
        &self,
        #[call_result] result: AsyncCallResult<TokenIdentifier>,
    ) -> OptionalResult<AsyncCall<Self::SendApi>> {
        match result {
            AsyncCallResult::Ok(token_id) => {
                self.stablecoin_token_id().set(&token_id);

                OptionalResult::Some(self.set_stablecoin_roles())
            }
            AsyncCallResult::Err(_) => {
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

    #[view(getStablecoinTokenId)]
    #[storage_mapper("stablecoinTokenId")]
    fn stablecoin_token_id(&self) -> SingleValueMapper<Self::Storage, TokenIdentifier>;

    #[view(getTotalCirculatingSupply)]
    #[storage_mapper("totalCirculatingSupply")]
    fn total_circulating_supply(&self) -> SingleValueMapper<Self::Storage, Self::BigUint>;
}
