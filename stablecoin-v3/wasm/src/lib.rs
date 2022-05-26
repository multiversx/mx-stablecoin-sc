////////////////////////////////////////////////////
////////////////// AUTO-GENERATED //////////////////
////////////////////////////////////////////////////

#![no_std]

elrond_wasm_node::wasm_endpoints! {
    stablecoin_v3
    (
        deployStablecoin
        getBasePool
        getCollateralTokenId
        getCollateralTokenSupply
        getLastReplenishBlock
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
        setSpreadFeeMinPercent
        setStateActiveNoSwaps
        setTokenTicker
        swapStablecoin
    )
}

elrond_wasm_node::wasm_empty_callback! {}
