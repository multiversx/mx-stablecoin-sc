elrond_wasm::imports!();

pub type AggregatorResultAsMultiValue<M> =
    MultiValue6<u32, ManagedBuffer<M>, ManagedBuffer<M>, u64, BigUint<M>, u8>;

#[elrond_wasm::proxy]
pub trait Aggregator {
    #[view(latestPriceFeed)]
    fn latest_price_feed(
        &self,
        from: ManagedBuffer,
        to: ManagedBuffer,
    ) -> AggregatorResultAsMultiValue<Self::Api>;
}

pub struct AggregatorResult<M: ManagedTypeApi> {
    pub round_id: u32,
    pub from_token_name: ManagedBuffer<M>,
    pub to_token_name: ManagedBuffer<M>,
    pub timestamp: u64,
    pub price: BigUint<M>,
    pub decimals: u8,
}

impl<M: ManagedTypeApi> From<AggregatorResultAsMultiValue<M>> for AggregatorResult<M> {
    fn from(multi_result: AggregatorResultAsMultiValue<M>) -> Self {
        let (round_id, from_token_name, to_token_name, timestamp, price, decimals) =
            multi_result.into_tuple();

        AggregatorResult {
            round_id,
            from_token_name,
            to_token_name,
            timestamp,
            price,
            decimals,
        }
    }
}
