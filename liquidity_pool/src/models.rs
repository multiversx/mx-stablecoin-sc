use elrond_wasm::{api::BigUintApi, types::TokenIdentifier};

elrond_wasm::derive_imports!();

#[derive(NestedEncode, NestedDecode, TopEncode, TopDecode, TypeAbi, PartialEq, Clone)]
pub struct DebtPosition<BigUint: BigUintApi> {
    pub health_factor: u32,
    pub is_liquidated: bool,
    pub collateral_timestamp: u64,
    pub collateral_amount: BigUint,
    pub collateral_id: TokenIdentifier,
}

#[derive(TopEncode, TopDecode, TypeAbi, PartialEq, Clone)]
pub struct RepayPosition<BigUint: BigUintApi> {
    pub collateral_amount_to_withdraw: BigUint,
    pub nft_nonce: u64,
    pub debt_paid: BigUint,
}
