use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, Decimal, ReadonlyStorage, StdResult, Storage, Uint128};
use cosmwasm_storage::{singleton, singleton_read, Bucket, ReadonlyBucket};

pub static KEY_CONFIG: &[u8] = b"config";
pub static PREFIX_POOL_INFO: &[u8] = b"pool_info";

static PREFIX_REWARD: &[u8] = b"reward";
static PREFIX_SHORT_REWARD: &[u8] = b"short_reward";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub mirror_token: CanonicalAddr,
    pub mint_contract: CanonicalAddr,
    pub oracle_contract: CanonicalAddr,
    pub terraswap_factory: CanonicalAddr,
    pub base_denom: String,
    pub premium_tolerance: Decimal,
    pub short_reward_weight: Decimal,
    pub premium_short_reward_weight: Decimal,
    pub premium_min_update_interval: u64,
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
    pub pending_reward: Uint128, // not distributed amount due to zero bonding
    pub short_pending_reward: Uint128, // not distributed amount due to zero bonding
    pub total_bond_amount: Uint128,
    pub total_short_amount: Uint128,
    pub reward_index: Decimal,
    pub short_reward_index: Decimal,
    pub premium_rate: Decimal,
    pub premium_updated_time: u64,
}

pub fn store_pool_info<S: Storage>(
    storage: &mut S,
    asset_token: &CanonicalAddr,
    pool_info: &PoolInfo,
) -> StdResult<()> {
    Bucket::new(PREFIX_POOL_INFO, storage).save(asset_token.as_slice(), pool_info)
}

pub fn read_pool_info<S: Storage>(storage: &S, asset_token: &CanonicalAddr) -> StdResult<PoolInfo> {
    ReadonlyBucket::new(PREFIX_POOL_INFO, storage).load(asset_token.as_slice())
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
    is_short: bool,
) -> Bucket<'a, S, RewardInfo> {
    if is_short {
        Bucket::multilevel(&[PREFIX_SHORT_REWARD, owner.as_slice()], storage)
    } else {
        Bucket::multilevel(&[PREFIX_REWARD, owner.as_slice()], storage)
    }
}

/// returns a bucket with all rewards owned by this owner (query it by owner)
/// (read-only version for queries)
pub fn rewards_read<'a, S: ReadonlyStorage>(
    storage: &'a S,
    owner: &CanonicalAddr,
    is_short: bool,
) -> ReadonlyBucket<'a, S, RewardInfo> {
    if is_short {
        ReadonlyBucket::multilevel(&[PREFIX_SHORT_REWARD, owner.as_slice()], storage)
    } else {
        ReadonlyBucket::multilevel(&[PREFIX_REWARD, owner.as_slice()], storage)
    }
}
