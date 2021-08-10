use cosmwasm_std::{CanonicalAddr, StdResult, Storage, Uint128};
use cosmwasm_storage::{
    singleton, singleton_read, Bucket, ReadonlyBucket, ReadonlySingleton, Singleton,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

static PREFIX_LOCK_INFOS: &[u8] = b"lock_infos";
static KEY_CONFIG: &[u8] = b"config";
static KEY_TOTAL_LOCKED_FUNDS: &[u8] = b"total_locked_funds";

pub fn total_locked_funds_store(storage: &mut dyn Storage) -> Singleton<Uint128> {
    singleton(storage, KEY_TOTAL_LOCKED_FUNDS)
}

pub fn total_locked_funds_read(storage: &dyn Storage) -> ReadonlySingleton<Uint128> {
    singleton_read(storage, KEY_TOTAL_LOCKED_FUNDS)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub mint_contract: CanonicalAddr,
    pub base_denom: String,
    pub lockup_period: u64,
}

pub fn store_config(storage: &mut dyn Storage, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config(storage: &dyn Storage) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionLockInfo {
    pub idx: Uint128,
    pub receiver: CanonicalAddr,
    pub locked_amount: Uint128,
    pub unlock_time: u64,
}

pub fn store_position_lock_info(
    storage: &mut dyn Storage,
    lock_info: &PositionLockInfo,
) -> StdResult<()> {
    let mut lock_infos_bucket: Bucket<PositionLockInfo> = Bucket::new(storage, PREFIX_LOCK_INFOS);
    lock_infos_bucket.save(&lock_info.idx.u128().to_be_bytes(), &lock_info)
}

pub fn read_position_lock_info(storage: &dyn Storage, idx: Uint128) -> StdResult<PositionLockInfo> {
    let lock_infos_bucket: ReadonlyBucket<PositionLockInfo> =
        ReadonlyBucket::new(storage, PREFIX_LOCK_INFOS);
    lock_infos_bucket.load(&idx.u128().to_be_bytes())
}

pub fn remove_position_lock_info(storage: &mut dyn Storage, idx: Uint128) {
    let mut lock_infos_bucket: Bucket<PositionLockInfo> = Bucket::new(storage, PREFIX_LOCK_INFOS);
    lock_infos_bucket.remove(&idx.u128().to_be_bytes())
}
