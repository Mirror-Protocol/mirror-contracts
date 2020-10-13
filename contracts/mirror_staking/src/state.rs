use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    from_slice, to_vec, CanonicalAddr, Decimal, ReadonlyStorage, StdError, StdResult, Storage,
    Uint128,
};
use cosmwasm_storage::{
    singleton, singleton_read, Bucket, PrefixedStorage, ReadonlyBucket, ReadonlyPrefixedStorage,
};

static KEY_CONFIG: &[u8] = b"config";

static PREFIX_POOL_INFO: &[u8] = b"pool_info";
static PREFIX_REWARD: &[u8] = b"reward";
static PREFIX_MIGRATION: &[u8] = b"migration";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub mirror_token: CanonicalAddr,
}

pub fn store_config<S: Storage>(storage: &mut S, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config<S: Storage>(storage: &S) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolInfo {
    pub staking_token: CanonicalAddr,
    pub total_bond_amount: Uint128,
    pub reward_index: Decimal,
}

pub fn store_pool_info<S: Storage>(
    storage: &mut S,
    asset_token: &CanonicalAddr,
    pool_info: &PoolInfo,
) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_POOL_INFO, storage)
        .set(asset_token.as_slice(), &to_vec(&pool_info)?);
    Ok(())
}

pub fn read_pool_info<S: Storage>(storage: &S, asset_token: &CanonicalAddr) -> StdResult<PoolInfo> {
    let res = ReadonlyPrefixedStorage::new(PREFIX_POOL_INFO, storage).get(asset_token.as_slice());
    match res {
        Some(data) => from_slice(&data),
        None => Err(StdError::generic_err("no pool data stored")),
    }
}

pub fn remove_pool_info<S: Storage>(storage: &mut S, asset_token: &CanonicalAddr) {
    PrefixedStorage::new(PREFIX_POOL_INFO, storage).remove(asset_token.as_slice());
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfo {
    pub index: Decimal,
    pub bond_amount: Uint128,
    pub pending_reward: Uint128,
}

/// returns a bucket with all rewards owned by this owner (query it by owner)
pub fn rewards_store<'a, S: Storage>(
    storage: &'a mut S,
    owner: &CanonicalAddr,
) -> Bucket<'a, S, RewardInfo> {
    Bucket::multilevel(&[PREFIX_REWARD, owner.as_slice()], storage)
}

/// returns a bucket with all rewards owned by this owner (query it by owner)
/// (read-only version for queries)
pub fn rewards_read<'a, S: ReadonlyStorage>(
    storage: &'a S,
    owner: &CanonicalAddr,
) -> ReadonlyBucket<'a, S, RewardInfo> {
    ReadonlyBucket::multilevel(&[PREFIX_REWARD, owner.as_slice()], storage)
}

pub fn read_migration<S: Storage>(
    storage: &S,
    asset_token: &CanonicalAddr,
) -> StdResult<CanonicalAddr> {
    let res = ReadonlyPrefixedStorage::new(PREFIX_MIGRATION, storage).get(asset_token.as_slice());
    match res {
        Some(data) => Ok(CanonicalAddr::from(data.as_slice())),
        None => Err(StdError::generic_err("no migration data stored")),
    }
}

pub fn store_migration<S: Storage>(
    storage: &mut S,
    asset_token: &CanonicalAddr,
    target_token: &CanonicalAddr,
) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_MIGRATION, storage)
        .set(asset_token.as_slice(), target_token.as_slice());
    Ok(())
}
