#![no_std]

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

mod fees;
mod math;
mod stablecoin_token;

use price_aggregator_proxy::DOLLAR_TICKER;

#[derive(TypeAbi, TopEncode, TopDecode)]
pub struct Pool<M: ManagedTypeApi> {
    pub collateral_amount: BigUint<M>,
    pub stablecoin_amount: BigUint<M>,
}

#[elrond_wasm::contract]
pub trait StablecoinV2:
    fees::FeesModule
    + math::MathModule
    + price_aggregator_proxy::PriceAggregatorModule
    + stablecoin_token::StablecoinTokenModule
{
    #[init]
    fn init(&self, price_aggregator_address: ManagedAddress) -> SCResult<()> {
        require!(
            self.blockchain()
                .is_smart_contract(&price_aggregator_address),
            "Price aggregator address is not a smart contract"
        );

        self.price_aggregator_address()
            .set(&price_aggregator_address);

        Ok(())
    }

    // endpoints - owner-only

    #[only_owner]
    #[endpoint(addCollateralToWhitelist)]
    fn add_collateral_to_whitelist(
        &self,
        collateral_id: TokenIdentifier,
        collateral_ticker: ManagedBuffer,
        max_leverage: BigUint,
        min_fees_percentage: BigUint,
        max_fees_percentage: BigUint,
    ) -> SCResult<()> {
        require!(
            min_fees_percentage <= max_fees_percentage
                && max_fees_percentage < math::PERCENTAGE_PRECISION,
            "Invalid fees percentages"
        );

        if self.pool_for_collateral(&collateral_id).is_empty() {
            self.pool_for_collateral(&collateral_id).set(&Pool {
                collateral_amount: BigUint::zero(),
                stablecoin_amount: BigUint::zero(),
            });
        }

        self.collateral_ticker(&collateral_id)
            .set(&collateral_ticker);
        self.max_leverage(&collateral_id).set(&max_leverage);
        self.min_max_fees_percentage(&collateral_id)
            .set(&(min_fees_percentage, max_fees_percentage));
        let _ = self.collateral_whitelist().insert(collateral_id);

        Ok(())
    }

    #[only_owner]
    #[endpoint(removeCollateralFromWhitelist)]
    fn remove_collateral_from_whitelist(&self, collateral_id: TokenIdentifier) {
        self.collateral_ticker(&collateral_id).clear();
        self.max_leverage(&collateral_id).clear();
        self.min_max_fees_percentage(&collateral_id).clear();
        let _ = self.collateral_whitelist().remove(&collateral_id);
    }

    // endpoints

    #[payable("*")]
    #[endpoint(sellCollateral)]
    fn sell_collateral(
        &self,
        #[payment_token] payment_token: TokenIdentifier,
        #[payment_amount] payment_amount: BigUint,
        min_amount_out: BigUint,
    ) -> SCResult<()> {
        self.require_collateral_in_whitelist(&payment_token)?;

        let collateral_ticker = self.collateral_ticker(&payment_token).get();
        let collateral_value_in_dollars = self
            .get_price_for_pair(collateral_ticker, ManagedBuffer::from(DOLLAR_TICKER))
            .ok_or("Could not get collateral value in dollars")?;

        let transaction_fees_percentage = self.get_mint_transaction_fees_percentage(&payment_token);
        let fees_amount_in_collateral =
            self.calculate_percentage_of(&transaction_fees_percentage, &payment_amount);
        let collateral_amount = &payment_amount - &fees_amount_in_collateral;

        let stablecoin_amount = &collateral_value_in_dollars * &collateral_amount;
        require!(stablecoin_amount >= min_amount_out, "Below min amount");

        self.pool_for_collateral(&payment_token).update(|pool| {
            pool.collateral_amount += &collateral_amount;
            pool.stablecoin_amount += &stablecoin_amount;
        });
        self.reserves(&payment_token)
            .update(|reserves| *reserves += collateral_amount);
        self.accumulated_tx_fees(&payment_token)
            .update(|accumulated_fees| *accumulated_fees += fees_amount_in_collateral);

        let caller = self.blockchain().get_caller();
        self.mint_and_send_stablecoin(&caller, &stablecoin_amount);

        // TODO: try re-balance the pool here?

        Ok(())
    }

    #[payable("*")]
    #[endpoint(buyCollateral)]
    fn buy_collateral(
        &self,
        #[payment_token] payment_token: TokenIdentifier,
        #[payment_amount] payment_amount: BigUint,
        collateral_id: TokenIdentifier,
        min_amount_out: BigUint,
    ) -> SCResult<()> {
        let stablecoin_token_id = self.stablecoin_token_id().get();
        require!(
            payment_token == stablecoin_token_id,
            "May only pay with stablecoins"
        );
        self.require_collateral_in_whitelist(&collateral_id)?;

        let collateral_ticker = self.collateral_ticker(&payment_token).get();
        let collateral_value_in_dollars = self
            .get_price_for_pair(collateral_ticker, ManagedBuffer::from(DOLLAR_TICKER))
            .ok_or("Could not get collateral value in dollars")?;

        let total_value_in_collateral = &payment_amount / &collateral_value_in_dollars;
        let transaction_fees_percentage = self.get_burn_transaction_fees_percentage(&payment_token);
        let fees_amount_in_collateral =
            self.calculate_percentage_of(&transaction_fees_percentage, &total_value_in_collateral);

        let collateral_amount = &total_value_in_collateral - &fees_amount_in_collateral;
        require!(collateral_amount >= min_amount_out, "Below min amount");

        self.pool_for_collateral(&collateral_id).update(|pool| {
            require!(
                pool.collateral_amount >= collateral_amount,
                "Insufficient funds for swap"
            );

            // TODO: This should be fixed by re-balancing the pool, rather than throwing an error
            require!(
                pool.stablecoin_amount >= payment_amount,
                "Too many stablecoins paid"
            );

            pool.collateral_amount -= &collateral_amount;
            pool.stablecoin_amount -= &payment_amount;

            Ok(())
        })?;
        self.reserves(&collateral_id)
            .update(|reserves| *reserves -= &collateral_amount);
        self.accumulated_tx_fees(&collateral_id)
            .update(|accumulated_fees| *accumulated_fees += fees_amount_in_collateral);

        self.burn_stablecoin(&payment_amount);

        let caller = self.blockchain().get_caller();
        self.send()
            .direct(&caller, &collateral_id, 0, &collateral_amount, &[]);

        Ok(())
    }

    // --- force close position if coverage_ratio is >=1

    // -- Slippage prot for hedgers: max oracle value for hedging, min value for exit

    // private

    fn require_collateral_in_whitelist(&self, collateral_id: &TokenIdentifier) -> SCResult<()> {
        require!(
            self.collateral_whitelist().contains(&collateral_id),
            "collateral is not whitelisted"
        );
        Ok(())
    }

    // storage

    #[view(getCollateralWhitelist)]
    #[storage_mapper("collateralWhitelist")]
    fn collateral_whitelist(&self) -> SetMapper<TokenIdentifier>;

    #[view(getCollateralTicker)]
    #[storage_mapper("collateralTicker")]
    fn collateral_ticker(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SingleValueMapper<ManagedBuffer>;

    #[view(getMaxLeverage)]
    #[storage_mapper("maxLeverage")]
    fn max_leverage(&self, collateral_id: &TokenIdentifier) -> SingleValueMapper<BigUint>;

    #[view(getPoolForCollateral)]
    #[storage_mapper("poolForCollateral")]
    fn pool_for_collateral(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SingleValueMapper<Pool<Self::Api>>;
}
