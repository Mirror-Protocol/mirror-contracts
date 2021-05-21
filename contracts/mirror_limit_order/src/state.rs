use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, StdError, StdResult, Storage, Uint128};
use cosmwasm_storage::{singleton, singleton_read, Bucket, ReadonlyBucket};

use mirror_protocol::common::OrderBy;
use std::convert::TryInto;
use terraswap::asset::AssetRaw;

static KEY_LAST_ORDER_ID: &[u8] = b"last_order_id";

static PREFIX_ORDER: &[u8] = b"order";
static PREFIX_ORDER_BY_BIDDER: &[u8] = b"order_by_bidder";

pub fn init_last_order_id(storage: &mut dyn Storage) -> StdResult<()> {
    singleton(storage, KEY_LAST_ORDER_ID).save(&0u64)
}

pub fn increase_last_order_id(storage: &mut dyn Storage) -> StdResult<u64> {
    singleton(storage, KEY_LAST_ORDER_ID).update(|v| Ok(v + 1))
}

pub fn read_last_order_id(storage: &dyn Storage) -> StdResult<u64> {
    singleton_read(storage, KEY_LAST_ORDER_ID).load()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Order {
    pub order_id: u64,
    pub bidder_addr: CanonicalAddr,
    pub offer_asset: AssetRaw,
    pub ask_asset: AssetRaw,
    pub filled_offer_amount: Uint128,
    pub filled_ask_amount: Uint128,
}

pub fn store_order(storage: &mut dyn Storage, order: &Order) -> StdResult<()> {
    Bucket::new(storage, PREFIX_ORDER).save(&order.order_id.to_be_bytes(), order)?;
    Bucket::multilevel(
        storage,
        &[PREFIX_ORDER_BY_BIDDER, order.bidder_addr.as_slice()],
    )
    .save(&order.order_id.to_be_bytes(), &true)?;

    Ok(())
}

pub fn remove_order(storage: &mut dyn Storage, order: &Order) {
    Bucket::<Order>::new(storage, PREFIX_ORDER).remove(&order.order_id.to_be_bytes());
    Bucket::<Order>::multilevel(
        storage,
        &[PREFIX_ORDER_BY_BIDDER, order.bidder_addr.as_slice()],
    )
    .remove(&order.order_id.to_be_bytes());
}

pub fn read_order(storage: &dyn Storage, order_id: u64) -> StdResult<Order> {
    ReadonlyBucket::new(storage, PREFIX_ORDER).load(&order_id.to_be_bytes())
}

pub fn read_orders_with_bidder_indexer(
    storage: &dyn Storage,
    bidder_addr: &CanonicalAddr,
    start_after: Option<u64>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<Vec<Order>> {
    let position_indexer: ReadonlyBucket<bool> =
        ReadonlyBucket::multilevel(storage, &[PREFIX_ORDER_BY_BIDDER, bidder_addr.as_slice()]);

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
            read_order(storage, bytes_to_u64(&k)?)
        })
        .collect()
}

// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;
pub fn read_orders(
    storage: &dyn Storage,
    start_after: Option<u64>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<Vec<Order>> {
    let position_bucket: ReadonlyBucket<Order> = ReadonlyBucket::new(storage, PREFIX_ORDER);

    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let (start, end, order_by) = match order_by {
        Some(OrderBy::Asc) => (calc_range_start(start_after), None, OrderBy::Asc),
        _ => (None, calc_range_end(start_after), OrderBy::Desc),
    };

    position_bucket
        .range(start.as_deref(), end.as_deref(), order_by.into())
        .take(limit)
        .map(|item| {
            let (_, v) = item?;
            Ok(v)
        })
        .collect()
}

fn bytes_to_u64(data: &[u8]) -> StdResult<u64> {
    match data[0..8].try_into() {
        Ok(bytes) => Ok(u64::from_be_bytes(bytes)),
        Err(_) => Err(StdError::generic_err(
            "Corrupted data found. 8 byte expected.",
        )),
    }
}

// this will set the first key after the provided key, by appending a 1 byte
fn calc_range_start(start_after: Option<u64>) -> Option<Vec<u8>> {
    start_after.map(|id| {
        let mut v = id.to_be_bytes().to_vec();
        v.push(1);
        v
    })
}

// this will set the first key after the provided key, by appending a 1 byte
fn calc_range_end(start_after: Option<u64>) -> Option<Vec<u8>> {
    start_after.map(|id| id.to_be_bytes().to_vec())
}
