elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use price_aggregator_proxy::DOLLAR_TICKER;

#[derive(TypeAbi, TopEncode, TopDecode)]
pub struct Pool<M: ManagedTypeApi> {
    pub collateral_amount: BigUint<M>,
    pub stablecoin_amount: BigUint<M>,
    pub collateral_reserves: BigUint<M>,
    pub total_collateral_covered: BigUint<M>,
    pub total_covered_value_in_stablecoin: BigUint<M>,
}

impl<M: ManagedTypeApi> Pool<M> {
    pub fn new(api: M) -> Self {
        Pool {
            collateral_amount: BigUint::zero(api.clone()),
            stablecoin_amount: BigUint::zero(api.clone()),
            collateral_reserves: BigUint::zero(api.clone()),
            total_collateral_covered: BigUint::zero(api.clone()),
            total_covered_value_in_stablecoin: BigUint::zero(api),
        }
    }
}

#[elrond_wasm::module]
pub trait PoolsModule:
    crate::math::MathModule + price_aggregator_proxy::PriceAggregatorModule
{
    #[inline(always)]
    fn get_pool(&self, collateral_id: &TokenIdentifier) -> Pool<Self::Api> {
        self.pool_for_collateral(collateral_id).get()
    }

    #[inline(always)]
    fn get_pool_collateral_amount(&self, collateral_id: &TokenIdentifier) -> BigUint {
        self.get_pool(collateral_id).collateral_amount
    }

    #[inline(always)]
    fn get_pool_amount_covered(&self, collateral_id: &TokenIdentifier) -> BigUint {
        self.get_pool(collateral_id).total_collateral_covered
    }

    #[inline(always)]
    fn get_pool_reserves(&self, collateral_id: &TokenIdentifier) -> BigUint {
        self.get_pool(collateral_id).collateral_reserves
    }

    #[inline(always)]
    fn update_pool<R, F: FnOnce(&mut Pool<Self::Api>) -> R>(
        &self,
        collateral_id: &TokenIdentifier,
        f: F,
    ) -> R {
        self.pool_for_collateral(collateral_id).update(f)
    }

    #[inline(always)]
    fn set_pool(&self, collateral_id: &TokenIdentifier, pool: &Pool<Self::Api>) {
        self.pool_for_collateral(collateral_id).set(pool);
    }

    fn get_collateral_value_in_dollars(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SCResult<BigUint> {
        let collateral_ticker = self.collateral_ticker(collateral_id).get();
        self.get_price_for_pair(collateral_ticker, ManagedBuffer::from(DOLLAR_TICKER))
            .ok_or("Could not get collateral value in dollars")
            .into()
    }

    fn get_collateral_precision(&self, collateral_id: &TokenIdentifier) -> BigUint {
        let collateral_num_decimals = self.collateral_num_decimals(collateral_id).get();
        self.create_precision_biguint(collateral_num_decimals)
    }

    #[inline(always)]
    fn is_collateral_whitelisted(&self, collateral_id: &TokenIdentifier) -> bool {
        self.collateral_whitelisted(collateral_id).get()
    }

    fn require_collateral_in_whitelist(&self, collateral_id: &TokenIdentifier) -> SCResult<()> {
        require!(
            self.is_collateral_whitelisted(collateral_id),
            "collateral is not whitelisted"
        );
        Ok(())
    }

    // storage

    #[view(isCollateralWhitelisted)]
    #[storage_mapper("collateralWhitelisted")]
    fn collateral_whitelisted(&self, collateral_id: &TokenIdentifier) -> SingleValueMapper<bool>;

    #[view(getCollateralTicker)]
    #[storage_mapper("collateralTicker")]
    fn collateral_ticker(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SingleValueMapper<ManagedBuffer>;

    #[storage_mapper("collateralNumDecimals")]
    fn collateral_num_decimals(&self, collateral_id: &TokenIdentifier) -> SingleValueMapper<u32>;

    #[view(getReservesLendPercentage)]
    #[storage_mapper("reservesLendPercentage")]
    fn reserves_lend_percentage(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SingleValueMapper<BigUint>;

    #[view(getMinLeftoverReservesAfterLend)]
    #[storage_mapper("minLeftoverReservesAfterLend")]
    fn min_leftover_reserves_after_lend(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SingleValueMapper<BigUint>;

    #[view(getPoolForCollateral)]
    #[storage_mapper("poolForCollateral")]
    fn pool_for_collateral(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SingleValueMapper<Pool<Self::Api>>;
}
