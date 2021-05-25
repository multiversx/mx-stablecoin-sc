#![no_std]

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

#[elrond_wasm_derive::contract]
pub trait LockRewards {
    #[init]
    fn init(&self) -> SCResult<()> {
        Ok(())
    }
}
