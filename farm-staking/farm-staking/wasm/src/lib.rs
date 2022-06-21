////////////////////////////////////////////////////
////////////////// AUTO-GENERATED //////////////////
////////////////////////////////////////////////////

#![no_std]

elrond_wasm_node::wasm_endpoints! {
    farm_staking
    (
        callBack
        addAddressToWhitelist
        calculateRewardsForGivenPosition
        claimRewards
        claimRewardsWithNewValue
        compoundRewards
        end_produce_rewards
        getAccumulatedRewards
        getAnnualPercentageRewards
        getBurnGasLimit
        getDivisionSafetyConstant
        getFarmTokenId
        getFarmTokenSupply
        getFarmingTokenId
        getLastRewardBlockNonce
        getLockedAssetFactoryManagedAddress
        getMinUnbondEpochs
        getMinimumFarmingEpoch
        getPairContractManagedAddress
        getPenaltyPercent
        getPerBlockRewardAmount
        getRewardCapacity
        getRewardPerShare
        getRewardTokenId
        getState
        isWhitelisted
        mergeFarmTokens
        pause
        registerFarmToken
        removeAddressFromWhitelist
        resume
        setMaxApr
        setMinUnbondEpochs
        setPerBlockRewardAmount
        set_burn_gas_limit
        set_minimum_farming_epochs
        set_penalty_percent
        stakeFarm
        stakeFarmThroughProxy
        startProduceRewards
        topUpRewards
        unbondFarm
        unstakeFarm
        unstakeFarmThroughProxy
    )
}
