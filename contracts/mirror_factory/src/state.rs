use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, CanonicalAddr, Order, StdError, StdResult, Storage, Uint128};
use cosmwasm_storage::{singleton, singleton_read, Bucket, ReadonlyBucket, Singleton};

use mirror_protocol::factory::Params;

static KEY_CONFIG: &[u8] = b"config";
static KEY_TOTAL_WEIGHT: &[u8] = b"total_weight";
static KEY_LAST_DISTRIBUTED: &[u8] = b"last_distributed";
static KEY_WHITELIST_TMP_INFO: &[u8] = b"tmp_whitelist_info";
static KEY_TMP_ASSET: &[u8] = b"tmp_asset_token";

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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct WhitelistTmpInfo {
    pub params: Params,
    pub oracle_proxy: CanonicalAddr,
    pub symbol: String,
}

pub fn store_tmp_asset(storage: &mut dyn Storage, tmp_asset: &Addr) -> StdResult<()> {
    singleton(storage, KEY_TMP_ASSET).save(tmp_asset)
}

pub fn read_tmp_asset(storage: &dyn Storage) -> StdResult<Addr> {
    singleton_read(storage, KEY_TMP_ASSET).load()
}

pub fn store_config(storage: &mut dyn Storage, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config(storage: &dyn Storage) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}

pub fn store_tmp_whitelist_info(
    storage: &mut dyn Storage,
    info: &WhitelistTmpInfo,
) -> StdResult<()> {
    singleton(storage, KEY_WHITELIST_TMP_INFO).save(info)
}

pub fn remove_tmp_whitelist_info(storage: &mut dyn Storage) {
    let mut store: Singleton<WhitelistTmpInfo> = singleton(storage, KEY_WHITELIST_TMP_INFO);
    store.remove()
}

pub fn read_tmp_whitelist_info(storage: &dyn Storage) -> StdResult<WhitelistTmpInfo> {
    singleton_read(storage, KEY_WHITELIST_TMP_INFO).load()
}

pub fn store_total_weight(storage: &mut dyn Storage, total_weight: u32) -> StdResult<()> {
    singleton(storage, KEY_TOTAL_WEIGHT).save(&total_weight)
}

pub fn increase_total_weight(storage: &mut dyn Storage, weight_increase: u32) -> StdResult<u32> {
    let mut store: Singleton<u32> = singleton(storage, KEY_TOTAL_WEIGHT);
    store.update(|total_weight| Ok(total_weight + weight_increase))
}

pub fn decrease_total_weight(storage: &mut dyn Storage, weight_decrease: u32) -> StdResult<u32> {
    let mut store: Singleton<u32> = singleton(storage, KEY_TOTAL_WEIGHT);
    store.update(|total_weight| Ok(total_weight - weight_decrease))
}

pub fn read_total_weight(storage: &dyn Storage) -> StdResult<u32> {
    singleton_read(storage, KEY_TOTAL_WEIGHT).load()
}

pub fn store_last_distributed(storage: &mut dyn Storage, last_distributed: u64) -> StdResult<()> {
    let mut store: Singleton<u64> = singleton(storage, KEY_LAST_DISTRIBUTED);
    store.save(&last_distributed)
}

pub fn read_last_distributed(storage: &dyn Storage) -> StdResult<u64> {
    singleton_read(storage, KEY_LAST_DISTRIBUTED).load()
}

pub fn store_weight(
    storage: &mut dyn Storage,
    asset_token: &CanonicalAddr,
    weight: u32,
) -> StdResult<()> {
    let mut weight_bucket: Bucket<u32> = Bucket::new(storage, PREFIX_WEIGHT);
    weight_bucket.save(asset_token.as_slice(), &weight)
}

pub fn read_weight(storage: &dyn Storage, asset_token: &CanonicalAddr) -> StdResult<u32> {
    let weight_bucket: ReadonlyBucket<u32> = ReadonlyBucket::new(storage, PREFIX_WEIGHT);
    match weight_bucket.load(asset_token.as_slice()) {
        Ok(v) => Ok(v),
        _ => Err(StdError::generic_err("No distribution info stored")),
    }
}

pub fn remove_weight(storage: &mut dyn Storage, asset_token: &CanonicalAddr) {
    let mut weight_bucket: Bucket<u32> = Bucket::new(storage, PREFIX_WEIGHT);
    weight_bucket.remove(asset_token.as_slice());
}

pub fn read_all_weight(storage: &dyn Storage) -> StdResult<Vec<(CanonicalAddr, u32)>> {
    let weight_bucket: ReadonlyBucket<u32> = ReadonlyBucket::new(storage, PREFIX_WEIGHT);
    weight_bucket
        .range(None, None, Order::Ascending)
        .map(|item| {
            let (k, v) = item?;
            Ok((CanonicalAddr::from(k), v))
        })
        .collect()
}
