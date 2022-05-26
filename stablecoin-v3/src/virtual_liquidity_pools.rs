elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use crate::MEDIAN_POOL_DELTA;

use super::config;

pub type Block = u64;

#[elrond_wasm::module]
pub trait VLPModule: config::ConfigModule {
    fn update_pool_delta(&self) {
        let current_block = self.blockchain().get_block_nonce();
        let last_replenish_block = self.last_replenish_block().get();
        if current_block > last_replenish_block {
            let median_delta = BigUint::from(MEDIAN_POOL_DELTA);
            let pool_delta_mapper = self.pool_delta();
            let pool_delta = pool_delta_mapper.get();
            let replenish_rate = self.pool_recovery_period().get();
            let replenish_value = if pool_delta > median_delta {
                (&pool_delta - &median_delta) / replenish_rate
            } else if median_delta > pool_delta {
                (&median_delta - &pool_delta) / replenish_rate
            } else {
                BigUint::zero()
            };

            if pool_delta < median_delta && ((&median_delta - &pool_delta) <= replenish_value) {
                pool_delta_mapper.update(|delta| *delta = median_delta.clone());
            } else if pool_delta > median_delta
                && ((&pool_delta - &median_delta) <= replenish_value)
            {
                pool_delta_mapper.update(|delta| *delta = median_delta.clone());
            } else if pool_delta < median_delta {
                pool_delta_mapper.update(|delta| *delta += &replenish_value);
            } else if pool_delta > median_delta {
                pool_delta_mapper.update(|delta| *delta -= &replenish_value);
            }

            self.last_replenish_block().set(current_block)
        }
    }

    fn mint_stablecoins(&self, amount: BigUint) -> EsdtTokenPayment<Self::Api> {
        let token_id = self.stablecoin().get_token_id();
        self.send().esdt_local_mint(&token_id, 0, &amount);
        self.stablecoin_supply().update(|x| *x += &amount);

        EsdtTokenPayment::new(token_id, 0, amount)
    }

    fn burn_stablecoins(&self, amount: BigUint) {
        let token_id = self.stablecoin().get_token_id();
        self.send().esdt_local_burn(&token_id, 0, &amount);
        self.stablecoin_supply().update(|x| *x -= &amount);
    }
}
