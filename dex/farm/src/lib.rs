#![no_std]
#![allow(clippy::too_many_arguments)]
#![feature(exact_size_is_empty)]

pub mod custom_rewards;
pub mod farm_token_merge;

use common_errors::*;
use common_macros::assert;
use common_structs::{Epoch, FarmTokenAttributes};
use config::State;
use contexts::generic::{GenericContext, StorageCache};
use farm_token::FarmToken;

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use config::{
    DEFAULT_BURN_GAS_LIMIT, DEFAULT_MINUMUM_FARMING_EPOCHS, DEFAULT_PENALTY_PERCENT,
    DEFAULT_TRANSFER_EXEC_GAS_LIMIT, MAX_PENALTY_PERCENT,
};

type EnterFarmResultType<BigUint> = EsdtTokenPayment<BigUint>;
type CompoundRewardsResultType<BigUint> = EsdtTokenPayment<BigUint>;
type ClaimRewardsResultType<BigUint> =
    MultiResult2<EsdtTokenPayment<BigUint>, EsdtTokenPayment<BigUint>>;
type ExitFarmResultType<BigUint> =
    MultiResult2<EsdtTokenPayment<BigUint>, EsdtTokenPayment<BigUint>>;

#[elrond_wasm::contract]
pub trait Farm:
    custom_rewards::CustomRewardsModule
    + rewards::RewardsModule
    + config::ConfigModule
    + token_send::TokenSendModule
    + token_merge::TokenMergeModule
    + farm_token::FarmTokenModule
    + farm_token_merge::FarmTokenMergeModule
    + events::EventsModule
    + contexts::ctx_helper::CtxHelper
{
    #[proxy]
    fn pair_contract_proxy(&self, to: ManagedAddress) -> pair::Proxy<Self::Api>;

    #[init]
    fn init(
        &self,
        reward_token_id: TokenIdentifier,
        farming_token_id: TokenIdentifier,
        division_safety_constant: BigUint,
        pair_contract_address: ManagedAddress,
    ) {
        assert!(self, reward_token_id.is_esdt(), ERROR_NOT_AN_ESDT);
        assert!(self, farming_token_id.is_esdt(), ERROR_NOT_AN_ESDT);
        assert!(self, division_safety_constant != 0u64, ERROR_ZERO_AMOUNT);
        let farm_token = self.farm_token_id().get();
        assert!(self, reward_token_id != farm_token, ERROR_SAME_TOKEN_IDS);
        assert!(self, farming_token_id != farm_token, ERROR_SAME_TOKEN_IDS);

        self.state().set(&State::Inactive);
        self.penalty_percent()
            .set_if_empty(&DEFAULT_PENALTY_PERCENT);
        self.minimum_farming_epochs()
            .set_if_empty(&DEFAULT_MINUMUM_FARMING_EPOCHS);
        self.transfer_exec_gas_limit()
            .set_if_empty(&DEFAULT_TRANSFER_EXEC_GAS_LIMIT);
        self.burn_gas_limit().set_if_empty(&DEFAULT_BURN_GAS_LIMIT);
        self.division_safety_constant()
            .set_if_empty(&division_safety_constant);

        self.owner().set(&self.blockchain().get_caller());
        self.reward_token_id().set(&reward_token_id);
        self.farming_token_id().set(&farming_token_id);
        self.pair_contract_address().set(&pair_contract_address);
    }

    #[payable("*")]
    #[endpoint(enterFarm)]
    fn enter_farm(
        &self,
        #[var_args] opt_accept_funds_func: OptionalArg<ManagedBuffer>,
    ) -> EnterFarmResultType<Self::Api> {
        let mut context = self.new_farm_context(opt_accept_funds_func);

        self.load_state(&mut context);
        assert!(
            self,
            context.get_contract_state().unwrap() == &State::Active,
            ERROR_NOT_ACTIVE
        );

        self.load_farm_token_id(&mut context);
        assert!(
            self,
            !context.get_farm_token_id().unwrap().is_empty(),
            ERROR_NO_FARM_TOKEN,
        );

        self.load_farming_token_id(&mut context);
        assert!(
            self,
            context.is_accepted_payment_enter(),
            ERROR_BAD_PAYMENTS,
        );

        self.load_reward_token_id(&mut context);
        self.load_reward_reserve(&mut context);
        self.load_block_nonce(&mut context);
        self.load_block_epoch(&mut context);
        self.load_reward_per_share(&mut context);
        self.load_farm_token_supply(&mut context);
        self.load_division_safety_constant(&mut context);
        self.generate_aggregated_rewards(context.get_storage_cache_mut());

        let first_payment_amount = context
            .get_tx_input()
            .get_payments()
            .get_first()
            .amount
            .clone();

        let virtual_position_token_amount = self.create_payment(
            context.get_farm_token_id().unwrap(),
            0,
            &first_payment_amount,
        );
        let virtual_position_attributes = FarmTokenAttributes {
            reward_per_share: context.get_reward_per_share().unwrap().clone(),
            entering_epoch: context.get_block_epoch(),
            original_entering_epoch: context.get_block_epoch(),
            initial_farming_amount: first_payment_amount.clone(),
            compounded_reward: BigUint::zero(),
            current_farm_amount: first_payment_amount.clone(),
        };
        let virtual_position = FarmToken {
            token_amount: virtual_position_token_amount,
            attributes: virtual_position_attributes,
        };

        let (new_farm_token, created_with_merge) = self.create_farm_tokens_by_merging(
            &virtual_position,
            context
                .get_tx_input()
                .get_payments()
                .get_additional()
                .unwrap(),
            context.get_storage_cache(),
        );
        context.set_output_position(new_farm_token, created_with_merge);

        self.commit_changes(&context);
        self.execute_output_payments(&context);
        self.emit_enter_farm_event(&context);

        context
            .get_output_payments()
            .get(0)
            .as_ref()
            .unwrap()
            .clone()
    }

    #[payable("*")]
    #[endpoint(exitFarm)]
    fn exit_farm(
        &self,
        #[var_args] opt_accept_funds_func: OptionalArg<ManagedBuffer>,
    ) -> ExitFarmResultType<Self::Api> {
        let mut context = self.new_farm_context(opt_accept_funds_func);

        self.load_state(&mut context);
        assert!(
            self,
            context.get_contract_state().unwrap() == &State::Active,
            ERROR_NOT_ACTIVE
        );

        self.load_farm_token_id(&mut context);
        assert!(
            self,
            !context.get_farm_token_id().unwrap().is_empty(),
            ERROR_NO_FARM_TOKEN,
        );

        self.load_farming_token_id(&mut context);
        assert!(self, context.is_accepted_payment_exit(), ERROR_BAD_PAYMENTS,);

        self.load_reward_token_id(&mut context);
        self.load_reward_reserve(&mut context);
        self.load_block_nonce(&mut context);
        self.load_block_epoch(&mut context);
        self.load_reward_per_share(&mut context);
        self.load_farm_token_supply(&mut context);
        self.load_division_safety_constant(&mut context);
        self.load_farm_attributes(&mut context);

        self.generate_aggregated_rewards(context.get_storage_cache_mut());
        self.calculate_reward(&mut context);
        context.decrease_reward_reserve();
        self.calculate_initial_farming_amount(&mut context);
        self.increase_reward_with_compounded_rewards(&mut context);

        self.commit_changes(&context);
        self.burn_penalty(&mut context);
        self.burn_position(&context);

        self.send_rewards(&mut context);
        self.construct_output_payments_exit(&mut context);
        self.execute_output_payments(&context);
        self.emit_exit_farm_event(&context);

        self.construct_and_get_result(&context)
    }

    #[payable("*")]
    #[endpoint(claimRewards)]
    fn claim_rewards(
        &self,
        #[var_args] opt_accept_funds_func: OptionalArg<ManagedBuffer>,
    ) -> ClaimRewardsResultType<Self::Api> {
        let mut context = self.new_farm_context(opt_accept_funds_func);

        self.load_state(&mut context);
        assert!(
            self,
            context.get_contract_state().unwrap() == &State::Active,
            ERROR_NOT_ACTIVE
        );

        self.load_farm_token_id(&mut context);
        assert!(
            self,
            !context.get_farm_token_id().unwrap().is_empty(),
            ERROR_NO_FARM_TOKEN,
        );

        self.load_farming_token_id(&mut context);
        assert!(
            self,
            context.is_accepted_payment_claim(),
            ERROR_BAD_PAYMENTS,
        );

        self.load_reward_token_id(&mut context);
        self.load_reward_reserve(&mut context);
        self.load_block_nonce(&mut context);
        self.load_block_epoch(&mut context);
        self.load_reward_per_share(&mut context);
        self.load_farm_token_supply(&mut context);
        self.load_division_safety_constant(&mut context);
        self.load_farm_attributes(&mut context);

        self.generate_aggregated_rewards(context.get_storage_cache_mut());
        self.calculate_reward(&mut context);
        context.decrease_reward_reserve();

        self.calculate_initial_farming_amount(&mut context);
        let new_compound_reward_amount = self.calculate_new_compound_reward_amount(&context);

        let virtual_position_token_amount = EsdtTokenPayment::new(
            context.get_farm_token_id().unwrap().clone(),
            0,
            context
                .get_tx_input()
                .get_payments()
                .get_first()
                .amount
                .clone(),
        );
        let virtual_position_attributes = FarmTokenAttributes {
            reward_per_share: context.get_reward_per_share().unwrap().clone(),
            entering_epoch: context.get_input_attributes().unwrap().entering_epoch,
            original_entering_epoch: context
                .get_input_attributes()
                .unwrap()
                .original_entering_epoch,
            initial_farming_amount: context.get_initial_farming_amount().unwrap().clone(),
            compounded_reward: new_compound_reward_amount,
            current_farm_amount: context
                .get_tx_input()
                .get_payments()
                .get_first()
                .amount
                .clone(),
        };
        let virtual_position = FarmToken {
            token_amount: virtual_position_token_amount,
            attributes: virtual_position_attributes,
        };

        let (new_farm_token, created_with_merge) = self.create_farm_tokens_by_merging(
            &virtual_position,
            context
                .get_tx_input()
                .get_payments()
                .get_additional()
                .unwrap(),
            context.get_storage_cache(),
        );
        context.set_output_position(new_farm_token, created_with_merge);

        self.burn_position(&context);
        self.commit_changes(&context);

        self.send_rewards(&mut context);
        self.execute_output_payments(&context);
        self.emit_claim_rewards_event(&context);

        self.construct_and_get_result(&context)
    }

    #[payable("*")]
    #[endpoint(compoundRewards)]
    fn compound_rewards(
        &self,
        #[var_args] opt_accept_funds_func: OptionalArg<ManagedBuffer>,
    ) -> CompoundRewardsResultType<Self::Api> {
        let mut context = self.new_farm_context(opt_accept_funds_func);

        self.load_state(&mut context);
        assert!(
            self,
            context.get_contract_state().unwrap() == &State::Active,
            ERROR_NOT_ACTIVE
        );

        self.load_farm_token_id(&mut context);
        assert!(
            self,
            !context.get_farm_token_id().unwrap().is_empty(),
            ERROR_NO_FARM_TOKEN,
        );

        self.load_farming_token_id(&mut context);
        self.load_reward_token_id(&mut context);
        assert!(
            self,
            context.is_accepted_payment_compound(),
            ERROR_BAD_PAYMENTS,
        );

        assert!(
            self,
            context.get_farming_token_id().unwrap() == context.get_reward_token_id().unwrap(),
            ERROR_DIFFERENT_TOKEN_IDS
        );

        self.load_reward_per_share(&mut context);
        self.load_reward_reserve(&mut context);
        self.load_block_nonce(&mut context);
        self.load_block_epoch(&mut context);
        self.load_farm_token_supply(&mut context);
        self.load_division_safety_constant(&mut context);
        self.load_farm_attributes(&mut context);

        self.generate_aggregated_rewards(context.get_storage_cache_mut());
        self.calculate_reward(&mut context);
        context.decrease_reward_reserve();
        self.calculate_initial_farming_amount(&mut context);

        let virtual_position_amount = &context.get_tx_input().get_payments().get_first().amount
            + context.get_position_reward().unwrap();
        let virtual_position_token_amount = EsdtTokenPayment::new(
            context.get_farm_token_id().unwrap().clone(),
            0,
            virtual_position_amount,
        );

        let virtual_position_original_entering_epoch = self
            .aggregated_original_entering_epoch_on_compound(
                context.get_farm_token_id().unwrap(),
                &context.get_tx_input().get_payments().get_first().amount,
                context.get_input_attributes().unwrap(),
                context.get_position_reward().unwrap(),
            );
        let virtual_position_compounded_reward = self
            .calculate_new_compound_reward_amount(&context)
            + context.get_position_reward().unwrap();
        let virtual_position_current_farm_amount =
            &context.get_tx_input().get_payments().get_first().amount
                + context.get_position_reward().unwrap();
        let virtual_position_attributes = FarmTokenAttributes {
            reward_per_share: context.get_reward_per_share().unwrap().clone(),
            entering_epoch: context.get_block_epoch(),
            original_entering_epoch: virtual_position_original_entering_epoch,
            initial_farming_amount: context.get_initial_farming_amount().unwrap().clone(),
            compounded_reward: virtual_position_compounded_reward,
            current_farm_amount: virtual_position_current_farm_amount,
        };

        let virtual_position = FarmToken {
            token_amount: virtual_position_token_amount,
            attributes: virtual_position_attributes,
        };

        let (new_farm_token, created_with_merge) = self.create_farm_tokens_by_merging(
            &virtual_position,
            context
                .get_tx_input()
                .get_payments()
                .get_additional()
                .unwrap(),
            context.get_storage_cache(),
        );
        context.set_output_position(new_farm_token, created_with_merge);

        self.burn_position(&context);
        self.commit_changes(&context);

        self.execute_output_payments(&context);

        context.set_final_reward_for_emit_compound_event();
        self.emit_compound_rewards_event(&context);

        context
            .get_output_payments()
            .get(0)
            .as_ref()
            .unwrap()
            .clone()
    }

    fn aggregated_original_entering_epoch_on_compound(
        &self,
        farm_token_id: &TokenIdentifier,
        position_amount: &BigUint,
        position_attributes: &FarmTokenAttributes<Self::Api>,
        reward_amount: &BigUint,
    ) -> u64 {
        if reward_amount == &0 {
            return position_attributes.original_entering_epoch;
        }

        let initial_position = FarmToken {
            token_amount: self.create_payment(farm_token_id, 0, position_amount),
            attributes: position_attributes.clone(),
        };

        let mut reward_position = initial_position.clone();
        reward_position.token_amount.amount = reward_amount.clone();
        reward_position.attributes.original_entering_epoch = self.blockchain().get_block_epoch();

        let mut items = ManagedVec::new();
        items.push(initial_position);
        items.push(reward_position);
        self.aggregated_original_entering_epoch(&items)
    }

    fn burn_farming_tokens(
        &self,
        farming_token_id: &TokenIdentifier,
        farming_amount: &BigUint,
        reward_token_id: &TokenIdentifier,
    ) {
        let pair_contract_address = self.pair_contract_address().get();
        if pair_contract_address.is_zero() {
            self.send()
                .esdt_local_burn(farming_token_id, 0, farming_amount);
        } else {
            let gas_limit = self.burn_gas_limit().get();
            self.pair_contract_proxy(pair_contract_address)
                .remove_liquidity_and_burn_token(
                    farming_token_id.clone(),
                    0,
                    farming_amount.clone(),
                    reward_token_id.clone(),
                )
                .with_gas_limit(gas_limit)
                .transfer_execute();
        }
    }

    fn create_farm_tokens_by_merging(
        &self,
        virtual_position: &FarmToken<Self::Api>,
        additional_positions: &ManagedVec<EsdtTokenPayment<Self::Api>>,
        storage_cache: &StorageCache<Self::Api>,
    ) -> (FarmToken<Self::Api>, bool) {
        let additional_payments_len = additional_positions.len();
        let merged_attributes =
            self.get_merged_farm_token_attributes(additional_positions, Some(virtual_position));

        self.burn_farm_tokens_from_payments(additional_positions);

        let new_amount = merged_attributes.current_farm_amount.clone();
        let new_nonce = self.mint_farm_tokens(
            &storage_cache.farm_token_id.clone().unwrap(),
            &new_amount,
            &merged_attributes,
        );

        let new_farm_token = FarmToken {
            token_amount: self.create_payment(
                &storage_cache.farm_token_id.clone().unwrap(),
                new_nonce,
                &new_amount,
            ),
            attributes: merged_attributes,
        };
        let is_merged = additional_payments_len != 0;

        (new_farm_token, is_merged)
    }

    fn send_back_farming_tokens(
        &self,
        farming_token_id: &TokenIdentifier,
        farming_amount: &BigUint,
        destination: &ManagedAddress,
        opt_accept_funds_func: &OptionalArg<ManagedBuffer>,
    ) {
        self.transfer_execute_custom(
            destination,
            farming_token_id,
            0,
            farming_amount,
            opt_accept_funds_func,
        )
        .unwrap_or_signal_error(self.type_manager());
    }

    fn send_rewards(&self, context: &mut GenericContext<Self::Api>) {
        if context.get_position_reward().unwrap() > &0u64 {
            self.transfer_execute_custom(
                context.get_caller(),
                context.get_reward_token_id().unwrap(),
                0,
                context.get_position_reward().unwrap(),
                context.get_opt_accept_funds_func(),
            )
            .unwrap_or_signal_error(self.type_manager());
        }

        context.set_final_reward(self.create_payment(
            context.get_reward_token_id().unwrap(),
            0,
            context.get_position_reward().unwrap(),
        ));
    }

    #[view(calculateRewardsForGivenPosition)]
    fn calculate_rewards_for_given_position(
        &self,
        amount: BigUint,
        attributes: FarmTokenAttributes<Self::Api>,
    ) -> BigUint {
        assert!(self, amount > 0u64, ERROR_ZERO_AMOUNT);
        let farm_token_supply = self.farm_token_supply().get();
        assert!(self, farm_token_supply >= amount, ERROR_ZERO_AMOUNT);

        let last_reward_nonce = self.last_reward_block_nonce().get();
        let current_block_nonce = self.blockchain().get_block_nonce();
        let reward_increase =
            self.calculate_per_block_rewards(current_block_nonce, last_reward_nonce);
        let reward_per_share_increase = reward_increase * &self.division_safety_constant().get()
            / self.farm_token_supply().get();

        let future_reward_per_share = self.reward_per_share().get() + reward_per_share_increase;
        let mut reward = if future_reward_per_share > attributes.reward_per_share {
            let reward_per_share_diff = future_reward_per_share - attributes.reward_per_share;
            amount * &reward_per_share_diff / self.division_safety_constant().get()
        } else {
            BigUint::zero()
        };

        if self.should_apply_penalty(attributes.entering_epoch) {
            let penalty = self.get_penalty_amount(&reward);
            reward -= penalty;
        }

        reward
    }

    #[inline]
    fn should_apply_penalty(&self, entering_epoch: Epoch) -> bool {
        entering_epoch + self.minimum_farming_epochs().get() as u64
            > self.blockchain().get_block_epoch()
    }

    #[inline]
    fn get_penalty_amount(&self, amount: &BigUint) -> BigUint {
        amount * self.penalty_percent().get() / MAX_PENALTY_PERCENT
    }

    fn burn_penalty(&self, context: &mut GenericContext<Self::Api>) {
        if self.should_apply_penalty(context.get_input_attributes().unwrap().entering_epoch) {
            let penalty_amount =
                self.get_penalty_amount(context.get_initial_farming_amount().unwrap());
            if penalty_amount > 0u64 {
                self.burn_farming_tokens(
                    context.get_farming_token_id().unwrap(),
                    &penalty_amount,
                    context.get_reward_token_id().unwrap(),
                );
                context.decrease_farming_token_amount(&penalty_amount);
            }
        }
    }

    fn burn_position(&self, context: &GenericContext<Self::Api>) {
        let farm_token = context.get_tx_input().get_payments().get_first();
        self.burn_farm_tokens(
            &farm_token.token_identifier,
            farm_token.token_nonce,
            &farm_token.amount,
        );
    }

    fn calculate_new_compound_reward_amount(&self, context: &GenericContext<Self::Api>) -> BigUint {
        self.rule_of_three(
            &context.get_tx_input().get_payments().get_first().amount,
            &context.get_input_attributes().unwrap().current_farm_amount,
            &context.get_input_attributes().unwrap().compounded_reward,
        )
    }

    //This function is neccessary in old farms.
    #[payable("*")]
    #[endpoint(migrateToNewFarm)]
    fn migrate_to_new_farm(
        &self,
        #[var_args] orig_caller_opt: OptionalArg<ManagedAddress>,
    ) -> EsdtTokenPayment<Self::Api> {
        assert!(self, !self.farm_configuration().is_empty(), b"empty config");
        let config = self.farm_configuration().get();
        assert!(self, config.is_old, b"bad config");

        let new_farm_address = config.new_farm_address;
        let mut context = self.new_farm_context(OptionalArg::None);

        // Same as Exit Farm, since we want to migrate the rewards and compounded tokens also
        {
            self.load_state(&mut context);
            assert!(
                self,
                context.get_contract_state().unwrap() == &State::Migrate,
                ERROR_NOT_MIGRATION
            );

            self.load_farm_token_id(&mut context);
            assert!(
                self,
                !context.get_farm_token_id().unwrap().is_empty(),
                ERROR_NO_FARM_TOKEN,
            );

            self.load_farming_token_id(&mut context);
            assert!(self, context.is_accepted_payment_exit(), ERROR_BAD_PAYMENTS,);

            self.load_reward_token_id(&mut context);
            self.load_reward_reserve(&mut context);
            self.load_block_nonce(&mut context);
            self.load_block_epoch(&mut context);
            self.load_reward_per_share(&mut context);
            self.load_farm_token_supply(&mut context);
            self.load_division_safety_constant(&mut context);
            self.load_farm_attributes(&mut context);

            self.generate_aggregated_rewards(context.get_storage_cache_mut());
            self.calculate_reward(&mut context);
            context.decrease_reward_reserve();
            self.calculate_initial_farming_amount(&mut context);
            self.increase_reward_with_compounded_rewards(&mut context);
            self.commit_changes(&context);

            // Update the farm supply even though we dont burn the tokens by ourselves.
            // The position will be burned and this supply has to be updated.
            self.farm_token_supply()
                .update(|x| *x -= &context.get_tx_input().get_payments().get_first().amount);
        }

        let mut payments = ManagedVec::<Self::Api, EsdtTokenPayment<Self::Api>>::new();
        payments.push(context.get_tx_input().get_payments().get_first().clone());
        payments.push(context.get_final_reward().unwrap().clone());

        self.self_proxy(new_farm_address)
            .migrate_from_old_farm(orig_caller_opt)
            .with_multi_token_transfer(payments)
            .execute_on_dest_context_custom_range(|_, after| (after - 1, after))
    }

    //This function is neccessary in new farms.
    #[payable("*")]
    #[endpoint(migrateFromOldFarm)]
    fn migrate_from_old_farm(
        &self,
        #[var_args] orig_caller_opt: OptionalArg<ManagedAddress>,
    ) -> EsdtTokenPayment<Self::Api> {
        assert!(self, !self.farm_configuration().is_empty(), b"empty config");
        let config = self.farm_configuration().get();
        assert!(self, !config.is_old, b"bad config");

        let caller = self.blockchain().get_caller();
        assert!(self, caller == config.old_farm_address, b"bad caller");

        let payments = self.call_value().all_esdt_transfers();
        assert!(self, payments.len() == 2, b"bad payments len");

        let old_position = payments.get(0).unwrap();
        assert!(self, old_position.amount != 0u64, b"bad farm amount");

        assert!(
            self,
            old_position.token_identifier == config.old_farm_token_id,
            b"bad farm token id"
        );

        let reward = payments.get(1).unwrap();
        assert!(self, reward.amount != 0u64, b"bad reward amount");

        let reward_token_id = self.reward_token_id().get();
        assert!(
            self,
            reward.token_identifier == reward_token_id,
            b"bad reward token id"
        );

        // The actual work starts here
        self.reward_reserve().update(|x| *x += &reward.amount);

        let old_attrs: FarmTokenAttributes<Self::Api> = self
            .blockchain()
            .get_esdt_token_data(
                &self.blockchain().get_sc_address(),
                &old_position.token_identifier,
                old_position.token_nonce,
            )
            .decode_attributes()
            .unwrap();

        // Do not call burn_farm_tokens since this farm tokens belong to other contract
        // which already updated its farm token supply counter.
        self.send().esdt_local_burn(
            &old_position.token_identifier,
            old_position.token_nonce,
            &old_position.amount,
        );

        let new_pos_token_id = self.farm_token_id().get();
        let new_pos_amount = old_position.amount;

        // Use this function because it also updates the farm token supply for this contract instance.
        let new_pos_nonce = self.mint_farm_tokens(&new_pos_token_id, &new_pos_amount, &old_attrs);

        let orig_caller = orig_caller_opt
            .into_option()
            .unwrap_or_else(|| caller.clone());

        // Use this function since it works regardless of wasm ocasional unalignment.
        self.transfer_execute_custom(
            &orig_caller,
            &new_pos_token_id,
            new_pos_nonce,
            &new_pos_amount,
            &OptionalArg::None,
        )
        .unwrap_or_signal_error();

        EsdtTokenPayment::new(new_pos_token_id, new_pos_nonce, new_pos_amount)
    }

    // Each farm that will be migrated and the newer version to which we migrate to
    // will have to be configured using this function.
    #[only_owner]
    #[endpoint(setFarmConfiguration)]
    fn set_farm_configuration(
        &self,
        is_old: bool,
        old_farm_address: ManagedAddress,
        old_farm_token_id: TokenIdentifier,
        new_farm_address: ManagedAddress,
    ) {
        self.farm_configuration().set(&FarmContractConfig {
            is_old,
            old_farm_address,
            old_farm_token_id,
            new_farm_address,
        });
    }

    // We also need to get the rps and transfer it to the new SC.
    #[only_owner]
    #[endpoint(stopRewardsAndMigrateRps)]
    fn stop_rewards_and_migrate_rps(&self) {
        assert!(self, !self.farm_configuration().is_empty(), b"empty config");
        let config = self.farm_configuration().get();
        assert!(self, config.is_old, b"bad config");

        self.state().set(&State::Migrate);
        self.end_produce_rewards();

        self.self_proxy(config.new_farm_address)
            .set_rps_and_start_rewards(self.reward_per_share().get())
            .execute_on_dest_context_ignore_result();
    }

    // In the new sc, we have to set the rps, so rewards can continue
    // with positions being untouched.
    #[endpoint(setRpsAndStartRewards)]
    fn set_rps_and_start_rewards(&self, rps: BigUint) {
        assert!(self, !self.farm_configuration().is_empty(), b"empty config");
        let config = self.farm_configuration().get();
        assert!(self, !config.is_old, b"bad config");
        let caller = self.blockchain().get_caller();
        assert!(self, caller == config.old_farm_address, b"bad caller");

        self.reward_per_share().set(&rps);
        self.start_produce_rewards();
        self.state().set(&State::Active);
    }

    #[proxy]
    fn self_proxy(&self, to: ManagedAddress) -> self::Proxy<Self::Api>;

    #[view(getFarmConfiguration)]
    #[storage_mapper("farm_configuration")]
    fn farm_configuration(&self) -> SingleValueMapper<FarmContractConfig<Self::Api>>;
}

#[derive(TypeAbi, TopEncode, TopDecode)]
pub struct FarmContractConfig<M: ManagedTypeApi> {
    is_old: bool,
    old_farm_address: ManagedAddress<M>,
    old_farm_token_id: TokenIdentifier<M>,
    new_farm_address: ManagedAddress<M>,
}
