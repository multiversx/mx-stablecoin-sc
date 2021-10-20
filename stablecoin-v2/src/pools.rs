elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use price_aggregator_proxy::DOLLAR_TICKER;

#[derive(TypeAbi, TopEncode, TopDecode)]
pub struct Pool<M: ManagedTypeApi> {
    pub collateral_amount: BigUint<M>,
    pub stablecoin_amount: BigUint<M>,
    pub total_collateral_covered: BigUint<M>,
    pub total_covered_value_in_stablecoin: BigUint<M>,
}

impl<M: ManagedTypeApi> Pool<M> {
    fn new(api: M) -> Self {
        Pool {
            collateral_amount: BigUint::zero(api.clone()),
            stablecoin_amount: BigUint::zero(api.clone()),
            total_collateral_covered: BigUint::zero(api.clone()),
            total_covered_value_in_stablecoin: BigUint::zero(api.clone()),
        }
    }
}

#[elrond_wasm::module]
pub trait PoolsModule:
    crate::math::MathModule + price_aggregator_proxy::PriceAggregatorModule
{
    #[view(getCoverageRatio)]
    fn get_coverage_ratio(&self, collateral_id: &TokenIdentifier) -> BigUint {
        let reserves = self.get_pool_reserves(collateral_id);
        let total_covered = self.get_pool_amount_covered(collateral_id);

        self.calculate_ratio(&total_covered, &reserves)
    }

    fn get_pool(&self, collateral_id: &TokenIdentifier) -> Pool<Self::Api> {
        if self.pool_for_collateral(collateral_id).is_empty() {
            Pool::new(self.raw_vm_api())
        } else {
            self.pool_for_collateral(collateral_id).get()
        }
    }

    #[inline(always)]
    fn get_pool_reserves(&self, collateral_id: &TokenIdentifier) -> BigUint {
        self.get_pool(collateral_id).collateral_amount
    }

    #[inline(always)]
    fn get_pool_amount_covered(&self, collateral_id: &TokenIdentifier) -> BigUint {
        self.get_pool(collateral_id).total_collateral_covered
    }

    fn update_pool<R, F: FnOnce(&mut Pool<Self::Api>) -> R>(
        &self,
        collateral_id: &TokenIdentifier,
        f: F,
    ) -> R {
        let mut pool = self.get_pool(collateral_id);
        let result = f(&mut pool);
        self.pool_for_collateral(collateral_id).set(&pool);

        result
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

    fn require_collateral_in_whitelist(&self, collateral_id: &TokenIdentifier) -> SCResult<()> {
        require!(
            self.collateral_whitelist().contains(collateral_id),
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

    #[storage_mapper("collateralNumDecimals")]
    fn collateral_num_decimals(&self, collateral_id: &TokenIdentifier) -> SingleValueMapper<u32>;

    #[view(getPoolForCollateral)]
    #[storage_mapper("poolForCollateral")]
    fn pool_for_collateral(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SingleValueMapper<Pool<Self::Api>>;
}
