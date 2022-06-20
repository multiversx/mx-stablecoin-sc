////////////////////////////////////////////////////
////////////////// AUTO-GENERATED //////////////////
////////////////////////////////////////////////////

#![no_std]

elrond_wasm_node::wasm_endpoints! {
    stablecoin_v3
    (
        deployStablecoin
        getBasePool
        getCollateralSupply
        getCollateralTokenId
        getLastReplenishBlock
        getMedianPoolDelta
        getPoolDelta
        getPoolRecoveryPeriod
        getPriceAggregatorAddress
        getSpreadFeeMinPercent
        getStablecoinId
        getStablecoinSupply
        getState
        pause
        registerStablecoin
        resume
        setPoolRecoveryPeriod
        setSpreadFeeMinPercent
        setStateActiveNoSwaps
        setTokenTicker
        swapStablecoin
    )
}

elrond_wasm_node::wasm_empty_callback! {}
