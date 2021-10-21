elrond_wasm::imports!();

const STABLE_COIN_NAME: &[u8] = b"StableCoin";
const STABLE_COIN_TICKER: &[u8] = b"STCOIN";
const STABLE_COIN_NUM_DECIMALS: usize = 6;
// pub const STABLE_COIN_PRECISION: u64 = 1_000_000;

#[elrond_wasm::module]
pub trait StablecoinTokenModule: crate::token_common::TokenCommonModule {
    #[only_owner]
    #[payable("EGLD")]
    #[endpoint(issueStablecoinToken)]
    fn issue_stablecoin_token(&self, #[payment] issue_cost: BigUint) -> SCResult<AsyncCall> {
        require!(
            self.stablecoin_token_id().is_empty(),
            "Stablecoin already issued"
        );

        let token_display_name = ManagedBuffer::from(STABLE_COIN_NAME);
        let token_ticker = ManagedBuffer::from(STABLE_COIN_TICKER);
        let initial_supply = BigUint::zero();

        Ok(self
            .send()
            .esdt_system_sc_proxy()
            .issue_fungible(
                issue_cost,
                &token_display_name,
                &token_ticker,
                &initial_supply,
                FungibleTokenProperties {
                    can_burn: true,
                    can_mint: true,
                    num_decimals: STABLE_COIN_NUM_DECIMALS,
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

    #[endpoint(setStablecoinRoles)]
    fn set_stablecoin_roles(&self) -> AsyncCall {
        let token_id = self.stablecoin_token_id().get();
        let roles = [EsdtLocalRole::Mint, EsdtLocalRole::Burn];

        self.set_local_roles(&token_id, &roles)
    }

    fn mint_stablecoin(&self, amount: &BigUint) {
        self.send()
            .esdt_local_mint(&self.stablecoin_token_id().get(), 0, amount);

        self.stablecoin_total_circulating_supply()
            .update(|total| *total += amount);
    }

    fn burn_stablecoin(&self, amount: &BigUint) {
        self.send()
            .esdt_local_burn(&self.stablecoin_token_id().get(), 0, amount);

        self.stablecoin_total_circulating_supply()
            .update(|total| *total -= amount);
    }

    fn send_stablecoin(&self, to: &ManagedAddress, amount: &BigUint) {
        self.send()
            .direct(to, &self.stablecoin_token_id().get(), 0, amount, &[]);
    }

    fn mint_and_send_stablecoin(&self, to: &ManagedAddress, amount: &BigUint) {
        self.mint_stablecoin(amount);
        self.send_stablecoin(to, amount);
    }

    #[callback]
    fn stablecoin_issue_callback(
        &self,
        #[call_result] result: ManagedAsyncCallResult<TokenIdentifier>,
    ) -> OptionalResult<AsyncCall> {
        match result {
            ManagedAsyncCallResult::Ok(token_id) => {
                self.stablecoin_token_id().set(&token_id);

                OptionalResult::Some(self.set_stablecoin_roles())
            }
            ManagedAsyncCallResult::Err(_) => {
                self.refund_owner_failed_issue();

                OptionalResult::None
            }
        }
    }

    // storage

    #[view(getStablecoinTokenId)]
    #[storage_mapper("stablecoinTokenId")]
    fn stablecoin_token_id(&self) -> SingleValueMapper<TokenIdentifier>;

    #[view(getStablecoinTotalCirculatingSupply)]
    #[storage_mapper("stablecoinTotalCirculatingSupply")]
    fn stablecoin_total_circulating_supply(&self) -> SingleValueMapper<BigUint>;
}
