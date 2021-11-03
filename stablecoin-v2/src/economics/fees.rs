elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use crate::math::ONE;

#[derive(TopEncode, TopDecode, Clone)]
pub struct MinMaxPair<M: ManagedTypeApi> {
    pub min: BigUint<M>,
    pub max: BigUint<M>,
}

#[derive(TopEncode, TopDecode)]
pub struct CurrentFeeConfiguration<M: ManagedTypeApi> {
    pub hedging_ratio: BigUint<M>,
    pub mint_fee_percentage: BigUint<M>,
    pub burn_fee_percentage: BigUint<M>,
    pub slippage_percentage: BigUint<M>,
}

#[elrond_wasm::module]
pub trait FeesModule:
    crate::math::MathModule + crate::pools::PoolsModule + price_aggregator_proxy::PriceAggregatorModule
{
    #[view(getCurrentHedgingRatio)]
    fn get_current_hedging_ratio(&self, collateral_id: &TokenIdentifier) -> BigUint {
        self.current_fee_configuration(collateral_id)
            .get()
            .hedging_ratio
    }

    #[view(getMintTransactionFeesPercentage)]
    fn get_mint_transaction_fees_percentage(&self, collateral_id: &TokenIdentifier) -> BigUint {
        self.current_fee_configuration(collateral_id)
            .get()
            .mint_fee_percentage
    }

    #[view(getBurnTransactionFeesPercentage)]
    fn get_burn_transaction_fees_percentage(&self, collateral_id: &TokenIdentifier) -> BigUint {
        self.current_fee_configuration(collateral_id)
            .get()
            .burn_fee_percentage
    }

    #[view(getSlippagePercentage)]
    fn get_slippage_percenage(&self, collateral_id: &TokenIdentifier) -> BigUint {
        self.current_fee_configuration(collateral_id)
            .get()
            .slippage_percentage
    }

    // The more collateral is covered, the more expensive it is to open a position
    // This scales the same way as the burn transaction fees, so we use the same formula
    #[view(getHedgingPositionOpenTransactionFeesPercentage)]
    fn get_hedging_position_open_transaction_fees_percentage(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> BigUint {
        self.get_burn_transaction_fees_percentage(collateral_id)
    }

    // The more collateral is covered, the less expensive it is to exit
    // This scales the same way as the mint transaction fees
    #[view(getHedgingPositionCloseTransactionFeesPercentage)]
    fn get_hedging_position_close_transaction_fees_percentage(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> BigUint {
        self.get_mint_transaction_fees_percentage(collateral_id)
    }

    // mint fees decrease as coverage ratio increases
    fn calculate_mint_transaction_fees_percentage(
        &self,
        current_hedging_ratio: &BigUint,
        fees_percentage: MinMaxPair<Self::Api>,
    ) -> BigUint {
        let target_hedging_ratio = self.target_hedging_ratio().get();
        let one = BigUint::from(ONE);

        if current_hedging_ratio == &0 {
            return fees_percentage.max;
        }
        if current_hedging_ratio >= &target_hedging_ratio {
            return fees_percentage.min;
        }

        let percentage_diff = &fees_percentage.max - &fees_percentage.min;

        // max - (max - min) * hedging_ratio
        fees_percentage.max - self.multiply(current_hedging_ratio, &percentage_diff, &one)
    }

    // burn fees decrease as coverage ratio decreases
    fn calculate_burn_transaction_fees_percentage(
        &self,
        current_hedging_ratio: &BigUint,
        fees_percentage: MinMaxPair<Self::Api>,
    ) -> BigUint {
        let target_hedging_ratio = self.target_hedging_ratio().get();
        let one = BigUint::from(ONE);

        if current_hedging_ratio == &0 {
            return fees_percentage.min;
        }
        if current_hedging_ratio >= &target_hedging_ratio {
            return fees_percentage.max;
        }

        let percentage_diff = &fees_percentage.max - &fees_percentage.min;

        // min + (max - min) * hedging_ratio
        fees_percentage.min + self.multiply(current_hedging_ratio, &percentage_diff, &one)
    }

    fn calculate_slippage_percentage(
        &self,
        current_hedging_ratio: &BigUint,
        slippage_percentage: MinMaxPair<Self::Api>,
    ) -> BigUint {
        let one = BigUint::from(ONE);

        // no slippage if all collateral is covered
        if current_hedging_ratio >= &one {
            return BigUint::zero();
        }

        let percentage_diff = &slippage_percentage.max - &slippage_percentage.min;

        // max - (max - min) * hedging_ratio
        slippage_percentage.max - self.multiply(current_hedging_ratio, &percentage_diff, &one)
    }

    fn calculate_current_hedging_ratio(&self, collateral_id: &TokenIdentifier) -> BigUint {
        let pool = self.get_pool(collateral_id);
        let target_hedge_amount = self.calculate_target_hedge_amount(&pool.collateral_amount);
        self.calculate_ratio(
            &pool.total_covered_value_in_stablecoin,
            &target_hedge_amount,
        )
    }

    fn calculate_target_hedge_amount(&self, collateral_amount: &BigUint) -> BigUint {
        let target_hedging_ratio = self.target_hedging_ratio().get();
        self.multiply(
            &target_hedging_ratio,
            collateral_amount,
            &BigUint::from(ONE),
        )
    }

    fn calculate_limit_hedge_amount(&self, collateral_amount: &BigUint) -> BigUint {
        let hedging_ratio_limit = self.hedging_ratio_limit().get();
        self.multiply(&hedging_ratio_limit, collateral_amount, &BigUint::from(ONE))
    }

    // storage

    #[storage_mapper("minMaxFeesPercentage")]
    fn min_max_fees_percentage(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SingleValueMapper<MinMaxPair<Self::Api>>;

    #[storage_mapper("minMaxSlippagePercentage")]
    fn min_max_slippage_percentage(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SingleValueMapper<MinMaxPair<Self::Api>>;

    #[storage_mapper("currentFeeConfiguration")]
    fn current_fee_configuration(
        &self,
        collateral_id: &TokenIdentifier,
    ) -> SingleValueMapper<CurrentFeeConfiguration<Self::Api>>;

    #[storage_mapper("accumulatedTxFees")]
    fn accumulated_tx_fees(&self, collateral_id: &TokenIdentifier) -> SingleValueMapper<BigUint>;

    #[view(getTargetHedgingRatio)]
    #[storage_mapper("targetHedgingRatio")]
    fn target_hedging_ratio(&self) -> SingleValueMapper<BigUint>;

    #[view(getHedgingRatioLimit)]
    #[storage_mapper("hedgingRatioLimit")]
    fn hedging_ratio_limit(&self) -> SingleValueMapper<BigUint>;
}
