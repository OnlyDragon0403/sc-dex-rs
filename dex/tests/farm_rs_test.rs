use common_structs::FarmTokenAttributes;
use elrond_wasm::types::{
    Address, BigUint, EsdtLocalRole, EsdtTokenPayment, ManagedAddress, OptionalArg, TokenIdentifier,
};
use elrond_wasm_debug::tx_mock::{TxContextStack, TxInputESDT};
use elrond_wasm_debug::{
    managed_address, managed_biguint, managed_token_id, rust_biguint, testing_framework::*,
    DebugApi,
};

type RustBigUint = num_bigint::BigUint;
use migration_from_v1_2::{FarmTokenAttributesV1_2, MigrationModule};

use config::*;
use farm::*;

const GENERATED_FILE_PREFIX: &'static str = "_generated_";
const MANDOS_FILE_EXTENSION: &'static str = ".scen.json";
const FARM_WASM_PATH: &'static str = "farm/output/farm.wasm";

const MEX_TOKEN_ID: &[u8] = b"MEX-abcdef"; // reward token ID
const LP_TOKEN_ID: &[u8] = b"LPTOK-abcdef"; // farming token ID
const FARM_TOKEN_ID: &[u8] = b"FARM-abcdef";
const OLD_FARM_TOKEN_ID: &[u8] = b"OFARM-abcdef";
const DIVISION_SAFETY_CONSTANT: u64 = 1_000_000_000_000;
const MIN_FARMING_EPOCHS: u8 = 2;
const PENALTY_PERCENT: u64 = 10;
const PER_BLOCK_REWARD_AMOUNT: u64 = 5_000;

const USER_TOTAL_LP_TOKENS: u64 = 5_000_000_000;

#[allow(dead_code)] // owner_address is unused, at least for now
struct FarmSetup<FarmObjBuilder>
where
    FarmObjBuilder: 'static + Copy + Fn() -> farm::ContractObj<DebugApi>,
{
    pub blockchain_wrapper: BlockchainStateWrapper,
    pub owner_address: Address,
    pub user_address: Address,
    pub farm_wrapper: ContractObjWrapper<farm::ContractObj<DebugApi>, FarmObjBuilder>,
}

fn setup_farm<FarmObjBuilder>(farm_builder: FarmObjBuilder) -> FarmSetup<FarmObjBuilder>
where
    FarmObjBuilder: 'static + Copy + Fn() -> farm::ContractObj<DebugApi>,
{
    let rust_zero = rust_biguint!(0u64);
    let mut blockchain_wrapper = BlockchainStateWrapper::new();
    let owner_addr = blockchain_wrapper.create_user_account(&rust_zero);
    let farm_wrapper = blockchain_wrapper.create_sc_account(
        &rust_zero,
        Some(&owner_addr),
        farm_builder,
        FARM_WASM_PATH,
    );

    // init farm contract

    blockchain_wrapper
        .execute_tx(&owner_addr, &farm_wrapper, &rust_zero, |sc| {
            let reward_token_id = managed_token_id!(MEX_TOKEN_ID);
            let farming_token_id = managed_token_id!(LP_TOKEN_ID);
            let division_safety_constant = managed_biguint!(DIVISION_SAFETY_CONSTANT);
            let pair_address = managed_address!(&Address::zero());

            sc.init(
                reward_token_id,
                farming_token_id,
                division_safety_constant,
                pair_address,
            );

            let farm_token_id = managed_token_id!(FARM_TOKEN_ID);
            sc.farm_token_id().set(&farm_token_id);

            sc.per_block_reward_amount()
                .set(&managed_biguint!(PER_BLOCK_REWARD_AMOUNT));
            sc.minimum_farming_epochs().set(&MIN_FARMING_EPOCHS);
            sc.penalty_percent().set(&PENALTY_PERCENT);

            sc.state().set(&State::Active);
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
        farm_wrapper.address_ref(),
        FARM_TOKEN_ID,
        &farm_token_roles[..],
    );

    let farming_token_roles = [EsdtLocalRole::Burn];
    blockchain_wrapper.set_esdt_local_roles(
        farm_wrapper.address_ref(),
        LP_TOKEN_ID,
        &farming_token_roles[..],
    );

    let reward_token_roles = [EsdtLocalRole::Mint];
    blockchain_wrapper.set_esdt_local_roles(
        farm_wrapper.address_ref(),
        MEX_TOKEN_ID,
        &reward_token_roles[..],
    );

    let user_addr = blockchain_wrapper.create_user_account(&rust_biguint!(100_000_000));
    blockchain_wrapper.set_esdt_balance(
        &user_addr,
        LP_TOKEN_ID,
        &rust_biguint!(USER_TOTAL_LP_TOKENS),
    );

    FarmSetup {
        blockchain_wrapper,
        owner_address: owner_addr,
        user_address: user_addr,
        farm_wrapper,
    }
}

fn enter_farm<FarmObjBuilder>(
    farm_setup: &mut FarmSetup<FarmObjBuilder>,
    farm_in_amount: u64,
    additional_farm_tokens: &[TxInputESDT],
    expected_farm_token_nonce: u64,
    expected_reward_per_share: u64,
    expected_original_entering_epoch: u64,
    expected_entering_epoch: u64,
    expected_initial_farming_amount: u64,
    expected_compounded_reward: u64,
) where
    FarmObjBuilder: 'static + Copy + Fn() -> farm::ContractObj<DebugApi>,
{
    let mut payments = Vec::with_capacity(1 + additional_farm_tokens.len());
    payments.push(TxInputESDT {
        token_identifier: LP_TOKEN_ID.to_vec(),
        nonce: 0,
        value: rust_biguint!(farm_in_amount),
    });
    payments.extend_from_slice(additional_farm_tokens);

    let mut expected_total_out_amount = 0;
    for payment in payments.iter() {
        expected_total_out_amount += payment.value.to_u64_digits()[0];
    }

    let b_mock = &mut farm_setup.blockchain_wrapper;
    b_mock
        .execute_esdt_multi_transfer(
            &farm_setup.user_address,
            &farm_setup.farm_wrapper,
            &payments,
            |sc| {
                let payment = sc.enter_farm(OptionalArg::None);
                assert_eq!(payment.token_identifier, managed_token_id!(FARM_TOKEN_ID));
                assert_eq!(payment.token_nonce, expected_farm_token_nonce);
                assert_eq!(payment.amount, managed_biguint!(expected_total_out_amount));

                StateChange::Commit
            },
        )
        .assert_ok();

    let _ = DebugApi::dummy();

    let mut sc_call = ScCallMandos::new(
        &farm_setup.user_address,
        farm_setup.farm_wrapper.address_ref(),
        "enterFarm",
    );
    sc_call.add_esdt_transfer(LP_TOKEN_ID, 0, &rust_biguint!(farm_in_amount));

    let mut tx_expect = TxExpectMandos::new(0);
    tx_expect.add_out_value(&expected_farm_token_nonce);
    tx_expect.add_out_value(&EsdtTokenPayment::<DebugApi> {
        token_type: elrond_wasm::types::EsdtTokenType::SemiFungible,
        token_identifier: managed_token_id!(FARM_TOKEN_ID),
        token_nonce: expected_farm_token_nonce,
        amount: managed_biguint!(farm_in_amount),
    });

    b_mock.add_mandos_sc_call(sc_call, Some(tx_expect));

    let expected_attributes = FarmTokenAttributes::<DebugApi> {
        reward_per_share: managed_biguint!(expected_reward_per_share),
        original_entering_epoch: expected_original_entering_epoch,
        entering_epoch: expected_entering_epoch,
        initial_farming_amount: managed_biguint!(expected_initial_farming_amount),
        compounded_reward: managed_biguint!(expected_compounded_reward),
        current_farm_amount: managed_biguint!(expected_total_out_amount),
    };
    b_mock.check_nft_balance(
        &farm_setup.user_address,
        FARM_TOKEN_ID,
        expected_farm_token_nonce,
        &rust_biguint!(expected_total_out_amount),
        &expected_attributes,
    );

    let _ = TxContextStack::static_pop();
}

fn exit_farm<FarmObjBuilder>(
    farm_setup: &mut FarmSetup<FarmObjBuilder>,
    farm_token_amount: u64,
    farm_token_nonce: u64,
    expected_mex_out: u64,
    expected_user_mex_balance: &RustBigUint,
    expected_user_lp_token_balance: &RustBigUint,
) where
    FarmObjBuilder: 'static + Copy + Fn() -> farm::ContractObj<DebugApi>,
{
    let b_mock = &mut farm_setup.blockchain_wrapper;
    b_mock
        .execute_esdt_transfer(
            &farm_setup.user_address,
            &farm_setup.farm_wrapper,
            FARM_TOKEN_ID,
            farm_token_nonce,
            &rust_biguint!(farm_token_amount),
            |sc| {
                let multi_result = sc.exit_farm(OptionalArg::None);

                let (first_result, second_result) = multi_result.into_tuple();

                assert_eq!(
                    first_result.token_identifier,
                    managed_token_id!(LP_TOKEN_ID)
                );
                assert_eq!(first_result.token_nonce, 0);
                assert_eq!(first_result.amount, managed_biguint!(farm_token_amount));

                assert_eq!(
                    second_result.token_identifier,
                    managed_token_id!(MEX_TOKEN_ID)
                );
                assert_eq!(second_result.token_nonce, 0);
                assert_eq!(second_result.amount, managed_biguint!(expected_mex_out));

                StateChange::Commit
            },
        )
        .assert_ok();

    b_mock.check_esdt_balance(
        &farm_setup.user_address,
        MEX_TOKEN_ID,
        expected_user_mex_balance,
    );
    b_mock.check_esdt_balance(
        &farm_setup.user_address,
        LP_TOKEN_ID,
        expected_user_lp_token_balance,
    );
}

fn claim_rewards<FarmObjBuilder>(
    farm_setup: &mut FarmSetup<FarmObjBuilder>,
    farm_token_amount: u64,
    farm_token_nonce: u64,
    expected_mex_out: u64,
    expected_user_mex_balance: &RustBigUint,
    expected_user_lp_token_balance: &RustBigUint,
    expected_farm_token_nonce_out: u64,
    expected_reward_per_share: u64,
) where
    FarmObjBuilder: 'static + Copy + Fn() -> farm::ContractObj<DebugApi>,
{
    let b_mock = &mut farm_setup.blockchain_wrapper;
    b_mock
        .execute_esdt_transfer(
            &farm_setup.user_address,
            &farm_setup.farm_wrapper,
            FARM_TOKEN_ID,
            farm_token_nonce,
            &rust_biguint!(farm_token_amount),
            |sc| {
                let multi_result = sc.claim_rewards(OptionalArg::None);

                let (first_result, second_result) = multi_result.into_tuple();

                assert_eq!(
                    first_result.token_identifier,
                    managed_token_id!(FARM_TOKEN_ID)
                );
                assert_eq!(first_result.token_nonce, expected_farm_token_nonce_out);
                assert_eq!(first_result.amount, managed_biguint!(farm_token_amount));

                assert_eq!(
                    second_result.token_identifier,
                    managed_token_id!(MEX_TOKEN_ID)
                );
                assert_eq!(second_result.token_nonce, 0);
                assert_eq!(second_result.amount, managed_biguint!(expected_mex_out));

                StateChange::Commit
            },
        )
        .assert_ok();

    let _ = DebugApi::dummy();
    let expected_attributes = FarmTokenAttributes::<DebugApi> {
        reward_per_share: managed_biguint!(expected_reward_per_share),
        original_entering_epoch: 0,
        entering_epoch: 0,
        initial_farming_amount: managed_biguint!(farm_token_amount),
        compounded_reward: managed_biguint!(0),
        current_farm_amount: managed_biguint!(farm_token_amount),
    };

    b_mock.check_nft_balance(
        &farm_setup.user_address,
        FARM_TOKEN_ID,
        expected_farm_token_nonce_out,
        &rust_biguint!(farm_token_amount),
        &expected_attributes,
    );
    b_mock.check_esdt_balance(
        &farm_setup.user_address,
        MEX_TOKEN_ID,
        expected_user_mex_balance,
    );
    b_mock.check_esdt_balance(
        &farm_setup.user_address,
        LP_TOKEN_ID,
        expected_user_lp_token_balance,
    );

    let _ = TxContextStack::static_pop();
}

fn check_farm_token_supply<FarmObjBuilder>(
    farm_setup: &mut FarmSetup<FarmObjBuilder>,
    expected_farm_token_supply: u64,
) where
    FarmObjBuilder: 'static + Copy + Fn() -> farm::ContractObj<DebugApi>,
{
    let b_mock = &mut farm_setup.blockchain_wrapper;
    b_mock
        .execute_query(&farm_setup.farm_wrapper, |sc| {
            let actual_farm_supply = sc.farm_token_supply().get();
            assert_eq!(
                managed_biguint!(expected_farm_token_supply),
                actual_farm_supply
            );
        })
        .assert_ok();
}

fn set_block_nonce<FarmObjBuilder>(farm_setup: &mut FarmSetup<FarmObjBuilder>, block_nonce: u64)
where
    FarmObjBuilder: 'static + Copy + Fn() -> farm::ContractObj<DebugApi>,
{
    farm_setup.blockchain_wrapper.set_block_nonce(block_nonce);
}

fn set_block_epoch<FarmObjBuilder>(farm_setup: &mut FarmSetup<FarmObjBuilder>, block_epoch: u64)
where
    FarmObjBuilder: 'static + Copy + Fn() -> farm::ContractObj<DebugApi>,
{
    farm_setup.blockchain_wrapper.set_block_epoch(block_epoch);
}

fn create_generated_mandos_file_name(suffix: &str) -> String {
    let mut path = GENERATED_FILE_PREFIX.to_owned();
    path += suffix;
    path += MANDOS_FILE_EXTENSION;

    path
}

#[test]
fn test_farm_setup() {
    let farm_setup = setup_farm(farm::contract_obj);
    let file_name = create_generated_mandos_file_name("init");

    farm_setup
        .blockchain_wrapper
        .write_mandos_output(&file_name);
}

#[test]
fn test_enter_farm() {
    let mut farm_setup = setup_farm(farm::contract_obj);

    let farm_in_amount = 100_000_000;
    let expected_farm_token_nonce = 1;
    enter_farm(
        &mut farm_setup,
        farm_in_amount,
        &[],
        expected_farm_token_nonce,
        0,
        0,
        0,
        farm_in_amount,
        0,
    );
    check_farm_token_supply(&mut farm_setup, farm_in_amount);

    let file_name = create_generated_mandos_file_name("enter_farm");
    farm_setup
        .blockchain_wrapper
        .write_mandos_output(&file_name);
}

#[test]
fn test_exit_farm() {
    let mut farm_setup = setup_farm(farm::contract_obj);

    let farm_in_amount = 100_000_000;
    let expected_farm_token_nonce = 1;
    enter_farm(
        &mut farm_setup,
        farm_in_amount,
        &[],
        expected_farm_token_nonce,
        0,
        0,
        0,
        farm_in_amount,
        0,
    );
    check_farm_token_supply(&mut farm_setup, farm_in_amount);

    set_block_epoch(&mut farm_setup, 5);
    set_block_nonce(&mut farm_setup, 10);

    let expected_mex_out = 10 * PER_BLOCK_REWARD_AMOUNT;
    let expected_lp_token_balance = rust_biguint!(USER_TOTAL_LP_TOKENS);
    exit_farm(
        &mut farm_setup,
        farm_in_amount,
        expected_farm_token_nonce,
        expected_mex_out,
        &rust_biguint!(expected_mex_out),
        &expected_lp_token_balance,
    );
    check_farm_token_supply(&mut farm_setup, 0);
}

#[test]
fn test_claim_rewards() {
    let mut farm_setup = setup_farm(farm::contract_obj);

    let farm_in_amount = 100_000_000;
    let expected_farm_token_nonce = 1;
    enter_farm(
        &mut farm_setup,
        farm_in_amount,
        &[],
        expected_farm_token_nonce,
        0,
        0,
        0,
        farm_in_amount,
        0,
    );
    check_farm_token_supply(&mut farm_setup, farm_in_amount);

    set_block_epoch(&mut farm_setup, 5);
    set_block_nonce(&mut farm_setup, 10);

    let expected_mex_out = 10 * PER_BLOCK_REWARD_AMOUNT;
    let expected_lp_token_balance = rust_biguint!(USER_TOTAL_LP_TOKENS - farm_in_amount);
    let expected_reward_per_share = 500_000_000;
    claim_rewards(
        &mut farm_setup,
        farm_in_amount,
        expected_farm_token_nonce,
        expected_mex_out,
        &rust_biguint!(expected_mex_out),
        &expected_lp_token_balance,
        expected_farm_token_nonce + 1,
        expected_reward_per_share,
    );
    check_farm_token_supply(&mut farm_setup, farm_in_amount);
}

fn steps_enter_farm_twice<FarmObjBuilder>(farm_builder: FarmObjBuilder) -> FarmSetup<FarmObjBuilder>
where
    FarmObjBuilder: 'static + Copy + Fn() -> farm::ContractObj<DebugApi>,
{
    let mut farm_setup = setup_farm(farm_builder);

    let farm_in_amount = 100_000_000;
    let expected_farm_token_nonce = 1;
    enter_farm(
        &mut farm_setup,
        farm_in_amount,
        &[],
        expected_farm_token_nonce,
        0,
        0,
        0,
        farm_in_amount,
        0,
    );
    check_farm_token_supply(&mut farm_setup, farm_in_amount);

    set_block_epoch(&mut farm_setup, 5);
    set_block_nonce(&mut farm_setup, 10);

    let second_farm_in_amount = 200_000_000;
    let prev_farm_tokens = [TxInputESDT {
        token_identifier: FARM_TOKEN_ID.to_vec(),
        nonce: expected_farm_token_nonce,
        value: rust_biguint!(farm_in_amount),
    }];
    let current_farm_supply = farm_in_amount;

    let total_amount = farm_in_amount + second_farm_in_amount;
    let first_reward_share = 0;
    let second_reward_share =
        0 + DIVISION_SAFETY_CONSTANT * 10 * PER_BLOCK_REWARD_AMOUNT / current_farm_supply;
    let expected_reward_per_share = (first_reward_share * farm_in_amount
        + second_reward_share * second_farm_in_amount
        + total_amount
        - 1)
        / total_amount;

    enter_farm(
        &mut farm_setup,
        second_farm_in_amount,
        &prev_farm_tokens,
        expected_farm_token_nonce + 1,
        expected_reward_per_share,
        5,
        5,
        total_amount,
        0,
    );
    check_farm_token_supply(&mut farm_setup, total_amount);

    farm_setup
}

#[test]
fn test_enter_farm_twice() {
    let _ = steps_enter_farm_twice(farm::contract_obj);
}

#[test]
fn test_exit_farm_after_enter_twice() {
    let mut farm_setup = steps_enter_farm_twice(farm::contract_obj);
    let farm_in_amount = 100_000_000;
    let second_farm_in_amount = 200_000_000;
    let total_farm_token = farm_in_amount + second_farm_in_amount;
    let expected_user_lp_balance = rust_biguint!(USER_TOTAL_LP_TOKENS);

    set_block_epoch(&mut farm_setup, 8);
    set_block_nonce(&mut farm_setup, 25);

    let current_farm_supply = farm_in_amount;

    let first_reward_share = 0;
    let second_reward_share =
        0 + DIVISION_SAFETY_CONSTANT * 10 * PER_BLOCK_REWARD_AMOUNT / current_farm_supply;
    let prev_reward_per_share = (first_reward_share * farm_in_amount
        + second_reward_share * second_farm_in_amount
        + total_farm_token
        - 1)
        / total_farm_token;
    let new_reward_per_share = prev_reward_per_share
        + 25 * PER_BLOCK_REWARD_AMOUNT * DIVISION_SAFETY_CONSTANT / total_farm_token;
    let reward_per_share_diff = new_reward_per_share - prev_reward_per_share;

    let expected_reward_amount =
        total_farm_token * reward_per_share_diff / DIVISION_SAFETY_CONSTANT;
    exit_farm(
        &mut farm_setup,
        total_farm_token,
        2,
        expected_reward_amount,
        &rust_biguint!(expected_reward_amount),
        &expected_user_lp_balance,
    );
    check_farm_token_supply(&mut farm_setup, 0);
}

fn set_migration_config<FarmObjBuilder>(farm_setup: &mut FarmSetup<FarmObjBuilder>)
where
    FarmObjBuilder: 'static + Copy + Fn() -> farm::ContractObj<DebugApi>,
{
    let owner = farm_setup.owner_address.clone();
    let own = farm_setup.farm_wrapper.address_ref().clone();

    let b_mock = &mut farm_setup.blockchain_wrapper;
    b_mock
        .execute_tx(
            &farm_setup.user_address,
            &farm_setup.farm_wrapper,
            &rust_biguint!(0),
            |sc| {
                sc.set_farm_migration_config(
                    managed_address!(&owner),
                    managed_token_id!(OLD_FARM_TOKEN_ID),
                    managed_address!(&own),
                    managed_address!(&owner),
                );
                StateChange::Commit
            },
        )
        .assert_ok();
}

fn do_basic_migration<FarmObjBuilder>(farm_setup: &mut FarmSetup<FarmObjBuilder>)
where
    FarmObjBuilder: 'static + Copy + Fn() -> farm::ContractObj<DebugApi>,
{
    let owner = farm_setup.owner_address.clone();
    let own = farm_setup.farm_wrapper.address_ref().clone();

    let farm_token_roles = [EsdtLocalRole::NftBurn];
    farm_setup.blockchain_wrapper.set_esdt_local_roles(
        &own,
        OLD_FARM_TOKEN_ID,
        &farm_token_roles[..],
    );

    let b_mock = &mut farm_setup.blockchain_wrapper;
    let _ = DebugApi::dummy();

    let nft_balance = rust_biguint!(1_000);
    let nft_attributes: FarmTokenAttributesV1_2<DebugApi> = FarmTokenAttributesV1_2 {
        reward_per_share: managed_biguint!(1_000),
        original_entering_epoch: 10,
        entering_epoch: 10,
        apr_multiplier: 0,
        with_locked_rewards: false,
        initial_farming_amount: managed_biguint!(1_000),
        compounded_reward: managed_biguint!(0),
        current_farm_amount: managed_biguint!(1_000),
    };

    b_mock.set_nft_balance(&owner, OLD_FARM_TOKEN_ID, 1, &nft_balance, &nft_attributes);
    b_mock.set_esdt_balance(&owner, LP_TOKEN_ID, &nft_balance);
    b_mock.set_esdt_balance(&owner, MEX_TOKEN_ID, &nft_balance);

    let payments = vec![TxInputESDT {
        token_identifier: LP_TOKEN_ID.to_vec(),
        nonce: 0,
        value: nft_balance.clone(),
    }];

    b_mock
        .execute_esdt_multi_transfer(&owner, &farm_setup.farm_wrapper, &payments, |sc| {
            sc.migrate_from_v1_2_farm(nft_attributes, managed_address!(&owner));

            StateChange::Commit
        })
        .assert_ok();

    b_mock.check_nft_balance(
        &farm_setup.owner_address,
        FARM_TOKEN_ID,
        1,
        &nft_balance,
        &FarmTokenAttributes::<DebugApi> {
            reward_per_share: managed_biguint!(1_000),
            original_entering_epoch: 10,
            entering_epoch: 10,
            initial_farming_amount: managed_biguint!(1_000),
            compounded_reward: managed_biguint!(0),
            current_farm_amount: managed_biguint!(1_000),
        },
    );

    let _ = TxContextStack::static_pop();
}

#[test]
fn test_migration() {
    let mut farm_setup = setup_farm(farm::contract_obj);

    set_migration_config(&mut farm_setup);

    do_basic_migration(&mut farm_setup);
}
