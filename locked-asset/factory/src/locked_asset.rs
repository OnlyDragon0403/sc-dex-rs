elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use common_structs::{Epoch, LockedAssetTokenAttributes, Nonce, UnlockMilestone};

pub const PERCENTAGE_TOTAL: u64 = 100;

#[elrond_wasm::module]
pub trait LockedAssetModule: token_supply::TokenSupplyModule + token_send::TokenSendModule {
    fn create_and_send_locked_assets(
        &self,
        amount: &BigUint,
        additional_amount_to_create: &BigUint,
        address: &ManagedAddress,
        attributes: &LockedAssetTokenAttributes<Self::Api>,
        opt_accept_funds_func: &OptionalArg<ManagedBuffer>,
    ) -> SCResult<Nonce> {
        let token_id = self.locked_asset_token_id().get();
        let last_created_nonce = self.nft_create_tokens(
            &token_id,
            &(amount + additional_amount_to_create),
            attributes,
        );
        self.transfer_execute_custom(
            address,
            &token_id,
            last_created_nonce,
            amount,
            opt_accept_funds_func,
        )?;
        Ok(last_created_nonce)
    }

    fn add_quantity_and_send_locked_assets(
        &self,
        amount: &BigUint,
        sft_nonce: Nonce,
        address: &ManagedAddress,
        opt_accept_funds_func: &OptionalArg<ManagedBuffer>,
    ) -> SCResult<()> {
        let token_id = self.locked_asset_token_id().get();
        self.nft_add_quantity_tokens(&token_id, sft_nonce, amount);
        self.transfer_execute_custom(address, &token_id, sft_nonce, amount, opt_accept_funds_func)
    }

    fn get_unlock_amount(
        &self,
        amount: &BigUint,
        current_epoch: Epoch,
        unlock_milestones: &ManagedVec<UnlockMilestone>,
    ) -> BigUint {
        amount
            * &self
                .types()
                .big_uint_from(self.get_unlock_percent(current_epoch, unlock_milestones) as u64)
            / PERCENTAGE_TOTAL
    }

    fn get_unlock_percent(
        &self,
        current_epoch: Epoch,
        unlock_milestones: &ManagedVec<UnlockMilestone>,
    ) -> u8 {
        let mut unlock_percent = 0u8;

        for milestone in unlock_milestones {
            if milestone.unlock_epoch <= current_epoch {
                unlock_percent += milestone.unlock_percent;
            }
        }

        if unlock_percent > 100 {
            let mut err = self.error().new_error();
            err.append_bytes(&b"unlock percent greater than 100"[..]);
            err.exit_now();
        }

        unlock_percent
    }

    fn create_new_unlock_milestones(
        &self,
        current_epoch: Epoch,
        old_unlock_milestones: &ManagedVec<UnlockMilestone>,
    ) -> ManagedVec<UnlockMilestone> {
        let mut new_unlock_milestones = ManagedVec::new();
        let unlock_percent = self.get_unlock_percent(current_epoch, old_unlock_milestones);
        let unlock_percent_remaining = PERCENTAGE_TOTAL - (unlock_percent as u64);

        if unlock_percent_remaining == 0 {
            return new_unlock_milestones;
        }

        for old_milestone in old_unlock_milestones.iter() {
            if old_milestone.unlock_epoch > current_epoch {
                let new_unlock_percent: u64 = (old_milestone.unlock_percent as u64)
                    * PERCENTAGE_TOTAL
                    / unlock_percent_remaining;
                new_unlock_milestones.push(UnlockMilestone {
                    unlock_epoch: old_milestone.unlock_epoch,
                    unlock_percent: new_unlock_percent as u8,
                });
            }
        }
        let mut sum_of_new_percents = 0u8;

        for new_milestone in new_unlock_milestones.iter() {
            sum_of_new_percents += new_milestone.unlock_percent;
        }

        let mut first_element = new_unlock_milestones.get(0).unwrap();
        first_element.unlock_percent += PERCENTAGE_TOTAL as u8 - sum_of_new_percents;

        let mut new_unlocks = ManagedVec::new();
        new_unlocks.push(first_element);

        for (index, elem) in new_unlock_milestones.iter().enumerate() {
            if index != 0 {
                new_unlocks.push(elem);
            }
        }

        new_unlocks
    }

    fn validate_unlock_milestones(
        &self,
        unlock_milestones: &ManagedVarArgs<UnlockMilestone>,
    ) -> SCResult<()> {
        require!(!unlock_milestones.is_empty(), "Empty param");

        let mut percents_sum: u8 = 0;
        let mut last_milestone_unlock_epoch: u64 = 0;

        for milestone in unlock_milestones.0.clone() {
            require!(
                milestone.unlock_epoch >= last_milestone_unlock_epoch,
                "Unlock epochs not in order"
            );
            require!(
                milestone.unlock_percent <= 100,
                "Unlock percent more than 100"
            );
            last_milestone_unlock_epoch = milestone.unlock_epoch;
            percents_sum += milestone.unlock_percent;
        }

        require!(percents_sum == 100, "Percents do not sum up to 100");
        Ok(())
    }

    fn get_attributes(
        &self,
        token_id: &TokenIdentifier,
        token_nonce: u64,
    ) -> SCResult<LockedAssetTokenAttributes<Self::Api>> {
        let token_info = self.blockchain().get_esdt_token_data(
            &self.blockchain().get_sc_address(),
            token_id,
            token_nonce,
        );

        Ok(self
            .serializer()
            .top_decode_from_managed_buffer::<LockedAssetTokenAttributes<Self::Api>>(
                &token_info.attributes,
            ))
    }

    #[only_owner]
    #[endpoint]
    fn set_transfer_exec_gas_limit(&self, gas_limit: u64) {
        self.transfer_exec_gas_limit().set(&gas_limit);
    }

    fn mint_and_send_assets(&self, dest: &ManagedAddress, amount: &BigUint) {
        if amount > &0 {
            let asset_token_id = self.asset_token_id().get();
            self.mint_tokens(&asset_token_id, amount);
            self.send().direct(dest, &asset_token_id, 0, amount, &[]);
        }
    }

    #[view(getLockedAssetTokenId)]
    #[storage_mapper("locked_asset_token_id")]
    fn locked_asset_token_id(&self) -> SingleValueMapper<TokenIdentifier>;

    #[view(getAssetTokenId)]
    #[storage_mapper("asset_token_id")]
    fn asset_token_id(&self) -> SingleValueMapper<TokenIdentifier>;
}
