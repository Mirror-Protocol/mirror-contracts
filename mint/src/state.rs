use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    from_slice, CanonicalAddr, Decimal, ReadonlyStorage, StdResult, Storage, Uint128,
};
use cosmwasm_storage::{
    singleton, singleton_read, PrefixedStorage, ReadonlyPrefixedStorage, ReadonlySingleton,
    Singleton,
};

static CONFIG_KEY: &[u8] = b"config";
static ASSET_KEY: &[u8] = b"asset";
static POSITION_KEY: &[u8] = b"position";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigState {
    pub owner: CanonicalAddr,
    pub collateral_denom: String,
    pub auction_discount: Decimal,
    pub auction_threshold_rate: Decimal,
    pub mint_capacity: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AssetState {
    pub oracle: CanonicalAddr,
    pub token: CanonicalAddr,
    pub symbol: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionState {
    pub collateral_amount: Uint128,
    pub asset_amount: Uint128,
}

pub fn config_store<S: Storage>(storage: &mut S) -> Singleton<S, ConfigState> {
    singleton(storage, CONFIG_KEY)
}

pub fn config_read<S: Storage>(storage: &S) -> ReadonlySingleton<S, ConfigState> {
    singleton_read(storage, CONFIG_KEY)
}

pub fn asset_store<S: Storage>(storage: &mut S) -> Singleton<S, AssetState> {
    singleton(storage, ASSET_KEY)
}

pub fn asset_read<S: Storage>(storage: &S) -> ReadonlySingleton<S, AssetState> {
    singleton_read(storage, ASSET_KEY)
}

pub fn position_store<S: Storage>(storage: &mut S) -> PrefixedStorage<S> {
    PrefixedStorage::new(POSITION_KEY, storage)
}

pub fn position_read<S: Storage>(storage: &S, address: &CanonicalAddr) -> StdResult<PositionState> {
    let position_storage = ReadonlyPrefixedStorage::new(POSITION_KEY, storage);
    let result = position_storage.get(address.as_slice());

    match result {
        Some(data) => from_slice(&data),
        None => Ok(PositionState {
            collateral_amount: Uint128::from(0u128),
            asset_amount: Uint128::from(0u128),
        }),
    }
}
