elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use super::base::*;
use crate::State;

pub struct SwapContext<M: ManagedTypeApi> {
    caller: ManagedAddress<M>,
    tx_input: SwapTxInput<M>,
    storage_cache: StorageCache<M>,
    initial_k: BigUint<M>,
    output_payments: ManagedVec<M, EsdtTokenPayment<M>>,
}

pub struct SwapTxInput<M: ManagedTypeApi> {
    args: SwapArgs<M>,
    payments: SwapPayments<M>,
}

pub struct SwapArgs<M: ManagedTypeApi> {
    pub output_token_id: TokenIdentifier<M>,
    pub output_amount: BigUint<M>,
    opt_accept_funds_func: OptionalArg<ManagedBuffer<M>>,
}

pub struct SwapPayments<M: ManagedTypeApi> {
    input: EsdtTokenPayment<M>,
}

impl<M: ManagedTypeApi> SwapTxInput<M> {
    pub fn new(args: SwapArgs<M>, payments: SwapPayments<M>) -> Self {
        SwapTxInput { args, payments }
    }
}

impl<M: ManagedTypeApi> SwapArgs<M> {
    pub fn new(
        output_token_id: TokenIdentifier<M>,
        output_amount: BigUint<M>,
        opt_accept_funds_func: OptionalArg<ManagedBuffer<M>>,
    ) -> Self {
        SwapArgs {
            output_token_id,
            output_amount,
            opt_accept_funds_func,
        }
    }
}

impl<M: ManagedTypeApi> SwapPayments<M> {
    pub fn new(input: EsdtTokenPayment<M>) -> Self {
        SwapPayments { input }
    }
}

impl<M: ManagedTypeApi> SwapContext<M> {
    pub fn new(tx_input: SwapTxInput<M>, caller: ManagedAddress<M>) -> Self {
        SwapContext {
            caller,
            tx_input,
            storage_cache: StorageCache::default(),
            initial_k: BigUint::zero(),
            output_payments: ManagedVec::new(),
        }
    }
}

impl<M: ManagedTypeApi> Context<M> for SwapContext<M> {
    fn set_contract_state(&mut self, contract_state: State) {
        self.storage_cache.contract_state = contract_state;
    }

    fn get_contract_state(&self) -> &State {
        &self.storage_cache.contract_state
    }

    fn set_lp_token_id(&mut self, lp_token_id: TokenIdentifier<M>) {
        self.storage_cache.lp_token_id = lp_token_id;
    }

    fn get_lp_token_id(&self) -> &TokenIdentifier<M> {
        &self.storage_cache.lp_token_id
    }

    fn set_first_token_id(&mut self, token_id: TokenIdentifier<M>) {
        self.storage_cache.first_token_id = token_id;
    }

    fn get_first_token_id(&self) -> &TokenIdentifier<M> {
        &self.storage_cache.first_token_id
    }

    fn set_second_token_id(&mut self, token_id: TokenIdentifier<M>) {
        self.storage_cache.second_token_id = token_id;
    }

    fn get_second_token_id(&self) -> &TokenIdentifier<M> {
        &self.storage_cache.second_token_id
    }

    fn set_first_token_reserve(&mut self, amount: BigUint<M>) {
        self.storage_cache.first_token_reserve = amount;
    }

    fn get_first_token_reserve(&self) -> &BigUint<M> {
        &self.storage_cache.first_token_reserve
    }

    fn set_second_token_reserve(&mut self, amount: BigUint<M>) {
        self.storage_cache.second_token_reserve = amount;
    }

    fn get_second_token_reserve(&self) -> &BigUint<M> {
        &self.storage_cache.second_token_reserve
    }

    fn set_lp_token_supply(&mut self, amount: BigUint<M>) {
        self.storage_cache.lp_token_supply = amount;
    }

    fn get_lp_token_supply(&self) -> &BigUint<M> {
        &self.storage_cache.lp_token_supply
    }

    fn set_initial_k(&mut self, k: BigUint<M>) {
        self.initial_k = k;
    }

    fn get_initial_k(&self) -> &BigUint<M> {
        &self.initial_k
    }

    fn get_caller(&self) -> &ManagedAddress<M> {
        &self.caller
    }

    fn set_output_payments(&mut self, payments: ManagedVec<M, EsdtTokenPayment<M>>) {
        self.output_payments = payments
    }

    fn get_output_payments(&self) -> &ManagedVec<M, EsdtTokenPayment<M>> {
        &self.output_payments
    }

    fn get_opt_accept_funds_func(&self) -> &OptionalArg<ManagedBuffer<M>> {
        &self.tx_input.args.opt_accept_funds_func
    }

    fn get_tx_input(&self) -> &dyn TxInput<M> {
        &self.tx_input
    }
}

impl<M: ManagedTypeApi> TxInputArgs<M> for SwapArgs<M> {
    fn are_valid(&self) -> bool {
        self.output_amount != 0 && self.output_token_id.is_esdt()
    }
}

impl<M: ManagedTypeApi> TxInputPayments<M> for SwapPayments<M> {
    fn are_valid(&self) -> bool {
        self.input.amount != 0
            && self.input.token_identifier.is_esdt()
            && self.input.token_nonce == 0
    }
}

impl<M: ManagedTypeApi> TxInput<M> for SwapTxInput<M> {
    fn get_args(&self) -> &dyn TxInputArgs<M> {
        &self.args
    }

    fn get_payments(&self) -> &dyn TxInputPayments<M> {
        &self.payments
    }

    fn is_valid(&self) -> bool {
        self.args.output_token_id != self.payments.input.token_identifier
    }
}

impl<M: ManagedTypeApi> SwapContext<M> {
    pub fn input_tokens_match_pool_tokens(&self) -> bool {
        (self.tx_input.args.output_token_id == self.storage_cache.first_token_id
            || self.tx_input.args.output_token_id == self.storage_cache.second_token_id)
            && (self.tx_input.payments.input.token_identifier == self.storage_cache.first_token_id
                || self.tx_input.payments.input.token_identifier
                    == self.storage_cache.second_token_id)
    }

    pub fn get_payment(&self) -> &EsdtTokenPayment<M> {
        &self.tx_input.payments.input
    }

    pub fn get_swap_args(&self) -> &SwapArgs<M> {
        &self.tx_input.args
    }

    pub fn get_token_in(&self) -> &TokenIdentifier<M> {
        &self.tx_input.payments.input.token_identifier
    }

    pub fn get_amount_in(&self) -> &BigUint<M> {
        &self.tx_input.payments.input.amount
    }

    pub fn get_token_out(&self) -> &TokenIdentifier<M> {
        &self.tx_input.args.output_token_id
    }

    pub fn get_amount_out_min(&self) -> &BigUint<M> {
        self.get_amount_out()
    }

    pub fn get_amount_in_max(&self) -> &BigUint<M> {
        self.get_amount_in()
    }

    pub fn get_amount_out(&self) -> &BigUint<M> {
        &self.tx_input.args.output_amount
    }

    pub fn get_reserve_in(&self) -> &BigUint<M> {
        let payment_token_id = &self.tx_input.payments.input.token_identifier;

        if payment_token_id == &self.storage_cache.first_token_id {
            &self.storage_cache.first_token_reserve
        } else if payment_token_id == &self.storage_cache.second_token_id {
            &self.storage_cache.second_token_reserve
        } else {
            unreachable!()
        }
    }

    pub fn get_reserve_out(&self) -> &BigUint<M> {
        let args_token_id = &self.tx_input.args.output_token_id;

        if args_token_id == &self.storage_cache.first_token_id {
            &self.storage_cache.first_token_reserve
        } else if args_token_id == &self.storage_cache.second_token_id {
            &self.storage_cache.second_token_reserve
        } else {
            unreachable!()
        }
    }

    pub fn increase_reserve_in(&mut self, amount: &BigUint<M>) {
        let payment_token_id = &self.tx_input.payments.input.token_identifier;

        if payment_token_id == &self.storage_cache.first_token_id {
            self.storage_cache.first_token_reserve += amount;
        } else if payment_token_id == &self.storage_cache.second_token_id {
            self.storage_cache.second_token_reserve += amount;
        } else {
            unreachable!()
        }
    }

    pub fn decrease_reserve_out(&mut self, amount: &BigUint<M>) {
        let args_token_id = &self.tx_input.args.output_token_id;

        if args_token_id == &self.storage_cache.first_token_id {
            self.storage_cache.first_token_reserve -= amount;
        } else if args_token_id == &self.storage_cache.second_token_id {
            self.storage_cache.second_token_reserve -= amount;
        } else {
            unreachable!()
        }
    }
}
