use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, StdResult, Storage, Uint128};

use cosmwasm_storage::{
    singleton, singleton_read, Bucket, ReadonlyBucket, ReadonlySingleton, Singleton,
};

static PREFIX_LOCK_INFOS: &[u8] = b"lock_infos";

static KEY_CONFIG: &[u8] = b"config";
static KEY_TOTAL_LOCKED_FUNDS: &[u8] = b"total_locked_funds";

pub fn total_locked_funds_store<S: Storage>(storage: &mut S) -> Singleton<S, Uint128> {
    singleton(storage, KEY_TOTAL_LOCKED_FUNDS)
}

pub fn total_locked_funds_read<S: Storage>(storage: &S) -> ReadonlySingleton<S, Uint128> {
    singleton_read(storage, KEY_TOTAL_LOCKED_FUNDS)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub mint_contract: CanonicalAddr,
    pub base_denom: String,
    pub lockup_period: u64,
}

pub fn store_config<S: Storage>(storage: &mut S, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config<S: Storage>(storage: &S) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionLockInfo {
    pub idx: Uint128,
    pub receiver: CanonicalAddr,
    pub locked_funds: Vec<(u64, Uint128)>, // locking time and amount
}

pub fn store_position_lock_info<S: Storage>(
    storage: &mut S,
    lock_info: &PositionLockInfo,
) -> StdResult<()> {
    let mut lock_infos_bucket: Bucket<S, PositionLockInfo> =
        Bucket::new(PREFIX_LOCK_INFOS, storage);
    lock_infos_bucket.save(&lock_info.idx.u128().to_be_bytes(), &lock_info)
}

pub fn read_position_lock_info<S: Storage>(
    storage: &S,
    idx: Uint128,
) -> StdResult<PositionLockInfo> {
    let lock_infos_bucket: ReadonlyBucket<S, PositionLockInfo> =
        ReadonlyBucket::new(PREFIX_LOCK_INFOS, storage);
    lock_infos_bucket.load(&idx.u128().to_be_bytes())
}

pub fn remove_position_lock_info<S: Storage>(storage: &mut S, idx: Uint128) {
    let mut lock_infos_bucket: Bucket<S, PositionLockInfo> =
        Bucket::new(PREFIX_LOCK_INFOS, storage);
    lock_infos_bucket.remove(&idx.u128().to_be_bytes())
}
