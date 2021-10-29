elrond_wasm::imports!();
elrond_wasm::derive_imports!();

const LEND_ACCEPT_FUNDS_FUNC_NAME: &[u8] = b"lend_accept_funds_func";
const WITHDRAW_ACCEPT_FUNDS_FUNC_NAME: &[u8] = b"withdraw_accept_funds_func";

type ManagedHash<M> = ManagedByteArray<M, 32>;

#[derive(TopEncode, TopDecode)]
pub struct LendMetadata<M: ManagedTypeApi> {
    lend_epoch: u64,
    lend_amount: BigUint<M>,
    lend_token_nonce: u64,
}

pub mod lending_proxy {
    elrond_wasm::imports!();

    #[elrond_wasm::proxy]
    pub trait Lending {
        #[payable("*")]
        #[endpoint]
        fn deposit(
            &self,
            #[payment_token] asset: TokenIdentifier,
            #[payment_amount] amount: BigUint,
            #[var_args] caller: OptionalArg<ManagedAddress>,
            #[var_args] accept_funds_func: OptionalArg<ManagedBuffer>,
        );

        #[payable("*")]
        #[endpoint]
        fn withdraw(
            &self,
            #[payment_token] lend_token: TokenIdentifier,
            #[payment_nonce] token_nonce: u64,
            #[payment_amount] amount: BigUint,
            #[var_args] caller: OptionalArg<ManagedAddress>,
            #[var_args] accept_funds_func: OptionalArg<ManagedBuffer>,
        );
    }
}

#[elrond_wasm::module]
pub trait LendingModule:
    crate::lending_events::LendingEventsModule
    + crate::math::MathModule
    + crate::pools::PoolsModule
    + price_aggregator_proxy::PriceAggregatorModule
{
    fn lend(&self, collateral_id: TokenIdentifier) -> SCResult<AsyncCall> {
        require!(
            self.lend_metadata_for_collateral(&collateral_id).is_empty(),
            "Already lended for this pool"
        );

        let lend_amount = self.update_pool(&collateral_id, |pool| {
            let reserves_lend_percentage = self.reserves_lend_percentage(&collateral_id).get();
            let lend_amount =
                self.calculate_percentage_of(&reserves_lend_percentage, &pool.collateral_reserves);

            pool.collateral_reserves -= &lend_amount;

            let min_leftover_reserves_after_lend =
                self.min_leftover_reserves_after_lend(&collateral_id).get();
            require!(
                pool.collateral_reserves >= min_leftover_reserves_after_lend,
                "Not enough reserves to lend"
            );

            Ok(lend_amount)
        })?;

        self.lend_metadata_for_collateral(&collateral_id)
            .set(&LendMetadata {
                lend_epoch: self.blockchain().get_block_epoch(),
                lend_amount: lend_amount.clone(),
                lend_token_nonce: 0,
            });
        self.set_temp_collateral_id(&collateral_id);

        let lending_sc_address = self.lending_sc_address().get();
        let own_sc_address = self.blockchain().get_sc_address();
        let accept_funds_func = ManagedBuffer::from(LEND_ACCEPT_FUNDS_FUNC_NAME);

        Ok(self
            .lending_proxy(lending_sc_address)
            .deposit(
                collateral_id,
                lend_amount,
                OptionalArg::Some(own_sc_address),
                OptionalArg::Some(accept_funds_func),
            )
            .async_call()
            .with_callback(self.callbacks().lend_callback()))
    }

    #[payable("*")]
    #[endpoint]
    fn lend_accept_funds_func(&self, #[payment_nonce] payment_nonce: u64) -> SCResult<()> {
        let collateral_id = self.get_temp_collateral_id();
        self.lend_metadata_for_collateral(&collateral_id)
            .update(|lend_metadata| {
                require!(
                    lend_metadata.lend_token_nonce == 0,
                    "Invalid call, token nonce already set"
                );

                lend_metadata.lend_token_nonce = payment_nonce;

                self.reserves_lended_event(
                    &collateral_id,
                    lend_metadata.lend_epoch,
                    lend_metadata.lend_token_nonce,
                    &lend_metadata.lend_amount,
                );

                Ok(())
            })
    }

    #[payable("*")]
    #[callback]
    fn lend_callback(
        &self,
        #[payment_amount] payment_amount: BigUint,
        #[call_result] result: ManagedAsyncCallResult<MultiResultVec<ManagedBuffer>>,
    ) {
        match result {
            // ignore results, they are from nested calls from the callee contract
            ManagedAsyncCallResult::Ok(_) => {}
            // revert
            ManagedAsyncCallResult::Err(_) => {
                let collateral_id = self.get_temp_collateral_id();
                
                self.lend_metadata_for_collateral(&collateral_id).clear();
                self.update_pool(&collateral_id, |pool| {
                    pool.collateral_reserves += payment_amount;
                });
            }
        }

        self.clear_temp_collateral_id();
    }

    fn withdraw(&self, collateral_id: TokenIdentifier) -> SCResult<AsyncCall> {
        require!(
            !self.lend_metadata_for_collateral(&collateral_id).is_empty(),
            "Must lend first"
        );

        let lend_metadata = self.lend_metadata_for_collateral(&collateral_id).get();

        let min_lend_epochs = self.min_lend_epochs().get();
        let current_epoch = self.blockchain().get_block_epoch();
        let epoch_diff = current_epoch - lend_metadata.lend_epoch;
        require!(
            epoch_diff >= min_lend_epochs,
            "Trying to withdraw too early"
        );

        self.set_temp_collateral_id(&collateral_id);

        let lending_token_id = self.lending_token_id().get();
        let lending_sc_address = self.lending_sc_address().get();
        let own_sc_address = self.blockchain().get_sc_address();
        let accept_funds_func = ManagedBuffer::from(WITHDRAW_ACCEPT_FUNDS_FUNC_NAME);

        Ok(self
            .lending_proxy(lending_sc_address)
            .withdraw(
                lending_token_id,
                lend_metadata.lend_token_nonce,
                lend_metadata.lend_amount,
                OptionalArg::Some(own_sc_address),
                OptionalArg::Some(accept_funds_func),
            )
            .async_call()
            .with_callback(self.callbacks().withdraw_callback()))
    }

    #[payable("*")]
    #[endpoint]
    fn withdraw_accept_funds_func(
        &self,
        #[payment_token] payment_token: TokenIdentifier,
        #[payment_amount] payment_amount: BigUint,
    ) {
        let lend_metadata = self.lend_metadata_for_collateral(&payment_token).get();
        let rewards = &payment_amount - &lend_metadata.lend_amount;

        self.update_pool(&payment_token, |pool| {
            pool.collateral_reserves += &lend_metadata.lend_amount;
        });
        self.accumulated_lend_rewards(&payment_token)
            .update(|accumulated_rewards| *accumulated_rewards += rewards);

        self.lended_reserves_withdrawn_event(&payment_token, &payment_amount);
    }

    #[payable("*")]
    #[callback]
    fn withdraw_callback(
        &self,
        #[call_result] result: ManagedAsyncCallResult<MultiResultVec<ManagedBuffer>>,
    ) {
        match result {
            // ignore results, they are from nested calls from the callee contract
            ManagedAsyncCallResult::Ok(_) => {
                let collateral_id = self.get_temp_collateral_id();
                self.lend_metadata_for_collateral(&collateral_id).clear();
            }
            // nothing to revert in case of error
            ManagedAsyncCallResult::Err(_) => {}
        }

        self.clear_temp_collateral_id();
    }

    fn set_temp_collateral_id(&self, collateral_id: &TokenIdentifier) {
        let tx_hash = self.blockchain().get_tx_hash();
        self.lend_temp_collateral_id(&tx_hash).set(collateral_id);
    }

    fn get_temp_collateral_id(&self) -> TokenIdentifier {
        let tx_hash = self.blockchain().get_tx_hash();
        self.lend_temp_collateral_id(&tx_hash).get()
    }

    fn clear_temp_collateral_id(&self) {
        let tx_hash = self.blockchain().get_tx_hash();
        self.lend_temp_collateral_id(&tx_hash).clear();
    }

    // proxies

    #[proxy]
    fn lending_proxy(&self, sc_address: ManagedAddress) -> lending_proxy::Proxy<Self::Api>;

    // storage

    #[view(getLendingScAddress)]
    #[storage_mapper("lendingScAddress")]
    fn lending_sc_address(&self) -> SingleValueMapper<ManagedAddress>;

    #[view(getLendingTokenId)]
    #[storage_mapper("lendingTokenId")]
    fn lending_token_id(&self) -> SingleValueMapper<TokenIdentifier>;

    #[view(getMinLendEpochs)]
    #[storage_mapper("minLendEpochs")]
    fn min_lend_epochs(&self) -> SingleValueMapper<u64>;

    #[storage_mapper("lendMetadataForCollateral")]
    fn lend_metadata_for_collateral(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SingleValueMapper<LendMetadata<Self::Api>>;

    #[storage_mapper("accumulatedLendRewards")]
    fn accumulated_lend_rewards(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SingleValueMapper<BigUint>;

    // used to keep the token ID between the initial call and the receive funds function
    #[storage_mapper("lendTempTokenId")]
    fn lend_temp_collateral_id(
        &self,
        tx_hash: &ManagedHash<Self::Api>,
    ) -> SingleValueMapper<TokenIdentifier>;
}
