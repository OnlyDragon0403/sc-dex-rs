elrond_wasm::imports!();
elrond_wasm::derive_imports!();

#[derive(TopEncode)]
pub struct CreatePairEvent {
    user_address: Address,
    first_token_id: TokenIdentifier,
    second_token_id: TokenIdentifier,
    total_fee_percent: u64,
    special_fee_percent: u64,
    pair_address: Address,
    block: u64,
    epoch: u64,
    timestamp: u64,
}

#[elrond_wasm_derive::module]
pub trait EventsModule {
    fn emit_create_pair_event(
        &self,
        user_address: &Address,
        first_token_id: &TokenIdentifier,
        second_token_id: &TokenIdentifier,
        total_fee_percent: u64,
        special_fee_percent: u64,
        pair_address: &Address,
    ) {
        let epoch = self.blockchain().get_block_epoch();
        self.create_pair_event(
            first_token_id,
            second_token_id,
            user_address,
            epoch,
            CreatePairEvent {
                user_address: user_address.clone(),
                first_token_id: first_token_id.clone(),
                second_token_id: second_token_id.clone(),
                total_fee_percent,
                special_fee_percent,
                pair_address: pair_address.clone(),
                block: self.blockchain().get_block_nonce(),
                epoch,
                timestamp: self.blockchain().get_block_timestamp(),
            },
        )
    }
    
    #[event("create_pair")]
    fn create_pair_event(
        &self,
        #[indexed] first_token_id: &TokenIdentifier,
        #[indexed] second_token_id: &TokenIdentifier,
        #[indexed] user_address: &Address,
        #[indexed] epoch: u64,
        swap_event: CreatePairEvent,
    );
}
