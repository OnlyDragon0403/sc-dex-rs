use common_structs::WrappedFarmTokenAttributes;

use super::proxy_common;

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use super::wrapped_lp_token_merge;

use super::proxy_pair;
use proxy_pair::WrappedLpToken;

use super::proxy_farm;
use proxy_farm::WrappedFarmToken;

use factory::locked_asset_token_merge::ProxyTrait as _;
use farm::farm_token_merge::ProxyTrait as _;

#[elrond_wasm::module]
pub trait WrappedFarmTokenMerge:
    token_merge::TokenMergeModule
    + token_send::TokenSendModule
    + proxy_common::ProxyCommonModule
    + wrapped_lp_token_merge::WrappedLpTokenMerge
{
    #[proxy]
    fn locked_asset_factory_proxy(&self, to: ManagedAddress) -> factory::Proxy<Self::Api>;

    #[proxy]
    fn farm_contract_merge_proxy(&self, to: ManagedAddress) -> farm::Proxy<Self::Api>;

    #[payable("*")]
    #[endpoint(mergeWrappedFarmTokens)]
    fn merge_wrapped_farm_tokens(
        &self,
        farm_contract: ManagedAddress,
        #[var_args] opt_accept_funds_func: OptionalArg<ManagedBuffer>,
    ) -> SCResult<()> {
        let caller = self.blockchain().get_caller();
        require!(
            self.intermediated_farms().contains(&farm_contract),
            "Invalid farm contract address"
        );
        let payments_vec = self.get_all_payments_managed_vec();
        let payments_iter = payments_vec.iter();

        self.merge_wrapped_farm_tokens_and_send(
            &caller,
            &farm_contract,
            payments_iter,
            Option::None,
            opt_accept_funds_func,
        )?;
        Ok(())
    }

    fn merge_wrapped_farm_tokens_and_send(
        &self,
        caller: &ManagedAddress,
        farm_contract: &ManagedAddress,
        payments: ManagedVecRefIterator<Self::Api, EsdtTokenPayment<Self::Api>>,
        replic: Option<WrappedFarmToken<Self::Api>>,
        opt_accept_funds_func: OptionalArg<ManagedBuffer>,
    ) -> SCResult<(WrappedFarmToken<Self::Api>, bool)> {
        require!(!payments.is_empty() || replic.is_some(), "Empty deposit");
        let deposit_len = payments.len();

        let wrapped_farm_token_id = self.wrapped_farm_token_id().get();
        self.require_all_tokens_are_wrapped_farm_tokens(payments.clone(), &wrapped_farm_token_id)?;

        let mut tokens = self.get_wrapped_farm_tokens_from_deposit(payments.clone())?;
        if let Some(r) = replic {
            tokens.push(r);
        }

        self.require_wrapped_farm_tokens_have_same_token_ids(&tokens)?;

        let merged_farm_token_amount = self.merge_farm_tokens(farm_contract, &tokens);
        let farming_token_amount = self.merge_farming_tokens(&tokens)?;
        self.burn_payment_tokens(payments);

        let new_attrs = WrappedFarmTokenAttributes {
            farm_token_id: merged_farm_token_amount.token_identifier.clone(),
            farm_token_nonce: merged_farm_token_amount.token_nonce,
            farm_token_amount: merged_farm_token_amount.amount.clone(),
            farming_token_id: farming_token_amount.token_identifier,
            farming_token_nonce: farming_token_amount.token_nonce,
            farming_token_amount: farming_token_amount.amount,
        };

        let new_nonce = self.nft_create_tokens(
            &wrapped_farm_token_id,
            &merged_farm_token_amount.amount,
            &new_attrs,
        );
        self.transfer_execute_custom(
            caller,
            &wrapped_farm_token_id,
            new_nonce,
            &merged_farm_token_amount.amount,
            &opt_accept_funds_func,
        )?;

        let new_token = WrappedFarmToken {
            token_amount: merged_farm_token_amount,
            attributes: new_attrs,
        };
        let is_merged = deposit_len != 0;

        Ok((new_token, is_merged))
    }

    fn get_wrapped_farm_tokens_from_deposit(
        &self,
        payments: ManagedVecRefIterator<Self::Api, EsdtTokenPayment<Self::Api>>,
    ) -> SCResult<ManagedVec<WrappedFarmToken<Self::Api>>> {
        let mut result = ManagedVec::new();

        for payment in payments {
            let attr = self.get_wrapped_farm_token_attributes(
                &payment.token_identifier,
                payment.token_nonce,
            )?;

            result.push(WrappedFarmToken {
                token_amount: payment.clone(),
                attributes: attr,
            })
        }

        Ok(result)
    }

    fn require_wrapped_farm_tokens_have_same_token_ids(
        &self,
        tokens: &ManagedVec<WrappedFarmToken<Self::Api>>,
    ) -> SCResult<()> {
        let token_0 = tokens.get(0);
        let farm_token_id = token_0.attributes.farm_token_id.clone();
        let farming_token_id = token_0.attributes.farming_token_id.clone();

        for elem in tokens.iter() {
            require!(
                elem.attributes.farm_token_id == farm_token_id,
                "Farm token id differs"
            );
            require!(
                elem.attributes.farming_token_id == farming_token_id,
                "Farming token id differs"
            );
        }
        Ok(())
    }

    fn require_all_tokens_are_wrapped_farm_tokens(
        &self,
        tokens: ManagedVecRefIterator<Self::Api, EsdtTokenPayment<Self::Api>>,
        wrapped_farm_token_id: &TokenIdentifier,
    ) -> SCResult<()> {
        for elem in tokens {
            require!(
                &elem.token_identifier == wrapped_farm_token_id,
                "Not a Wrapped Farm Token"
            );
        }
        Ok(())
    }

    fn merge_locked_asset_tokens_from_wrapped_farm(
        &self,
        tokens: &ManagedVec<WrappedFarmToken<Self::Api>>,
    ) -> SCResult<EsdtTokenPayment<Self::Api>> {
        let locked_asset_token = self.locked_asset_token_id().get();
        let locked_asset_factory_addr = self.locked_asset_factory_address().get();

        if tokens.len() == 1 {
            let token = tokens.get(0);
            let locked_token_amount = self.rule_of_three_non_zero_result(
                &token.token_amount.amount,
                &token.attributes.farm_token_amount,
                &token.attributes.farming_token_amount,
            );

            return Ok(self.create_payment(
                &locked_asset_token,
                token.attributes.farming_token_nonce,
                &locked_token_amount,
            ));
        }

        let mut payments = ManagedVec::new();
        for entry in tokens.iter() {
            let locked_token_amount = self.rule_of_three_non_zero_result(
                &entry.token_amount.amount,
                &entry.attributes.farm_token_amount,
                &entry.attributes.farming_token_amount,
            );

            payments.push(EsdtTokenPayment::new(
                locked_asset_token.clone(),
                entry.attributes.farming_token_nonce,
                locked_token_amount,
            ));
        }

        Ok(self
            .locked_asset_factory_proxy(locked_asset_factory_addr)
            .merge_locked_asset_tokens(OptionalArg::None)
            .with_multi_token_transfer(payments)
            .execute_on_dest_context_custom_range(|_, after| (after - 1, after)))
    }

    fn merge_farm_tokens(
        &self,
        farm_contract: &ManagedAddress,
        tokens: &ManagedVec<WrappedFarmToken<Self::Api>>,
    ) -> EsdtTokenPayment<Self::Api> {
        if tokens.len() == 1 {
            let token = tokens.get(0);

            return self.create_payment(
                &token.attributes.farm_token_id,
                token.attributes.farm_token_nonce,
                &token.token_amount.amount,
            );
        }

        let mut payments = ManagedVec::new();
        for entry in tokens.iter() {
            payments.push(EsdtTokenPayment::new(
                entry.attributes.farm_token_id.clone(),
                entry.attributes.farm_token_nonce,
                entry.token_amount.amount.clone(),
            ));
        }

        self.farm_contract_merge_proxy(farm_contract.clone())
            .merge_farm_tokens(OptionalArg::None)
            .with_multi_token_transfer(payments)
            .execute_on_dest_context_custom_range(|_, after| (after - 1, after))
    }

    fn merge_farming_tokens(
        &self,
        tokens: &ManagedVec<WrappedFarmToken<Self::Api>>,
    ) -> SCResult<EsdtTokenPayment<Self::Api>> {
        if tokens.len() == 1 {
            let first_token = tokens.get(0);
            let farming_amount = self.rule_of_three_non_zero_result(
                &first_token.token_amount.amount,
                &first_token.attributes.farm_token_amount,
                &first_token.attributes.farming_token_amount,
            );

            return Ok(self.create_payment(
                &first_token.attributes.farming_token_id,
                first_token.attributes.farming_token_nonce,
                &farming_amount,
            ));
        }

        let farming_token_id = tokens.get(0).attributes.farming_token_id;
        let locked_asset_token_id = self.locked_asset_token_id().get();

        if farming_token_id == locked_asset_token_id {
            self.merge_locked_asset_tokens_from_wrapped_farm(tokens)
        } else {
            self.merge_wrapped_lp_tokens_from_farm(tokens)
        }
    }

    fn merge_wrapped_lp_tokens_from_farm(
        &self,
        tokens: &ManagedVec<WrappedFarmToken<Self::Api>>,
    ) -> SCResult<EsdtTokenPayment<Self::Api>> {
        let mut wrapped_lp_tokens = ManagedVec::new();

        for token in tokens.iter() {
            let wrapped_lp_token_amount = self.rule_of_three_non_zero_result(
                &token.token_amount.amount,
                &token.attributes.farm_token_amount,
                &token.attributes.farming_token_amount,
            );

            let wrapped_lp_token_id = token.attributes.farming_token_id.clone();
            let wrapped_lp_token_nonce = token.attributes.farming_token_nonce;

            let attributes =
                self.get_wrapped_lp_token_attributes(&wrapped_lp_token_id, wrapped_lp_token_nonce)?;
            let wrapped_lp_token = WrappedLpToken {
                token_amount: self.create_payment(
                    &wrapped_lp_token_id,
                    wrapped_lp_token_nonce,
                    &wrapped_lp_token_amount,
                ),
                attributes,
            };
            wrapped_lp_tokens.push(wrapped_lp_token);
        }

        self.require_wrapped_lp_tokens_from_same_pair(&wrapped_lp_tokens)?;
        let merged_locked_token_amount =
            self.merge_locked_asset_tokens_from_wrapped_lp(&wrapped_lp_tokens)?;
        let merged_wrapped_lp_token_amount =
            self.get_merged_wrapped_lp_tokens_amount(&wrapped_lp_tokens);
        let lp_token_amount = self.create_payment(
            &wrapped_lp_tokens.get(0).attributes.lp_token_id,
            0,
            &merged_wrapped_lp_token_amount,
        );

        let attrs = self
            .get_merged_wrapped_lp_token_attributes(&lp_token_amount, &merged_locked_token_amount);

        let wrapped_lp_token_id = tokens.get(0).attributes.farming_token_id.clone();
        let new_nonce = self.nft_create_tokens(
            &wrapped_lp_token_id,
            &merged_wrapped_lp_token_amount,
            &attrs,
        );

        for wrapped_lp_token in wrapped_lp_tokens.iter() {
            self.send().esdt_local_burn(
                &wrapped_lp_token.token_amount.token_identifier,
                wrapped_lp_token.token_amount.token_nonce,
                &wrapped_lp_token.token_amount.amount,
            );
        }

        Ok(self.create_payment(
            &wrapped_lp_token_id,
            new_nonce,
            &merged_wrapped_lp_token_amount,
        ))
    }
}
