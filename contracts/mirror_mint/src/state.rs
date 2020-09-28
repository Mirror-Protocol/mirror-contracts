use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    from_slice, to_vec, CanonicalAddr, Decimal, Order, ReadonlyStorage, StdError, StdResult,
    Storage, Uint128,
};

use cosmwasm_storage::{
    singleton, singleton_read, Bucket, PrefixedStorage, ReadonlyBucket, ReadonlyPrefixedStorage,
};
use terraswap::{AssetInfoRaw, AssetRaw};

static PREFIX_ASSET: &[u8] = b"asset";
static PREFIX_POSITION: &[u8] = b"position";
static PREFIX_USER: &[u8] = b"user";

static KEY_CONFIG: &[u8] = b"config";
static KEY_POSITION_IDX: &[u8] = b"position_idx";

pub fn store_position_idx<S: Storage>(storage: &mut S, position_idx: Uint128) -> StdResult<()> {
    singleton(storage, KEY_POSITION_IDX).save(&position_idx)
}

pub fn read_position_idx<S: Storage>(storage: &S) -> StdResult<Uint128> {
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
    pub idx: Uint128,
    pub owner: CanonicalAddr,
    pub collateral: AssetRaw,
    pub asset: AssetRaw,
}

/// create position with index
pub fn create_position<'a, S: Storage>(
    storage: &'a mut S,
    idx: Uint128,
    position: &Position,
) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_POSITION, storage).set(&to_vec(&idx)?, &to_vec(&position)?);

    let mut position_indexer: Bucket<'a, S, bool> =
        Bucket::multilevel(&[PREFIX_USER, position.owner.as_slice()], storage);
    position_indexer.save(&to_vec(&idx)?, &true)?;

    Ok(())
}

/// store position with idx
pub fn store_position<'a, S: Storage>(
    storage: &'a mut S,
    idx: Uint128,
    position: &Position,
) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_POSITION, storage).set(&to_vec(&idx)?, &to_vec(&position)?);
    Ok(())
}

/// remove position with idx
pub fn remove_position<'a, S: Storage>(
    storage: &'a mut S,
    idx: Uint128,
    position_owner: &CanonicalAddr,
) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_POSITION, storage).remove(&to_vec(&idx)?);

    let mut position_indexer: Bucket<'a, S, bool> =
        Bucket::multilevel(&[PREFIX_USER, position_owner.as_slice()], storage);
    position_indexer.remove(&to_vec(&idx)?);

    Ok(())
}

/// read position from store with position idx
pub fn read_position<'a, S: ReadonlyStorage>(storage: &'a S, idx: Uint128) -> StdResult<Position> {
    let res = ReadonlyPrefixedStorage::new(PREFIX_POSITION, storage).get(&to_vec(&idx)?);
    match res {
        Some(v) => from_slice(&v),
        None => Err(StdError::generic_err(
            "No position info exists for the given idx",
        )),
    }
}

// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;
pub fn read_positions<'a, S: ReadonlyStorage>(
    storage: &'a S,
    position_owner: &CanonicalAddr,
    start_after: Option<Uint128>,
    limit: Option<u32>,
) -> StdResult<Vec<Position>> {
    let position_indexer: ReadonlyBucket<'a, S, bool> =
        ReadonlyBucket::multilevel(&[PREFIX_USER, position_owner.as_slice()], storage);

    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = calc_range_start(start_after);

    position_indexer
        .range(start.as_deref(), None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (k, _) = item?;
            read_position(storage, from_slice(&k)?)
        })
        .collect()
}

// this will set the first key after the provided key, by appending a 1 byte
fn calc_range_start(start_after: Option<Uint128>) -> Option<Vec<u8>> {
    start_after.map(|idx| {
        let mut v = to_vec(&idx).unwrap();
        v.push(1);
        v
    })
}
