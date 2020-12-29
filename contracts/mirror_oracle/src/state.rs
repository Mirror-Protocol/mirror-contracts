use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Api, CanonicalAddr, Decimal, Extern, Querier, StdResult, Storage};
use cosmwasm_storage::{singleton, singleton_read, Bucket, ReadonlyBucket};

use crate::msg::{OrderBy, PricesResponseElem};

static PREFIX_FEEDER: &[u8] = b"feeder";
static PREFIX_PRICE: &[u8] = b"price";

static KEY_CONFIG: &[u8] = b"config";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub base_asset: String,
}

pub fn store_config<S: Storage>(storage: &mut S, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config<S: Storage>(storage: &S) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}

pub fn store_feeder<S: Storage>(
    storage: &mut S,
    asset_token: &CanonicalAddr,
    feeder: &CanonicalAddr,
) -> StdResult<()> {
    let mut feeder_bucket: Bucket<S, CanonicalAddr> = Bucket::new(PREFIX_FEEDER, storage);

    feeder_bucket.save(asset_token.as_slice(), feeder)
}

pub fn read_feeder<S: Storage>(
    storage: &S,
    asset_token: &CanonicalAddr,
) -> StdResult<CanonicalAddr> {
    let feeder_bucket: ReadonlyBucket<S, CanonicalAddr> =
        ReadonlyBucket::new(PREFIX_FEEDER, storage);
    feeder_bucket.load(asset_token.as_slice())
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PriceInfo {
    pub price: Decimal,
    pub last_updated_time: u64,
}

pub fn store_price<S: Storage>(
    storage: &mut S,
    asset_token: &CanonicalAddr,
    price: &PriceInfo,
) -> StdResult<()> {
    let mut price_bucket: Bucket<S, PriceInfo> = Bucket::new(PREFIX_PRICE, storage);
    price_bucket.save(asset_token.as_slice(), price)
}

pub fn read_price<S: Storage>(storage: &S, asset_token: &CanonicalAddr) -> StdResult<PriceInfo> {
    let price_bucket: ReadonlyBucket<S, PriceInfo> = ReadonlyBucket::new(PREFIX_PRICE, storage);
    price_bucket.load(asset_token.as_slice())
}

// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;
pub fn read_prices<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    start_after: Option<CanonicalAddr>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<Vec<PricesResponseElem>> {
    let price_bucket: ReadonlyBucket<S, PriceInfo> =
        ReadonlyBucket::new(PREFIX_PRICE, &deps.storage);

    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let (start, end, order_by) = match order_by {
        Some(OrderBy::Asc) => (calc_range_start(start_after), None, OrderBy::Asc),
        _ => (None, calc_range_end(start_after), OrderBy::Desc),
    };

    price_bucket
        .range(start.as_deref(), end.as_deref(), order_by.into())
        .take(limit)
        .map(|item| {
            let (k, v) = item?;

            let asset_token = deps.api.human_address(&CanonicalAddr::from(k))?;
            Ok(PricesResponseElem {
                asset_token,
                price: v.price,
                last_updated_time: v.last_updated_time,
            })
        })
        .collect()
}

// this will set the first key after the provided key, by appending a 1 byte
fn calc_range_start(start_after: Option<CanonicalAddr>) -> Option<Vec<u8>> {
    start_after.map(|idx| {
        let mut v = idx.as_slice().to_vec();
        v.push(1);
        v
    })
}

// this will set the first key after the provided key in Desc
fn calc_range_end(start_after: Option<CanonicalAddr>) -> Option<Vec<u8>> {
    start_after.map(|idx| idx.as_slice().to_vec())
}
