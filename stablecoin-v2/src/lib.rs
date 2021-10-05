#![no_std]

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

mod fees;
mod math;
mod stablecoin_token;

#[elrond_wasm::contract]
pub trait StablecoinV2:
    fees::FeesModule
    + math::MathModule
    + stablecoin_token::StablecoinTokenModule
{
    #[init]
    fn init(&self) {}

    // private

    // storage

    #[view(getCollateralWhitelist)]
    #[storage_mapper("collateralWhitelist")]
    fn collateral_whitelist(&self) -> SetMapper<TokenIdentifier>;
}
