use elrond_wasm::{api::BigUintApi, types::TokenIdentifier};

elrond_wasm::derive_imports!();

#[derive(TopEncode, TopDecode, TypeAbi)]
pub struct InterestMetadata {
    pub timestamp: u64,
}


#[derive(NestedEncode, NestedDecode, TopEncode, TopDecode, TypeAbi, PartialEq, Clone)]
pub struct DebtPosition<BigUint: BigUintApi> {
    pub initial_amount: BigUint,
    pub health_factor: u32,
    pub is_liquidated: bool,
    pub collateral_timestamp: u64,
    pub collateral_amount: BigUint,
    pub collateral_id: TokenIdentifier,
}

#[derive(TopEncode, TopDecode, TypeAbi)]
pub struct LiquidateData<BigUint: BigUintApi> {
    pub collateral_token: TokenIdentifier,
    pub amount: BigUint,
}

#[derive(TopEncode, TopDecode, TypeAbi, Clone)]
pub struct DebtMetadata {
    pub collateral_id: TokenIdentifier,
    pub collateral_timestamp: u64,
}

#[derive(TopEncode, TopDecode, TypeAbi, PartialEq, Clone)]
pub struct RepayPostion<BigUint: BigUintApi> {
    pub identifier: TokenIdentifier,
    pub amount: BigUint,
    pub nonce: u64,
    pub collateral_id: TokenIdentifier,
    pub collateral_amount: BigUint,
    pub collateral_timestamp: u64,
}

impl<BigUint: BigUintApi> Default for DebtPosition<BigUint> {
    fn default() -> Self {
        DebtPosition {
            initial_amount: BigUint::zero(),
            health_factor: 0u32,
            is_liquidated: bool::default(),
            collateral_timestamp: 0u64,
            collateral_amount: BigUint::zero(),
            collateral_id: TokenIdentifier::egld(),
        }
    }
}

impl<BigUint: BigUintApi> Default for RepayPostion<BigUint> {
    fn default() -> Self {
        RepayPostion {
            identifier: TokenIdentifier::egld(),
            amount: BigUint::zero(),
            nonce: 0u64,
            collateral_id: TokenIdentifier::egld(),
            collateral_amount: BigUint::zero(),
            collateral_timestamp: 0u64,
        }
    }
}
