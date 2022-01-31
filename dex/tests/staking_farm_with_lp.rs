use elrond_wasm_debug::{rust_biguint, testing_framework::BlockchainStateWrapper};

mod staking_farm_with_lp_external_contracts;
use staking_farm_with_lp_external_contracts::*;

#[test]
fn test_pair_setup() {
    let rust_zero = rust_biguint!(0u64);
    let mut wrapper = BlockchainStateWrapper::new();
    let owner_addr = wrapper.create_user_account(&rust_zero);
    let user_addr = wrapper.create_user_account(&rust_biguint!(100_000_000));

    let _ = setup_pair(&owner_addr, &user_addr, &mut wrapper, pair::contract_obj);
}

#[test]
fn test_lp_farm_setup() {
    let rust_zero = rust_biguint!(0u64);
    let mut wrapper = BlockchainStateWrapper::new();
    let owner_addr = wrapper.create_user_account(&rust_zero);
    let user_addr = wrapper.create_user_account(&rust_biguint!(100_000_000));

    let _ = setup_lp_farm(
        &owner_addr,
        &user_addr,
        &mut wrapper,
        farm::contract_obj,
        USER_TOTAL_LP_TOKENS,
    );
}

#[test]
fn setup_pair_and_farm() {
    let rust_zero = rust_biguint!(0u64);
    let mut wrapper = BlockchainStateWrapper::new();
    let owner_addr = wrapper.create_user_account(&rust_zero);
    let user_addr = wrapper.create_user_account(&rust_biguint!(100_000_000));

    let _pair_wrapper = setup_pair(&owner_addr, &user_addr, &mut wrapper, pair::contract_obj);
    let _lp_farm_wrapper = setup_lp_farm(
        &owner_addr,
        &user_addr,
        &mut wrapper,
        farm::contract_obj,
        USER_TOTAL_LP_TOKENS,
    );
}
