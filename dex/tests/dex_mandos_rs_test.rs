use elrond_wasm::*;
use elrond_wasm_debug::*;

fn contract_map() -> ContractMap<DebugApi> {
    let mut contract_map = ContractMap::new();
    contract_map.register_contract(
        "file:../farm/output/farm.wasm",
        Box::new(|context| Box::new(farm::contract_obj(context))),
    );
    contract_map.register_contract(
        "file:../pair/output/pair.wasm",
        Box::new(|context| Box::new(pair::contract_obj(context))),
    );
    contract_map.register_contract(
        "file:../router/output/router.wasm",
        Box::new(|context| Box::new(router::contract_obj(context))),
    );
    contract_map
}

#[test]
fn add_liquidity_rs() {
    elrond_wasm_debug::mandos_rs("mandos/add_liquidity.scen.json", contract_map());
}

#[test]
fn calculate_rewards_for_given_position_after_compound_rs() {
    elrond_wasm_debug::mandos_rs(
        "mandos/calculate_rewards_for_given_position_after_compound.scen.json",
        contract_map(),
    );
}

#[test]
fn calculate_rewards_for_given_position_rs() {
    elrond_wasm_debug::mandos_rs(
        "mandos/calculate_rewards_for_given_position.scen.json",
        contract_map(),
    );
}

#[test]
fn check_fee_disabled_after_swap_rs() {
    elrond_wasm_debug::mandos_rs(
        "mandos/check_fee_disabled_after_swap.scen.json",
        contract_map(),
    );
}

#[test]
fn check_fee_enabled_after_swap_rs() {
    elrond_wasm_debug::mandos_rs(
        "mandos/check_fee_enabled_after_swap.scen.json",
        contract_map(),
    );
}

#[test]
fn claim_rewards_rs() {
    elrond_wasm_debug::mandos_rs("mandos/claim_rewards.scen.json", contract_map());
}

#[test]
fn complete_setup_rs() {
    elrond_wasm_debug::mandos_rs("mandos/complete_setup.scen.json", contract_map());
}

#[test]
fn compound_rewards_setup_rs() {
    elrond_wasm_debug::mandos_rs("mandos/compound_rewards.scen.json", contract_map());
}

#[test]
fn create_pair_twice_rs() {
    elrond_wasm_debug::mandos_rs("mandos/create_pair_twice.scen.json", contract_map());
}

#[test]
fn enter_farm_rs() {
    elrond_wasm_debug::mandos_rs("mandos/enter_farm.scen.json", contract_map());
}

#[test]
fn enter_mex_farm_rs() {
    elrond_wasm_debug::mandos_rs("mandos/enter_mex_farm.scen.json", contract_map());
}

#[test]
fn exit_farm_rs() {
    elrond_wasm_debug::mandos_rs("mandos/exit_farm.scen.json", contract_map());
}

#[test]
fn exit_farm_too_soon_rs() {
    elrond_wasm_debug::mandos_rs("mandos/exit_farm_too_soon.scen.json", contract_map());
}

#[test]
fn exit_mex_farm_rs() {
    elrond_wasm_debug::mandos_rs("mandos/exit_mex_farm.scen.json", contract_map());
}

#[test]
fn farm_reward_distr_scen_1_rs() {
    elrond_wasm_debug::mandos_rs("mandos/farm_reward_distr_scen_1.scen.json", contract_map());
}

#[test]
fn farm_reward_distr_scen_2_rs() {
    elrond_wasm_debug::mandos_rs("mandos/farm_reward_distr_scen_2.scen.json", contract_map());
}

#[test]
fn farm_reward_distr_scen_3_rs() {
    elrond_wasm_debug::mandos_rs("mandos/farm_reward_distr_scen_3.scen.json", contract_map());
}

#[test]
fn farm_reward_distr_scen_4_rs() {
    elrond_wasm_debug::mandos_rs("mandos/farm_reward_distr_scen_4.scen.json", contract_map());
}

#[test]
fn farm_reward_distr_scen_5_rs() {
    elrond_wasm_debug::mandos_rs("mandos/farm_reward_distr_scen_5.scen.json", contract_map());
}

#[test]
fn farm_with_egld_token_rs() {
    elrond_wasm_debug::mandos_rs("mandos/farm_with_egld_token.scen.json", contract_map());
}

#[test]
fn farm_wrong_lp_token_rs() {
    elrond_wasm_debug::mandos_rs("mandos/farm_wrong_lp_token.scen.json", contract_map());
}

#[test]
fn get_amounts_rs() {
    elrond_wasm_debug::mandos_rs("mandos/get_amounts.scen.json", contract_map());
}

#[test]
fn get_amounts_no_liquidity_rs() {
    elrond_wasm_debug::mandos_rs("mandos/get_amounts_no_liquidity.scen.json", contract_map());
}

#[test]
fn get_pair_non_existent_rs() {
    elrond_wasm_debug::mandos_rs("mandos/get_pair_non_existent.scen.json", contract_map());
}

#[test]
fn get_pair_views_rs() {
    elrond_wasm_debug::mandos_rs("mandos/get_pair_views.scen.json", contract_map());
}

#[test]
fn merge_tokens_rs() {
    elrond_wasm_debug::mandos_rs("mandos/merge_tokens.scen.json", contract_map());
}

#[test]
fn owner_pause_farm_rs() {
    elrond_wasm_debug::mandos_rs("mandos/owner_pause_farm.scen.json", contract_map());
}

#[test]
fn owner_resume_farm_rs() {
    elrond_wasm_debug::mandos_rs("mandos/owner_resume_farm.scen.json", contract_map());
}

#[test]
fn remove_liquidity_rs() {
    elrond_wasm_debug::mandos_rs("mandos/remove_liquidity.scen.json", contract_map());
}

#[test]
fn remove_liquidity_twice_rs() {
    elrond_wasm_debug::mandos_rs("mandos/remove_liquidity_twice.scen.json", contract_map());
}

#[test]
fn router_pause_self_rs() {
    elrond_wasm_debug::mandos_rs("mandos/router_pause_self.scen.json", contract_map());
}

#[test]
fn router_resume_self_rs() {
    elrond_wasm_debug::mandos_rs("mandos/router_resume_self.scen.json", contract_map());
}

#[test]
fn swap_fixed_input_rs() {
    elrond_wasm_debug::mandos_rs("mandos/swap_fixed_input.scen.json", contract_map());
}

#[test]
fn swap_fixed_input_after_removed_liquidity_rs() {
    elrond_wasm_debug::mandos_rs(
        "mandos/swap_fixed_input_after_removed_liquidity.scen.json",
        contract_map(),
    );
}

#[test]
fn swap_fixed_output_rs() {
    elrond_wasm_debug::mandos_rs("mandos/swap_fixed_output.scen.json", contract_map());
}

#[test]
fn swap_same_token_rs() {
    elrond_wasm_debug::mandos_rs("mandos/swap_same_token.scen.json", contract_map());
}

#[test]
fn swap_wrong_token_rs() {
    elrond_wasm_debug::mandos_rs("mandos/swap_wrong_token.scen.json", contract_map());
}

#[test]
fn upgrade_contract_rs() {
    elrond_wasm_debug::mandos_rs("mandos/upgrade_contract.scen.json", contract_map());
}
