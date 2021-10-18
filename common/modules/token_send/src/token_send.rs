#![no_std]

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

extern "C" {
    fn transferESDTNFTExecute(
        dstOffset: *const u8,
        tokenIdOffset: *const u8,
        tokenIdLen: i32,
        valueOffset: *const u8,
        nonce: i64,
        gasLimit: i64,
        functionOffset: *const u8,
        functionLength: i32,
        numArguments: i32,
        argumentsLengthOffset: *const u8,
        dataOffset: *const u8,
    ) -> i32;
    fn bigIntUnsignedByteLength(x: i32) -> i32;
    fn bigIntGetUnsignedBytes(reference: i32, byte_ptr: *mut u8) -> i32;
}

pub type Handle = i32;
const BUFFER_SIZE: usize = 32;
static mut BUFFER_CUSTOM: [u8; BUFFER_SIZE] = [b'u'; BUFFER_SIZE];

unsafe fn clear_buffer_custom() {
    core::ptr::write_bytes(BUFFER_CUSTOM.as_mut_ptr(), 0u8, BUFFER_SIZE);
}

unsafe fn buffer_ptr_custom() -> *mut u8 {
    BUFFER_CUSTOM.as_mut_ptr()
}

unsafe fn unsafe_buffer_load_be_pad_right_custom(bi_handle: Handle, nr_bytes: usize) -> *const u8 {
    let byte_len = bigIntUnsignedByteLength(bi_handle) as usize;
    if byte_len > nr_bytes {
        panic!("critical err, bad len");
    }
    clear_buffer_custom();
    if byte_len > 0 {
        bigIntGetUnsignedBytes(bi_handle, buffer_ptr_custom().add(nr_bytes - byte_len));
    }
    buffer_ptr_custom()
}

use common_structs::Nonce;

#[elrond_wasm::module]
pub trait TokenSendModule {
    fn send_fft_tokens(
        &self,
        destination: &ManagedAddress,
        token: &TokenIdentifier,
        amount: &BigUint,
        opt_accept_funds_func: &OptionalArg<BoxedBytes>,
    ) -> SCResult<()> {
        let (function, gas_limit) = match opt_accept_funds_func {
            OptionalArg::Some(accept_funds_func) => (
                accept_funds_func.clone(),
                self.transfer_exec_gas_limit().get(),
            ),
            OptionalArg::None => (BoxedBytes::empty(), 0u64),
        };

        self.raw_vm_api()
            .direct_esdt_execute(
                destination,
                token,
                amount,
                gas_limit,
                &ManagedBuffer::managed_from(self.type_manager(), function),
                &ManagedArgBuffer::new_empty(self.type_manager()),
            )
            .into()
    }

    fn send_nft_tokens(
        &self,
        destination: &ManagedAddress,
        token: &TokenIdentifier,
        nonce: Nonce,
        amount: &BigUint,
        opt_accept_funds_func: &OptionalArg<BoxedBytes>,
    ) -> SCResult<()> {
        let (function, gas_limit) = match opt_accept_funds_func {
            OptionalArg::Some(accept_funds_func) => (
                accept_funds_func.clone(),
                self.transfer_exec_gas_limit().get(),
            ),
            OptionalArg::None => (BoxedBytes::empty(), 0u64),
        };

        self.raw_vm_api()
            .direct_esdt_nft_execute(
                destination,
                token,
                nonce,
                amount,
                gas_limit,
                &ManagedBuffer::managed_from(self.type_manager(), function),
                &ManagedArgBuffer::new_empty(self.type_manager()),
            )
            .into()
    }

    fn send_multiple_tokens(
        &self,
        destination: &Address,
        payments: &[EsdtTokenPayment<Self::Api>],
        opt_accept_funds_func: &OptionalArg<BoxedBytes>,
    ) -> SCResult<()> {
        let (function, gas_limit) = match opt_accept_funds_func {
            OptionalArg::Some(accept_funds_func) => (
                accept_funds_func.as_slice(),
                self.transfer_exec_gas_limit().get(),
            ),
            OptionalArg::None => {
                let no_func: &[u8] = &[];
                (no_func, 0u64)
            }
        };

        self.raw_vm_api()
            .direct_multi_esdt_transfer_execute(
                &ManagedAddress::managed_from(self.type_manager(), destination),
                &ManagedVec::managed_from(self.type_manager(), payments.to_vec()),
                gas_limit,
                &ManagedBuffer::managed_from(self.type_manager(), function),
                &ManagedArgBuffer::new_empty(self.type_manager()),
            )
            .into()
    }

    fn send_multiple_tokens_compact(
        &self,
        destination: &Address,
        payments: &[EsdtTokenPayment<Self::Api>],
        opt_accept_funds_func: &OptionalArg<BoxedBytes>,
    ) -> SCResult<()> {
        let mut compact_payments = Vec::<EsdtTokenPayment<Self::Api>>::new();
        for payment in payments.iter() {
            if payment.amount != 0 {
                let existing_index = compact_payments.iter().position(|x| {
                    x.token_identifier == payment.token_identifier
                        && x.token_nonce == payment.token_nonce
                });

                match existing_index {
                    Some(index) => compact_payments[index].amount += &payment.amount,
                    None => compact_payments.push(payment.clone()),
                }
            }
        }

        match compact_payments.len() {
            0 => Ok(()),
            _ => self.send_multiple_tokens(destination, &compact_payments, opt_accept_funds_func),
        }
    }

    fn direct_esdt_nft_execute_custom(
        &self,
        to: &ManagedAddress,
        token: &TokenIdentifier,
        nonce: u64,
        amount: &BigUint,
        opt_accept_funds_func: &OptionalArg<BoxedBytes>,
    ) -> SCResult<()> {
        let to_address = to.to_address();
        let arg_buffer = ManagedArgBuffer::new_empty(self.type_manager());
        let (function, gas_limit) = match opt_accept_funds_func {
            OptionalArg::Some(accept_funds_func) => (
                accept_funds_func.as_slice(),
                self.transfer_exec_gas_limit().get(),
            ),
            OptionalArg::None => {
                let no_func: &[u8] = &[];
                (no_func, 0u64)
            }
        };

        unsafe {
            let amount_bytes32_ptr =
                unsafe_buffer_load_be_pad_right_custom(amount.get_raw_handle(), 32);
            let function = BoxedBytes::from(function);
            let legacy_arg_buffer = arg_buffer.to_legacy_arg_buffer();
            let result = transferESDTNFTExecute(
                to_address.as_ptr(),
                token.to_esdt_identifier().as_ptr(),
                token.len() as i32,
                amount_bytes32_ptr,
                nonce as i64,
                gas_limit as i64,
                function.as_ptr(),
                function.len() as i32,
                legacy_arg_buffer.num_args() as i32,
                legacy_arg_buffer.arg_lengths_bytes_ptr(),
                legacy_arg_buffer.arg_data_ptr(),
            );
            require!(result == 0, "bad result");
            Ok(())
        }
    }

    fn get_all_payments(&self) -> Vec<EsdtTokenPayment<Self::Api>> {
        self.raw_vm_api()
            .get_all_esdt_transfers()
            .into_iter()
            .collect()
    }

    #[view(getTransferExecGasLimit)]
    #[storage_mapper("transfer_exec_gas_limit")]
    fn transfer_exec_gas_limit(&self) -> SingleValueMapper<u64>;
}
