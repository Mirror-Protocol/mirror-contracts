use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    from_slice, to_vec, CanonicalAddr, Decimal, ReadonlyStorage, StdError, StdResult, Storage,
};

use cosmwasm_storage::{singleton, singleton_read, PrefixedStorage, ReadonlyPrefixedStorage};

use uniswap::{AssetInfoRaw, AssetRaw};
static PREFIX_ASSET: &[u8] = b"asset";
static PREFIX_POSITION: &[u8] = b"position";

static KEY_CONFIG: &[u8] = b"config";
static KEY_POSITION_IDX: &[u8] = b"position_idx";

pub fn store_position_idx<S: Storage>(storage: &mut S, position_idx: u64) -> StdResult<()> {
    singleton(storage, KEY_POSITION_IDX).save(&position_idx)
}

pub fn read_position_idx<S: Storage>(storage: &S) -> StdResult<u64> {
    singleton_read(storage, KEY_POSITION_IDX).load()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub oracle: CanonicalAddr,
    pub base_asset_info: AssetInfoRaw,
    pub token_code_id: u64,
}

pub fn store_config<S: Storage>(storage: &mut S, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config<S: Storage>(storage: &S) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AssetConfig {
    pub token: CanonicalAddr,
    pub auction_discount: Decimal,
    pub min_collateral_ratio: Decimal,
}

pub fn store_asset_config<S: Storage>(
    storage: &mut S,
    asset_info: &AssetInfoRaw,
    asset: &AssetConfig,
) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_ASSET, storage).set(asset_info.as_bytes(), &to_vec(&asset)?);
    Ok(())
}

pub fn read_asset_config<S: Storage>(
    storage: &S,
    asset_info: &AssetInfoRaw,
) -> StdResult<AssetConfig> {
    let res = ReadonlyPrefixedStorage::new(PREFIX_ASSET, storage).get(asset_info.as_bytes());
    match res {
        Some(data) => from_slice(&data),
        None => Err(StdError::generic_err("no asset data stored")),
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Position {
    pub owner: CanonicalAddr,
    pub collateral: AssetRaw,
    pub asset: AssetRaw,
}

/// store position with idx
pub fn store_position<'a, S: Storage>(
    storage: &'a mut S,
    idx: u64,
    position: &Position,
) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_POSITION, storage).set(&idx.to_be_bytes(), &to_vec(&position)?);
    Ok(())
}

/// remove position with idx
pub fn remove_position<'a, S: Storage>(storage: &'a mut S, idx: u64) {
    PrefixedStorage::new(PREFIX_POSITION, storage).remove(&idx.to_be_bytes());
}

/// read position from store with position idx
pub fn read_position<'a, S: ReadonlyStorage>(storage: &'a S, idx: u64) -> StdResult<Position> {
    let res = ReadonlyPrefixedStorage::new(PREFIX_POSITION, storage).get(&idx.to_be_bytes());
    match res {
        Some(v) => from_slice(&v),
        None => Err(StdError::generic_err(
            "No position info exists for the given idx",
        )),
    }
}
