use std::ops::Mul;

use elrond_wasm::{
    storage::mappers::StorageTokenWrapper,
    types::{Address, EsdtLocalRole, ManagedBuffer, ManagedVec, TokenIdentifier, BigUint},
};

use elrond_wasm_debug::{
    managed_address, managed_biguint, managed_buffer, managed_token_id, rust_biguint,
    testing_framework::*, DebugApi,
};
use price_aggregator::PriceAggregator;
use stablecoin_v3::config::ConfigModule;
use stablecoin_v3::*;

pub static COLLATERAL_TOKEN_ID: &[u8] = b"COLLATERAL-123456";
pub static STABLECOIN_TOKEN_ID: &[u8] = b"STABLE-123456";
pub static INITIAL_STABLECOIN_ID: &[u8] = b"WUSDC-abcdef";
pub static COLLATERAL_TOKEN_TICKER: &[u8] = b"EGLD";
pub static STABLECOIN_TOKEN_TICKER: &[u8] = b"USD";
pub static INITIAL_STABLECOIN_TOKEN_TICKER: &[u8] = b"WUSDC";
pub static ESDT_ROLES: &[EsdtLocalRole] = &[
    EsdtLocalRole::Mint,
    EsdtLocalRole::Burn,
    EsdtLocalRole::Transfer,
];

// pub const EGLD_DECIMALS: u64 = 1_000_000_000_000_000_000;
pub const POOL_RECOVERY_PERIOD: u64 = 100;

pub struct StablecoinContractSetup<StablecoinContractObjBuilder>
where
    StablecoinContractObjBuilder: 'static + Copy + Fn() -> stablecoin_v3::ContractObj<DebugApi>,
{
    pub b_mock: BlockchainStateWrapper,
    pub owner_address: Address,
    pub sc_wrapper:
        ContractObjWrapper<stablecoin_v3::ContractObj<DebugApi>, StablecoinContractObjBuilder>,
}

impl<StablecoinContractObjBuilder> StablecoinContractSetup<StablecoinContractObjBuilder>
where
    StablecoinContractObjBuilder: 'static + Copy + Fn() -> stablecoin_v3::ContractObj<DebugApi>,
{
    pub fn new(sc_builder: StablecoinContractObjBuilder) -> Self {
        let rust_zero = rust_biguint!(0u64);
        let mut b_mock = BlockchainStateWrapper::new();
        let owner_address = b_mock.create_user_account(&rust_zero);

        let price_aggregator_address = Self::init_price_aggregator(&mut b_mock, &owner_address);
        let sc_wrapper = b_mock.create_sc_account(
            &rust_zero,
            Some(&owner_address),
            sc_builder,
            "stablecoin_v3.wasm",
        );

        b_mock
            .execute_tx(&owner_address, &sc_wrapper, &rust_zero, |sc| {
                sc.init(
                    managed_address!(&price_aggregator_address),
                    POOL_RECOVERY_PERIOD,
                );
            })
            .assert_ok();

        b_mock
            .execute_tx(&owner_address, &sc_wrapper, &rust_zero, |sc| {
                sc.stablecoin()
                    .set_token_id(&managed_token_id!(STABLECOIN_TOKEN_ID));
            })
            .assert_ok();

        b_mock.set_esdt_balance(
            &owner_address,
            COLLATERAL_TOKEN_ID,
            &Self::exp18(1_000_000),
        );
        b_mock.set_esdt_local_roles(sc_wrapper.address_ref(), STABLECOIN_TOKEN_ID, ESDT_ROLES);

        b_mock
            .execute_esdt_transfer(
                &owner_address,
                &sc_wrapper,
                COLLATERAL_TOKEN_ID,
                0,
                &Self::exp18(1_000_000),
                |sc| {
                    sc.deploy_stablecoin(
                        TokenIdentifier::from_esdt_bytes(COLLATERAL_TOKEN_ID),
                        ManagedBuffer::new_from_bytes(COLLATERAL_TOKEN_TICKER),
                        ManagedBuffer::new_from_bytes(STABLECOIN_TOKEN_TICKER),
                        TokenIdentifier::from_esdt_bytes(INITIAL_STABLECOIN_ID),
                        ManagedBuffer::new_from_bytes(INITIAL_STABLECOIN_TOKEN_TICKER),
                        managed_biguint!(1000u64), // 1%
                    );
                },
            )
            .assert_ok();

        b_mock
            .execute_tx(&owner_address, &sc_wrapper, &rust_zero, |sc| {
                sc.resume();
            })
            .assert_ok();

        // check internal state & balances
        b_mock
            .execute_query(&sc_wrapper, |sc| {
                assert_eq!(sc.stablecoin_supply().get(), Self::to_managed_biguint(Self::exp18(100_000_000)));
                assert_eq!(sc.collateral_supply().get(), Self::to_managed_biguint(Self::exp18(1_000_000)));
                assert_eq!(sc.base_pool().get(), Self::to_managed_biguint(Self::exp18(100_000_000)))
            })
            .assert_ok();

        b_mock.check_esdt_balance(
            &owner_address,
            STABLECOIN_TOKEN_ID,
            &Self::exp18(100_000_000),
        );

        StablecoinContractSetup {
            b_mock,
            owner_address,
            sc_wrapper,
        }
    }

    fn init_price_aggregator(
        b_mock: &mut BlockchainStateWrapper,
        owner_address: &Address,
    ) -> Address {
        let rust_zero = rust_biguint!(0);
        let price_aggregator_wrapper = b_mock.create_sc_account(
            &rust_zero,
            Some(&owner_address),
            price_aggregator::contract_obj,
            "price_aggregator.wasm",
        );
        let oracle = b_mock.create_user_account(&rust_zero);

        b_mock
            .execute_tx(owner_address, &price_aggregator_wrapper, &rust_zero, |sc| {
                sc.init(
                    TokenIdentifier::egld(),
                    ManagedVec::from_single_item(managed_address!(&oracle)),
                    1,
                    0,
                    managed_biguint!(0),
                );
            })
            .assert_ok();

        b_mock
            .execute_tx(&oracle, &price_aggregator_wrapper, &rust_zero, |sc| {
                sc.submit(
                    managed_buffer!(b"EGLD"), // managed_buffer!(COLLATERAL_TOKEN_ID),// managed_buffer!(b"EGLD"),
                    managed_buffer!(b"USD"), // managed_buffer!(STABLECOIN_TOKEN_ID),// managed_buffer!(b"USD"),
                    managed_biguint!(100),
                );
            })
            .assert_ok();

        b_mock
            .execute_tx(&oracle, &price_aggregator_wrapper, &rust_zero, |sc| {
                sc.submit(
                    managed_buffer!(b"EGLD"), // managed_buffer!(COLLATERAL_TOKEN_ID),// managed_buffer!(b"EGLD"),
                    managed_buffer!(b"WUSDC"), // managed_buffer!(STABLECOIN_TOKEN_ID),// managed_buffer!(b"USD"),
                    managed_biguint!(100),
                );
            })
            .assert_ok();

        b_mock
            .execute_tx(&oracle, &price_aggregator_wrapper, &rust_zero, |sc| {
                sc.submit(
                    managed_buffer!(b"USD"), // managed_buffer!(COLLATERAL_TOKEN_ID),// managed_buffer!(b"EGLD"),
                    managed_buffer!(b"USD"), // managed_buffer!(STABLECOIN_TOKEN_ID),// managed_buffer!(b"USD"),
                    managed_biguint!(1),
                );
            })
            .assert_ok();

        price_aggregator_wrapper.address_ref().clone()
    }

    pub fn exp18(value: u64) -> num_bigint::BigUint {
        value.mul(rust_biguint!(10).pow(18))
    }

    pub fn to_managed_biguint(value: num_bigint::BigUint) -> BigUint<DebugApi> {
        BigUint::from_bytes_be(&value.to_bytes_be())
    }
    
}