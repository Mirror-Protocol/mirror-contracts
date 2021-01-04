use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, Order, StdError, StdResult, Storage, Uint128};
use cosmwasm_storage::{singleton, singleton_read, Bucket, ReadonlyBucket, Singleton};

use mirror_protocol::factory::Params;

static KEY_CONFIG: &[u8] = b"config";
static KEY_PARAMS: &[u8] = b"params";
static KEY_TOTAL_WEIGHT: &[u8] = b"total_weight";
static KEY_LAST_DISTRIBUTED: &[u8] = b"last_distributed";

static PREFIX_WEIGHT: &[u8] = b"weight";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub mirror_token: CanonicalAddr,
    pub mint_contract: CanonicalAddr,
    pub oracle_contract: CanonicalAddr,
    pub terraswap_factory: CanonicalAddr,
    pub staking_contract: CanonicalAddr,
    pub commission_collector: CanonicalAddr,
    pub token_code_id: u64, // used to create asset token
    pub base_denom: String,
    pub genesis_time: u64,
    pub distribution_schedule: Vec<(u64, u64, Uint128)>, // [[start_time, end_time, distribution_amount], [], ...]
}

pub fn store_config<S: Storage>(storage: &mut S, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config<S: Storage>(storage: &S) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}

pub fn store_params<S: Storage>(storage: &mut S, init_data: &Params) -> StdResult<()> {
    singleton(storage, KEY_PARAMS).save(init_data)
}

pub fn remove_params<S: Storage>(storage: &mut S) {
    let mut store: Singleton<S, Params> = singleton(storage, KEY_PARAMS);
    store.remove()
}

pub fn read_params<S: Storage>(storage: &S) -> StdResult<Params> {
    singleton_read(storage, KEY_PARAMS).load()
}

pub fn store_total_weight<S: Storage>(storage: &mut S, total_weight: u32) -> StdResult<()> {
    singleton(storage, KEY_TOTAL_WEIGHT).save(&total_weight)
}

pub fn increase_total_weight<S: Storage>(storage: &mut S, weight_increase: u32) -> StdResult<u32> {
    let mut store: Singleton<S, u32> = singleton(storage, KEY_TOTAL_WEIGHT);
    store.update(|total_weight| Ok(total_weight + weight_increase))
}

pub fn decrease_total_weight<S: Storage>(storage: &mut S, weight_decrease: u32) -> StdResult<u32> {
    let mut store: Singleton<S, u32> = singleton(storage, KEY_TOTAL_WEIGHT);
    store.update(|total_weight| Ok(total_weight - weight_decrease))
}

pub fn read_total_weight<S: Storage>(storage: &S) -> StdResult<u32> {
    singleton_read(storage, KEY_TOTAL_WEIGHT).load()
}

pub fn store_last_distributed<S: Storage>(storage: &mut S, last_distributed: u64) -> StdResult<()> {
    let mut store: Singleton<S, u64> = singleton(storage, KEY_LAST_DISTRIBUTED);
    store.save(&last_distributed)
}

pub fn read_last_distributed<S: Storage>(storage: &S) -> StdResult<u64> {
    singleton_read(storage, KEY_LAST_DISTRIBUTED).load()
}

pub fn store_weight<S: Storage>(
    storage: &mut S,
    asset_token: &CanonicalAddr,
    weight: u32,
) -> StdResult<()> {
    let mut weight_bucket: Bucket<S, u32> = Bucket::new(PREFIX_WEIGHT, storage);
    weight_bucket.save(asset_token.as_slice(), &weight)
}

pub fn read_weight<S: Storage>(storage: &S, asset_token: &CanonicalAddr) -> StdResult<u32> {
    let weight_bucket: ReadonlyBucket<S, u32> = ReadonlyBucket::new(PREFIX_WEIGHT, storage);
    match weight_bucket.load(asset_token.as_slice()) {
        Ok(v) => Ok(v),
        _ => Err(StdError::generic_err("No distribution info stored")),
    }
}

pub fn remove_weight<S: Storage>(storage: &mut S, asset_token: &CanonicalAddr) {
    let mut weight_bucket: Bucket<S, u32> = Bucket::new(PREFIX_WEIGHT, storage);
    weight_bucket.remove(asset_token.as_slice());
}

pub fn read_all_weight<S: Storage>(storage: &S) -> StdResult<Vec<(CanonicalAddr, u32)>> {
    let weight_bucket: ReadonlyBucket<S, u32> = ReadonlyBucket::new(PREFIX_WEIGHT, storage);
    weight_bucket
        .range(None, None, Order::Ascending)
        .map(|item| {
            let (k, v) = item?;
            Ok((CanonicalAddr::from(k), v))
        })
        .collect()
}
