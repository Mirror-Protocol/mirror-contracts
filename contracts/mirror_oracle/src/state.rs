use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, Decimal, Deps, StdResult, Storage};
use cosmwasm_storage::{singleton, singleton_read, Bucket, ReadonlyBucket};

use mirror_protocol::common::OrderBy;
use mirror_protocol::oracle::PricesResponseElem;

static PREFIX_FEEDER: &[u8] = b"feeder";
static PREFIX_PRICE: &[u8] = b"price";

static KEY_CONFIG: &[u8] = b"config";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub base_asset: String,
}

pub fn store_config(storage: &mut dyn Storage, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config(storage: &dyn Storage) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}

pub fn store_feeder(
    storage: &mut dyn Storage,
    asset_token: &CanonicalAddr,
    feeder: &CanonicalAddr,
) -> StdResult<()> {
    let mut feeder_bucket: Bucket<CanonicalAddr> = Bucket::new(storage, PREFIX_FEEDER);

    feeder_bucket.save(asset_token.as_slice(), feeder)
}

pub fn read_feeder(storage: &dyn Storage, asset_token: &CanonicalAddr) -> StdResult<CanonicalAddr> {
    let feeder_bucket: ReadonlyBucket<CanonicalAddr> = ReadonlyBucket::new(storage, PREFIX_FEEDER);
    feeder_bucket.load(asset_token.as_slice())
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PriceInfo {
    pub price: Decimal,
    pub last_updated_time: u64,
}

pub fn store_price(
    storage: &mut dyn Storage,
    asset_token: &CanonicalAddr,
    price: &PriceInfo,
) -> StdResult<()> {
    let mut price_bucket: Bucket<PriceInfo> = Bucket::new(storage, PREFIX_PRICE);
    price_bucket.save(asset_token.as_slice(), price)
}

pub fn read_price(storage: &dyn Storage, asset_token: &CanonicalAddr) -> StdResult<PriceInfo> {
    let price_bucket: ReadonlyBucket<PriceInfo> = ReadonlyBucket::new(storage, PREFIX_PRICE);
    price_bucket.load(asset_token.as_slice())
}

// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;
pub fn read_prices(
    deps: Deps,
    start_after: Option<CanonicalAddr>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<Vec<PricesResponseElem>> {
    let price_bucket: ReadonlyBucket<PriceInfo> = ReadonlyBucket::new(deps.storage, PREFIX_PRICE);

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

            let asset_token = deps.api.addr_humanize(&CanonicalAddr::from(k))?.to_string();
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
