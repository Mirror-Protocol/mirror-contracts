use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    from_slice, to_vec, CanonicalAddr, Decimal, ReadonlyStorage, StdError, StdResult, Storage,
};

use cosmwasm_storage::{
    singleton, singleton_read, Bucket, PrefixedStorage, ReadonlyBucket, ReadonlyPrefixedStorage,
};

use uniswap::{AssetInfoRaw, AssetRaw};
static PREFIX_ASSET: &[u8] = b"asset";
static PREFIX_POSITION: &[u8] = b"position";

static KEY_CONFIG: &[u8] = b"config";

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
    pub auction_threshold_ratio: Decimal,
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

pub fn remove_asset_config<S: Storage>(
    storage: &mut S,
    asset_info: &AssetInfoRaw,
) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_ASSET, storage).remove(asset_info.as_bytes());
    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Position {
    pub collateral: AssetRaw,
    pub asset: AssetRaw,
}

/// returns a bucket with all allowances authorized by this owner (query it by asset)
pub fn positions_store<'a, S: Storage>(
    storage: &'a mut S,
    owner: &CanonicalAddr,
) -> Bucket<'a, S, Position> {
    Bucket::multilevel(&[PREFIX_POSITION, owner.as_slice()], storage)
}

/// returns a bucket with all allowances authorized by this owner (query it by asset)
/// (read-only version for queries)
pub fn positions_read<'a, S: ReadonlyStorage>(
    storage: &'a S,
    owner: &CanonicalAddr,
) -> ReadonlyBucket<'a, S, Position> {
    ReadonlyBucket::multilevel(&[PREFIX_POSITION, owner.as_slice()], storage)
}
