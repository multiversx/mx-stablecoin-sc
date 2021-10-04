#![no_std]

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

#[elrond_wasm::contract]
pub trait StablecoinV2 {
    #[init]
    fn init(&self) {}
}
