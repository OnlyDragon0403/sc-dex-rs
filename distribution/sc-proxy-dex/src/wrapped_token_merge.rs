use common_structs::{
    FarmTokenAttributes, GenericEsdtAmountPair, WrappedFarmTokenAttributes,
    WrappedLpTokenAttributes,
};

use super::proxy_common;
use proxy_common::ACCEPT_PAY_FUNC_NAME;

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use super::proxy_pair;
use proxy_pair::WrappedLpToken;

use super::proxy_farm;
use proxy_farm::WrappedFarmToken;

#[elrond_wasm_derive::module]
pub trait WrappedTokenMerge:
    token_merge::TokenMergeModule
    + token_send::TokenSendModule
    + token_supply::TokenSupplyModule
    + proxy_common::ProxyCommonModule
    + nft_deposit::NftDepositModule
{
    #[proxy]
    fn locked_asset_factory(&self, to: Address) -> sc_locked_asset_factory::Proxy<Self::SendApi>;

    #[proxy]
    fn farm_contract_merge_proxy(&self, to: Address) -> elrond_dex_farm::Proxy<Self::SendApi>;

    #[endpoint(mergeWrappedLpTokens)]
    fn merge_wrapped_lp_tokens(
        &self,
        #[var_args] opt_accept_funds_func: OptionalArg<BoxedBytes>,
    ) -> SCResult<()> {
        let caller = self.blockchain().get_caller();
        self.merge_wrapped_lp_tokens_and_send(&caller, Option::None, opt_accept_funds_func)
    }

    fn merge_wrapped_lp_tokens_and_send(
        &self,
        caller: &Address,
        replic: Option<WrappedLpToken<Self::BigUint>>,
        opt_accept_funds_func: OptionalArg<BoxedBytes>,
    ) -> SCResult<()> {
        let deposit = self.nft_deposit(caller).load_as_vec();
        require!(!deposit.is_empty() || replic.is_some(), "Empty deposit");

        let wrapped_lp_token_id = self.wrapped_lp_token_id().get();
        self.require_all_tokens_are_wrapped_lp_tokens(&deposit, &wrapped_lp_token_id)?;

        let mut tokens = self.get_wrapped_lp_tokens_from_deposit(&deposit)?;

        if replic.is_some() {
            tokens.push(replic.unwrap());
        }
        self.require_wrapped_lp_tokens_from_same_pair(&tokens)?;

        let merged_locked_token_amount = self.merge_locked_asset_tokens_from_wrapped_lp(&tokens);

        let attrs =
            self.get_merged_wrapped_lp_token_attributes(&tokens, &merged_locked_token_amount);
        let amount = self.get_merged_wrapped_lp_tokens_amount(&tokens);

        self.burn_deposit_tokens(caller);
        self.nft_deposit(caller).clear();

        self.nft_create_tokens(&wrapped_lp_token_id, &amount, &attrs);
        let new_nonce = self.increase_wrapped_lp_token_nonce();

        self.send_nft_tokens(
            &wrapped_lp_token_id,
            new_nonce,
            &amount,
            caller,
            &opt_accept_funds_func,
        );

        Ok(())
    }

    #[endpoint(mergeWrappedFarmTokens)]
    fn merge_wrapped_farm_tokens(
        &self,
        farm_contract: Address,
        #[var_args] opt_accept_funds_func: OptionalArg<BoxedBytes>,
    ) -> SCResult<()> {
        let caller = self.blockchain().get_caller();
        require!(
            self.intermediated_farms().contains(&farm_contract),
            "Invalid farm contract address"
        );

        self.merge_wrapped_farm_tokens_and_send(
            &caller,
            &farm_contract,
            Option::None,
            opt_accept_funds_func,
        )
    }

    fn merge_wrapped_farm_tokens_and_send(
        &self,
        caller: &Address,
        farm_contract: &Address,
        replic: Option<WrappedFarmToken<Self::BigUint>>,
        opt_accept_funds_func: OptionalArg<BoxedBytes>,
    ) -> SCResult<()> {
        let deposit = self.nft_deposit(caller).load_as_vec();
        require!(!deposit.is_empty() || replic.is_some(), "Empty deposit");

        let wrapped_farm_token_id = self.wrapped_farm_token_id().get();
        self.require_all_tokens_are_wrapped_farm_tokens(&deposit, &wrapped_farm_token_id)?;

        let mut tokens = self.get_wrapped_farm_tokens_from_deposit(&deposit)?;

        if replic.is_some() {
            tokens.push(replic.unwrap());
        }
        self.require_wrapped_farm_tokens_from_same_farm(&tokens)?;

        let merged_farm_token_amount = self.merge_farm_tokens(farm_contract, &tokens);
        let farming_token_amount = self.merge_farming_tokens(&tokens)?;

        let new_attrs = WrappedFarmTokenAttributes {
            farm_token_id: merged_farm_token_amount.token_id,
            farm_token_nonce: merged_farm_token_amount.token_nonce,
            farm_token_amount: merged_farm_token_amount.amount.clone(),
            farming_token_id: farming_token_amount.token_id,
            farming_token_nonce: farming_token_amount.token_nonce,
            farming_token_amount: farming_token_amount.amount,
        };

        self.nft_create_tokens(
            &wrapped_farm_token_id,
            &merged_farm_token_amount.amount,
            &new_attrs,
        );
        let new_nonce = self.increase_wrapped_farm_token_nonce();

        self.burn_deposit_tokens(caller);
        self.nft_deposit(caller).clear();

        self.send_nft_tokens(
            &wrapped_farm_token_id,
            new_nonce,
            &merged_farm_token_amount.amount,
            caller,
            &opt_accept_funds_func,
        );

        Ok(())
    }

    fn get_wrapped_lp_tokens_from_deposit(
        &self,
        deposit: &[GenericEsdtAmountPair<Self::BigUint>],
    ) -> SCResult<Vec<WrappedLpToken<Self::BigUint>>> {
        let mut result = Vec::new();

        for elem in deposit.iter() {
            result.push(WrappedLpToken {
                token_amount: elem.clone(),
                attributes: self
                    .get_wrapped_lp_token_attributes(&elem.token_id, elem.token_nonce)?,
            })
        }
        Ok(result)
    }

    fn get_wrapped_farm_tokens_from_deposit(
        &self,
        deposit: &[GenericEsdtAmountPair<Self::BigUint>],
    ) -> SCResult<Vec<WrappedFarmToken<Self::BigUint>>> {
        let mut result = Vec::new();

        for elem in deposit.iter() {
            result.push(WrappedFarmToken {
                token_amount: elem.clone(),
                attributes: self
                    .get_wrapped_farm_token_attributes(&elem.token_id, elem.token_nonce)?,
            })
        }
        Ok(result)
    }

    fn require_wrapped_lp_tokens_from_same_pair(
        &self,
        tokens: &[WrappedLpToken<Self::BigUint>],
    ) -> SCResult<()> {
        let lp_token_id = tokens[0].attributes.lp_token_id.clone();

        for elem in tokens.iter() {
            require!(
                elem.attributes.lp_token_id == lp_token_id,
                "Lp token id differs"
            );
        }
        Ok(())
    }

    fn require_wrapped_farm_tokens_from_same_farm(
        &self,
        tokens: &[WrappedFarmToken<Self::BigUint>],
    ) -> SCResult<()> {
        let farm_token_id = tokens[0].attributes.farm_token_id.clone();

        for elem in tokens.iter() {
            require!(
                elem.attributes.farm_token_id == farm_token_id,
                "Farm token id differs"
            );
        }
        Ok(())
    }

    fn require_all_tokens_are_wrapped_lp_tokens(
        &self,
        tokens: &[GenericEsdtAmountPair<Self::BigUint>],
        wrapped_lp_token_id: &TokenIdentifier,
    ) -> SCResult<()> {
        for elem in tokens.iter() {
            require!(
                &elem.token_id == wrapped_lp_token_id,
                "Not a Wrapped Lp Token"
            );
        }
        Ok(())
    }

    fn require_all_tokens_are_wrapped_farm_tokens(
        &self,
        tokens: &[GenericEsdtAmountPair<Self::BigUint>],
        wrapped_farm_token_id: &TokenIdentifier,
    ) -> SCResult<()> {
        for elem in tokens.iter() {
            require!(
                &elem.token_id == wrapped_farm_token_id,
                "Not a Wrapped Farm Token"
            );
        }
        Ok(())
    }

    fn get_merged_wrapped_lp_token_attributes(
        &self,
        tokens: &[WrappedLpToken<Self::BigUint>],
        merged_locked_asset_token_amount: &GenericEsdtAmountPair<Self::BigUint>,
    ) -> WrappedLpTokenAttributes<Self::BigUint> {
        let mut lp_token_amount = Self::BigUint::zero();

        tokens
            .iter()
            .for_each(|x| lp_token_amount += &x.attributes.lp_token_total_amount);
        WrappedLpTokenAttributes {
            lp_token_id: tokens[0].attributes.lp_token_id.clone(),
            lp_token_total_amount: lp_token_amount,
            locked_assets_invested: merged_locked_asset_token_amount.amount.clone(),
            locked_assets_nonce: merged_locked_asset_token_amount.token_nonce,
        }
    }

    fn merge_locked_asset_tokens_from_wrapped_lp(
        &self,
        tokens: &[WrappedLpToken<Self::BigUint>],
    ) -> GenericEsdtAmountPair<Self::BigUint> {
        let locked_asset_factory_addr = self.locked_asset_factory_address().get();
        let locked_asset_token = self.locked_asset_token_id().get();

        if tokens.len() == 1 {
            let token = tokens[0].clone();

            let amount = self.rule_of_three(
                &token.token_amount.amount,
                &token.attributes.lp_token_total_amount,
                &token.attributes.locked_assets_invested,
            );
            return GenericEsdtAmountPair {
                token_id: locked_asset_token,
                token_nonce: token.attributes.locked_assets_nonce,
                amount,
            };
        }

        for entry in tokens.iter() {
            let amount = self.rule_of_three(
                &entry.token_amount.amount,
                &entry.attributes.lp_token_total_amount,
                &entry.attributes.locked_assets_invested,
            );

            self.locked_asset_factory(locked_asset_factory_addr.clone())
                .depositToken(
                    locked_asset_token.clone(),
                    entry.attributes.locked_assets_nonce,
                    amount,
                )
                .execute_on_dest_context();
        }

        self.locked_asset_factory(locked_asset_factory_addr)
            .mergeLockedAssetTokens(OptionalArg::Some(BoxedBytes::from(ACCEPT_PAY_FUNC_NAME)))
            .execute_on_dest_context_custom_range(|_, after| (after - 1, after))
    }

    fn merge_locked_asset_tokens_from_wrapped_farm(
        &self,
        tokens: &[WrappedFarmToken<Self::BigUint>],
    ) -> GenericEsdtAmountPair<Self::BigUint> {
        let locked_asset_factory_addr = self.locked_asset_factory_address().get();

        if tokens.len() == 1 {
            let token = tokens[0].clone();
            let locked_token_amount = self.rule_of_three(
                &token.token_amount.amount,
                &token.attributes.farm_token_amount,
                &token.attributes.farming_token_amount,
            );

            return GenericEsdtAmountPair {
                token_id: self.locked_asset_token_id().get(),
                token_nonce: token.attributes.farming_token_nonce,
                amount: locked_token_amount,
            };
        }

        let locked_asset_token = self.locked_asset_token_id().get();
        for entry in tokens.iter() {
            let locked_token_amount = self.rule_of_three(
                &entry.token_amount.amount,
                &entry.attributes.farm_token_amount,
                &entry.attributes.farming_token_amount,
            );

            self.locked_asset_factory(locked_asset_factory_addr.clone())
                .depositToken(
                    locked_asset_token.clone(),
                    entry.attributes.farming_token_nonce,
                    locked_token_amount,
                )
                .execute_on_dest_context();
        }

        self.locked_asset_factory(locked_asset_factory_addr)
            .mergeLockedAssetTokens(OptionalArg::Some(BoxedBytes::from(ACCEPT_PAY_FUNC_NAME)))
            .execute_on_dest_context_custom_range(|_, after| (after - 1, after))
    }

    fn merge_farm_tokens(
        &self,
        farm_contract: &Address,
        tokens: &[WrappedFarmToken<Self::BigUint>],
    ) -> GenericEsdtAmountPair<Self::BigUint> {
        if tokens.len() == 1 {
            let token = tokens[0].clone();

            return GenericEsdtAmountPair {
                token_id: token.attributes.farm_token_id,
                token_nonce: token.attributes.farm_token_nonce,
                amount: token.token_amount.amount,
            };
        }

        for entry in tokens.iter() {
            self.farm_contract_merge_proxy(farm_contract.clone())
                .depositFarmToken(
                    entry.attributes.farm_token_id.clone(),
                    entry.attributes.farm_token_nonce,
                    entry.token_amount.amount.clone(),
                )
                .execute_on_dest_context();
        }

        self.farm_contract_merge_proxy(farm_contract.clone())
            .mergeFarmTokens(OptionalArg::Some(BoxedBytes::from(ACCEPT_PAY_FUNC_NAME)))
            .execute_on_dest_context_custom_range(|_, after| (after - 1, after))
    }

    fn merge_farming_tokens(
        &self,
        tokens: &[WrappedFarmToken<Self::BigUint>],
    ) -> SCResult<GenericEsdtAmountPair<Self::BigUint>> {
        if tokens.len() == 1 {
            let first_token = tokens[0].clone();
            let farming_amount = self.rule_of_three(
                &first_token.token_amount.amount,
                &first_token.attributes.farm_token_amount,
                &first_token.attributes.farming_token_amount,
            );

            return Ok(GenericEsdtAmountPair {
                token_id: first_token.attributes.farming_token_id,
                token_nonce: first_token.attributes.farming_token_nonce,
                amount: farming_amount,
            });
        }

        let farming_token_id = tokens[0].clone().attributes.farming_token_id;
        let locked_asset_token_id = self.locked_asset_token_id().get();

        if farming_token_id == locked_asset_token_id {
            Ok(self.merge_locked_asset_tokens_from_wrapped_farm(tokens))
        } else {
            self.merge_wrapped_lp_tokens_from_farm(tokens)
        }
    }

    fn merge_wrapped_lp_tokens_from_farm(
        &self,
        tokens: &[WrappedFarmToken<Self::BigUint>],
    ) -> SCResult<GenericEsdtAmountPair<Self::BigUint>> {
        let mut wrapped_lp_tokens = Vec::new();

        for token in tokens.iter() {
            let wrapped_lp_token_amount = self.rule_of_three(
                &token.token_amount.amount,
                &token.attributes.farming_token_amount,
                &token.attributes.farm_token_amount,
            );
            let wrapped_lp_token_id = token.attributes.farming_token_id.clone();
            let wrapped_lp_token_nonce = token.attributes.farming_token_nonce;

            let attributes =
                self.get_wrapped_lp_token_attributes(&wrapped_lp_token_id, wrapped_lp_token_nonce)?;
            let wrapped_lp_token = WrappedLpToken {
                token_amount: GenericEsdtAmountPair {
                    token_id: wrapped_lp_token_id.clone(),
                    token_nonce: wrapped_lp_token_nonce,
                    amount: wrapped_lp_token_amount,
                },
                attributes,
            };
            wrapped_lp_tokens.push(wrapped_lp_token);
        }

        let merged_locked_token_amount =
            self.merge_locked_asset_tokens_from_wrapped_lp(&wrapped_lp_tokens);

        let attrs = self.get_merged_wrapped_lp_token_attributes(
            &wrapped_lp_tokens,
            &merged_locked_token_amount,
        );
        let amount = self.get_merged_wrapped_lp_tokens_amount(&wrapped_lp_tokens);

        let wrapped_lp_token_id = tokens[0].attributes.farming_token_id.clone();
        self.nft_create_tokens(&wrapped_lp_token_id, &amount, &attrs);
        let new_nonce = self.increase_wrapped_lp_token_nonce();

        for wrapped_lp_token in wrapped_lp_tokens.iter() {
            self.nft_burn_tokens(
                &wrapped_lp_token.token_amount.token_id,
                wrapped_lp_token.token_amount.token_nonce,
                &wrapped_lp_token.token_amount.amount,
            );
        }

        Ok(GenericEsdtAmountPair {
            token_id: wrapped_lp_token_id,
            token_nonce: new_nonce,
            amount,
        })
    }

    fn get_merged_wrapped_lp_tokens_amount(
        &self,
        tokens: &[WrappedLpToken<Self::BigUint>],
    ) -> Self::BigUint {
        let mut token_amount = Self::BigUint::zero();

        tokens
            .iter()
            .for_each(|x| token_amount += &x.token_amount.amount);
        token_amount
    }

    fn get_farm_attributes(
        &self,
        token_id: &TokenIdentifier,
        token_nonce: u64,
    ) -> SCResult<FarmTokenAttributes<Self::BigUint>> {
        let token_info = self.blockchain().get_esdt_token_data(
            &self.blockchain().get_sc_address(),
            token_id,
            token_nonce,
        );

        let farm_attributes = token_info.decode_attributes::<FarmTokenAttributes<Self::BigUint>>();
        match farm_attributes {
            Result::Ok(decoded_obj) => Ok(decoded_obj),
            Result::Err(_) => {
                return sc_error!("Decoding error");
            }
        }
    }
}
