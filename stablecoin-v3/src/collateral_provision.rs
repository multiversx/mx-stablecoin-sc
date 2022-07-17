elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use crate::{config, errors::ERROR_COLLATERAL_TOKEN_ALREADY_DEFINED};
use elrond_wasm::elrond_codec::TopEncode;

#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, TypeAbi, Clone, PartialEq, Debug)]
pub struct CpTokenAttributes<M: ManagedTypeApi> {
    pub stablecoin_reward_per_share: BigUint<M>,
    pub collateral_reward_per_share: BigUint<M>,
    pub entering_epoch: u64,
}

#[elrond_wasm::module]
pub trait CollateralProvisionModule: config::ConfigModule {
    #[only_owner]
    #[payable("EGLD")]
    #[endpoint(registerFarmToken)]
    fn register_cp_token(
        &self,
        token_display_name: ManagedBuffer,
        token_ticker: ManagedBuffer,
        num_decimals: usize,
    ) {
        let payment_amount = self.call_value().egld_value();
        self.cp_token().issue_and_set_all_roles(
            EsdtTokenType::Meta,
            payment_amount,
            token_display_name,
            token_ticker,
            num_decimals,
            None,
        );
    }

    #[only_owner]
    #[endpoint(registerCollateralToken)]
    fn register_collateral_token(
        &self,
        collateral_token: TokenIdentifier,
        collateral_token_ticker: ManagedBuffer,
    ) {
        require!(
            !self.collateral_tokens().contains(&collateral_token),
            ERROR_COLLATERAL_TOKEN_ALREADY_DEFINED
        );
        self.collateral_tokens().add(&collateral_token);
        self.token_ticker(&collateral_token)
            .set(collateral_token_ticker);
    }

    fn mint_cp_tokens<T: TopEncode>(
        &self,
        token_id: TokenIdentifier,
        amount: BigUint,
        attributes: &T,
    ) -> EsdtTokenPayment<Self::Api> {
        let new_nonce = self
            .send()
            .esdt_nft_create_compact(&token_id, &amount, attributes);
        self.cp_token_supply().update(|x| *x += &amount);

        EsdtTokenPayment::new(token_id, new_nonce, amount)
    }

    fn burn_cp_tokens(&self, token_id: &TokenIdentifier, nonce: u64, amount: &BigUint) {
        self.send().esdt_local_burn(token_id, nonce, amount);
        self.cp_token_supply().update(|x| *x -= amount);
    }

    fn get_cp_token_attributes<T: TopDecode>(
        &self,
        token_id: &TokenIdentifier,
        token_nonce: u64,
    ) -> T {
        let token_info = self.blockchain().get_esdt_token_data(
            &self.blockchain().get_sc_address(),
            token_id,
            token_nonce,
        );

        token_info.decode_attributes()
    }

    fn update_rewards(&self, token_id: &TokenIdentifier, fee_amount: &BigUint) {
        let division_safety_constant = self.division_safety_constant().get();
        let cp_token_supply = self.cp_token_supply().get();
        self.reward_reserve(token_id).update(|x| *x += fee_amount);

        if cp_token_supply != 0u64 {
            let increase = (fee_amount * &division_safety_constant) / cp_token_supply;
            self.reward_per_share(token_id).update(|x| *x += &increase);
        }
    }
}
