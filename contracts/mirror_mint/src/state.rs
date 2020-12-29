use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    CanonicalAddr, Decimal, ReadonlyStorage, StdError, StdResult, Storage, Uint128,
};

use cosmwasm_storage::{singleton, singleton_read, Bucket, ReadonlyBucket};
use std::convert::TryInto;
use terraswap::{AssetInfoRaw, AssetRaw};

use crate::msg::OrderBy;

static PREFIX_ASSET_CONFIG: &[u8] = b"asset_config";
static PREFIX_POSITION: &[u8] = b"position";
static PREFIX_INDEX_BY_USER: &[u8] = b"by_user";
static PREFIX_INDEX_BY_ASSET: &[u8] = b"by_asset";

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
    pub collector: CanonicalAddr,
    pub base_denom: String,
    pub token_code_id: u64,
    pub protocol_fee_rate: Decimal,
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
    pub end_price: Option<Decimal>,
}

pub fn store_asset_config<S: Storage>(
    storage: &mut S,
    asset_token: &CanonicalAddr,
    asset: &AssetConfig,
) -> StdResult<()> {
    let mut asset_bucket: Bucket<S, AssetConfig> = Bucket::new(PREFIX_ASSET_CONFIG, storage);
    asset_bucket.save(asset_token.as_slice(), &asset)
}

pub fn read_asset_config<S: Storage>(
    storage: &S,
    asset_token: &CanonicalAddr,
) -> StdResult<AssetConfig> {
    let asset_bucket: ReadonlyBucket<S, AssetConfig> =
        ReadonlyBucket::new(PREFIX_ASSET_CONFIG, storage);
    let res = asset_bucket.load(asset_token.as_slice());
    match res {
        Ok(data) => Ok(data),
        _ => Err(StdError::generic_err("no asset data stored")),
    }
}

pub fn read_end_price<S: Storage>(storage: &S, asset_info: &AssetInfoRaw) -> Option<Decimal> {
    match asset_info {
        AssetInfoRaw::Token { contract_addr } => {
            let asset_bucket: ReadonlyBucket<S, AssetConfig> =
                ReadonlyBucket::new(PREFIX_ASSET_CONFIG, storage);
            let res = asset_bucket.load(contract_addr.as_slice());
            match res {
                Ok(data) => data.end_price,
                _ => None,
            }
        }
        _ => None,
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
pub fn create_position<S: Storage>(
    storage: &mut S,
    idx: Uint128,
    position: &Position,
) -> StdResult<()> {
    let mut position_bucket: Bucket<S, Position> = Bucket::new(PREFIX_POSITION, storage);
    position_bucket.save(&idx.u128().to_be_bytes(), &position)?;

    let mut position_indexer_by_user: Bucket<S, bool> =
        Bucket::multilevel(&[PREFIX_INDEX_BY_USER, position.owner.as_slice()], storage);
    position_indexer_by_user.save(&idx.u128().to_be_bytes(), &true)?;

    let mut position_indexer_by_asset: Bucket<S, bool> = Bucket::multilevel(
        &[PREFIX_INDEX_BY_ASSET, position.asset.info.as_bytes()],
        storage,
    );
    position_indexer_by_asset.save(&idx.u128().to_be_bytes(), &true)?;

    Ok(())
}

/// store position with idx
pub fn store_position<S: Storage>(
    storage: &mut S,
    idx: Uint128,
    position: &Position,
) -> StdResult<()> {
    let mut position_bucket: Bucket<S, Position> = Bucket::new(PREFIX_POSITION, storage);
    position_bucket.save(&idx.u128().to_be_bytes(), &position)?;
    Ok(())
}

/// remove position with idx
pub fn remove_position<S: Storage>(storage: &mut S, idx: Uint128) -> StdResult<()> {
    let position: Position = read_position(storage, idx)?;
    let mut position_bucket: Bucket<S, Position> = Bucket::new(PREFIX_POSITION, storage);
    position_bucket.remove(&idx.u128().to_be_bytes());

    let mut position_indexer_by_user: Bucket<S, bool> =
        Bucket::multilevel(&[PREFIX_INDEX_BY_USER, position.owner.as_slice()], storage);
    position_indexer_by_user.remove(&idx.u128().to_be_bytes());

    let mut position_indexer_by_asset: Bucket<S, bool> = Bucket::multilevel(
        &[PREFIX_INDEX_BY_ASSET, position.asset.info.as_bytes()],
        storage,
    );
    position_indexer_by_asset.remove(&idx.u128().to_be_bytes());

    Ok(())
}

/// read position from store with position idx
pub fn read_position<S: ReadonlyStorage>(storage: &S, idx: Uint128) -> StdResult<Position> {
    let position_bucket: ReadonlyBucket<S, Position> =
        ReadonlyBucket::new(PREFIX_POSITION, storage);
    position_bucket.load(&idx.u128().to_be_bytes())
}

// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;
pub fn read_positions<S: ReadonlyStorage>(
    storage: &S,
    start_after: Option<Uint128>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<Vec<Position>> {
    let position_bucket: ReadonlyBucket<S, Position> =
        ReadonlyBucket::new(PREFIX_POSITION, storage);

    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = calc_range_start(start_after);

    position_bucket
        .range(
            start.as_deref(),
            None,
            order_by.unwrap_or(OrderBy::Desc).into(),
        )
        .take(limit)
        .map(|item| {
            let (_, v) = item?;
            Ok(v)
        })
        .collect()
}

pub fn read_positions_with_user_indexer<S: ReadonlyStorage>(
    storage: &S,
    position_owner: &CanonicalAddr,
    start_after: Option<Uint128>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<Vec<Position>> {
    let position_indexer: ReadonlyBucket<S, bool> =
        ReadonlyBucket::multilevel(&[PREFIX_INDEX_BY_USER, position_owner.as_slice()], storage);

    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let (start, end, order_by) = match order_by {
        Some(OrderBy::Asc) => (calc_range_start(start_after), None, OrderBy::Asc),
        _ => (None, calc_range_end(start_after), OrderBy::Desc),
    };

    position_indexer
        .range(start.as_deref(), end.as_deref(), order_by.into())
        .take(limit)
        .map(|item| {
            let (k, _) = item?;
            read_position(storage, Uint128(bytes_to_u128(&k)?))
        })
        .collect()
}

pub fn read_positions_with_asset_indexer<S: ReadonlyStorage>(
    storage: &S,
    asset_token: &CanonicalAddr,
    start_after: Option<Uint128>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<Vec<Position>> {
    let position_indexer: ReadonlyBucket<S, bool> =
        ReadonlyBucket::multilevel(&[PREFIX_INDEX_BY_ASSET, asset_token.as_slice()], storage);

    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let (start, end, order_by) = match order_by {
        Some(OrderBy::Asc) => (calc_range_start(start_after), None, OrderBy::Asc),
        _ => (None, calc_range_end(start_after), OrderBy::Desc),
    };

    position_indexer
        .range(start.as_deref(), end.as_deref(), order_by.into())
        .take(limit)
        .map(|item| {
            let (k, _) = item?;
            read_position(storage, Uint128(bytes_to_u128(&k)?))
        })
        .collect()
}

fn bytes_to_u128(data: &[u8]) -> StdResult<u128> {
    match data[0..16].try_into() {
        Ok(bytes) => Ok(u128::from_be_bytes(bytes)),
        Err(_) => Err(StdError::generic_err(
            "Corrupted data found. 16 byte expected.",
        )),
    }
}

// this will set the first key after the provided key, by appending a 1 byte
fn calc_range_start(start_after: Option<Uint128>) -> Option<Vec<u8>> {
    start_after.map(|idx| {
        let mut v = idx.u128().to_be_bytes().to_vec();
        v.push(1);
        v
    })
}

// this will set the first key after the provided key, by appending a 1 byte
fn calc_range_end(start_after: Option<Uint128>) -> Option<Vec<u8>> {
    start_after.map(|idx| idx.u128().to_be_bytes().to_vec())
}
