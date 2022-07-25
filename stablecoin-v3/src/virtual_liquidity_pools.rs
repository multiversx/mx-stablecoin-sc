elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use super::config;

pub type Block = u64;

#[elrond_wasm::module]
pub trait VLPModule: config::ConfigModule {
    fn update_pool_delta(&self) {
        let current_block = self.blockchain().get_block_nonce();
        let last_replenish_block = self.last_replenish_block().get();
        if current_block > last_replenish_block {
            let median_delta = self.median_pool_delta().get();
            let pool_delta_mapper = self.pool_delta();
            let pool_delta = pool_delta_mapper.get();
            let replenish_rate = self.pool_recovery_period().get();
            let blocks_diff = current_block - last_replenish_block; //needed for cases of nonconsecutive blocks, otherwise should be 1, with constant swaps
            let replenish_value = if pool_delta > median_delta {
                (&pool_delta - &median_delta) / replenish_rate * blocks_diff
            } else if median_delta > pool_delta {
                (&median_delta - &pool_delta) / replenish_rate * blocks_diff
            } else {
                BigUint::zero()
            };

            if pool_delta < median_delta {
                if (&median_delta - &pool_delta) <= replenish_value {
                    pool_delta_mapper.set(median_delta.clone());
                } else {
                    pool_delta_mapper.update(|delta| *delta += &replenish_value);
                }
            } else if pool_delta > median_delta {
                if (&pool_delta - &median_delta) <= replenish_value {
                    pool_delta_mapper.set(median_delta.clone());
                } else {
                    pool_delta_mapper.update(|delta| *delta -= &replenish_value);
                }
            }

            self.last_replenish_block().set(current_block)
        }
    }

    fn mint_stablecoins(&self, amount: BigUint) -> EsdtTokenPayment<Self::Api> {
        self.stablecoin_supply().update(|x| *x += &amount);
        self.stablecoin().mint(amount.clone());

        EsdtTokenPayment::new(self.stablecoin().get_token_id(), 0, amount)
    }

    fn burn_stablecoins(&self, amount: BigUint) {
        self.stablecoin_supply().update(|x| *x -= &amount);
        self.stablecoin().burn(&amount);
    }
}
