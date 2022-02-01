use elrond_wasm::types::{
    Address, BigUint, EsdtLocalRole, EsdtTokenPayment, ManagedAddress, ManagedVec, TokenIdentifier,
};
use elrond_wasm_debug::{
    managed_address, managed_biguint, managed_token_id, rust_biguint,
    testing_framework::{BlockchainStateWrapper, ContractObjWrapper, StateChange},
    DebugApi,
};

use ::config as farm_staking_config;
use farm_staking::whitelist::WhitelistModule;
use farm_staking::*;
use farm_staking_config::ConfigModule as _;

use farm_staking_proxy::dual_yield_token::DualYieldTokenModule;
use farm_staking_proxy::*;

use crate::constants::*;

pub fn setup_staking_farm<StakingContractObjBuilder>(
    owner_addr: &Address,
    blockchain_wrapper: &mut BlockchainStateWrapper,
    builder: StakingContractObjBuilder,
) -> ContractObjWrapper<farm_staking::ContractObj<DebugApi>, StakingContractObjBuilder>
where
    StakingContractObjBuilder: 'static + Copy + Fn() -> farm_staking::ContractObj<DebugApi>,
{
    let rust_zero = rust_biguint!(0u64);
    let farm_staking_wrapper = blockchain_wrapper.create_sc_account(
        &rust_zero,
        Some(owner_addr),
        builder,
        PROXY_WASM_PATH,
    );

    blockchain_wrapper
        .execute_tx(&owner_addr, &farm_staking_wrapper, &rust_zero, |sc| {
            let reward_token_id = managed_token_id!(STAKING_REWARD_TOKEN_ID);
            let farming_token_id = managed_token_id!(STAKING_TOKEN_ID);
            let div_const = managed_biguint!(DIVISION_SAFETY_CONSTANT);
            let max_apr = managed_biguint!(MAX_APR);

            sc.init(
                reward_token_id,
                farming_token_id,
                div_const,
                max_apr,
                UNBOND_EPOCHS,
            );

            sc.farm_token_id()
                .set(&managed_token_id!(STAKING_FARM_TOKEN_ID));
            sc.state().set(&farm_staking_config::State::Active);
            sc.produce_rewards_enabled().set(&true);

            StateChange::Commit
        })
        .assert_ok();

    let farm_token_roles = [
        EsdtLocalRole::NftCreate,
        EsdtLocalRole::NftAddQuantity,
        EsdtLocalRole::NftBurn,
    ];
    blockchain_wrapper.set_esdt_local_roles(
        farm_staking_wrapper.address_ref(),
        STAKING_FARM_TOKEN_ID,
        &farm_token_roles[..],
    );

    farm_staking_wrapper
}

pub fn add_proxy_to_whitelist<StakingContractObjBuilder>(
    owner_addr: &Address,
    proxy_address: &Address,
    blockchain_wrapper: &mut BlockchainStateWrapper,
    staking_farm_builder: &ContractObjWrapper<
        farm_staking::ContractObj<DebugApi>,
        StakingContractObjBuilder,
    >,
) where
    StakingContractObjBuilder: 'static + Copy + Fn() -> farm_staking::ContractObj<DebugApi>,
{
    let rust_zero = rust_biguint!(0u64);
    blockchain_wrapper
        .execute_tx(owner_addr, staking_farm_builder, &rust_zero, |sc| {
            sc.add_address_to_whitelist(managed_address!(proxy_address));

            StateChange::Commit
        })
        .assert_ok();
}

pub fn setup_proxy<ProxyContractObjBuilder>(
    owner_addr: &Address,
    lp_farm_address: &Address,
    staking_farm_address: &Address,
    pair_address: &Address,
    blockchain_wrapper: &mut BlockchainStateWrapper,
    builder: ProxyContractObjBuilder,
) -> ContractObjWrapper<farm_staking_proxy::ContractObj<DebugApi>, ProxyContractObjBuilder>
where
    ProxyContractObjBuilder: 'static + Copy + Fn() -> farm_staking_proxy::ContractObj<DebugApi>,
{
    let rust_zero = rust_biguint!(0u64);
    let farm_staking_wrapper = blockchain_wrapper.create_sc_account(
        &rust_zero,
        Some(owner_addr),
        builder,
        STAKING_FARM_WASM_PATH,
    );

    blockchain_wrapper
        .execute_tx(&owner_addr, &farm_staking_wrapper, &rust_zero, |sc| {
            sc.init(
                managed_address!(lp_farm_address),
                managed_address!(staking_farm_address),
                managed_address!(pair_address),
                managed_token_id!(STAKING_TOKEN_ID),
                managed_token_id!(LP_FARM_TOKEN_ID),
                managed_token_id!(STAKING_FARM_TOKEN_ID),
            );

            sc.dual_yield_token_id()
                .set(&managed_token_id!(DUAL_YIELD_TOKEN_ID));

            StateChange::Commit
        })
        .assert_ok();

    let dual_yield_token_roles = [
        EsdtLocalRole::NftCreate,
        EsdtLocalRole::NftAddQuantity,
        EsdtLocalRole::NftBurn,
    ];
    blockchain_wrapper.set_esdt_local_roles(
        farm_staking_wrapper.address_ref(),
        DUAL_YIELD_TOKEN_ID,
        &dual_yield_token_roles[..],
    );

    farm_staking_wrapper
}
