////////////////////////////////////////////////////
////////////////// AUTO-GENERATED //////////////////
////////////////////////////////////////////////////

#![no_std]

elrond_wasm_node::wasm_endpoints! {
    stablecoin_v3
    (
        calculateFeeRewards
        claimFeeRewards
        deployStablecoin
        getBaseCollateralTokenId
        getBasePool
        getCollateralSupply
        getCpTokenId
        getCpTokenSupply
        getDivisionSafetyConstant
        getLastReplenishBlock
        getMedianPoolDelta
        getPoolDelta
        getPoolRecoveryPeriod
        getPriceAggregatorAddress
        getRewardPerShare
        getRewardReserve
        getSpreadFeeMinPercent
        getStablecoinId
        getStablecoinSupply
        getState
        pause
        provideCollateral
        registerCollateralToken
        registerFarmToken
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
