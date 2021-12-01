use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, Decimal, StdResult, Storage, Uint128};
use cosmwasm_storage::{singleton, singleton_read, Bucket, ReadonlyBucket};

pub static KEY_CONFIG: &[u8] = b"config";
pub static PREFIX_POOL_INFO: &[u8] = b"pool_info";

static PREFIX_REWARD: &[u8] = b"reward";
static PREFIX_SHORT_REWARD: &[u8] = b"short_reward";

static PREFIX_IS_MIGRATED: &[u8] = b"is_migrated";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub mirror_token: CanonicalAddr,
    pub mint_contract: CanonicalAddr,
    pub oracle_contract: CanonicalAddr,
    pub terraswap_factory: CanonicalAddr,
    pub base_denom: String,
    pub premium_min_update_interval: u64,
    pub short_reward_contract: CanonicalAddr,
}

pub fn store_config(storage: &mut dyn Storage, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config(storage: &dyn Storage) -> StdResult<Config> {
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
    pub short_reward_weight: Decimal,
    pub premium_updated_time: u64,
    pub migration_params: Option<MigrationParams>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrationParams {
    pub index_snapshot: Decimal,
    pub deprecated_staking_token: CanonicalAddr,
}

pub fn store_pool_info(
    storage: &mut dyn Storage,
    asset_token: &CanonicalAddr,
    pool_info: &PoolInfo,
) -> StdResult<()> {
    Bucket::new(storage, PREFIX_POOL_INFO).save(asset_token.as_slice(), pool_info)
}

pub fn read_pool_info(storage: &dyn Storage, asset_token: &CanonicalAddr) -> StdResult<PoolInfo> {
    ReadonlyBucket::new(storage, PREFIX_POOL_INFO).load(asset_token.as_slice())
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfo {
    pub index: Decimal,
    pub bond_amount: Uint128,
    pub pending_reward: Uint128,
}

/// returns a bucket with all rewards owned by this owner (query it by owner)
pub fn rewards_store<'a>(
    storage: &'a mut dyn Storage,
    owner: &CanonicalAddr,
    is_short: bool,
) -> Bucket<'a, RewardInfo> {
    if is_short {
        Bucket::multilevel(storage, &[PREFIX_SHORT_REWARD, owner.as_slice()])
    } else {
        Bucket::multilevel(storage, &[PREFIX_REWARD, owner.as_slice()])
    }
}

/// returns a bucket with all rewards owned by this owner (query it by owner)
/// (read-only version for queries)
pub fn rewards_read<'a>(
    storage: &'a dyn Storage,
    owner: &CanonicalAddr,
    is_short: bool,
) -> ReadonlyBucket<'a, RewardInfo> {
    if is_short {
        ReadonlyBucket::multilevel(storage, &[PREFIX_SHORT_REWARD, owner.as_slice()])
    } else {
        ReadonlyBucket::multilevel(storage, &[PREFIX_REWARD, owner.as_slice()])
    }
}

pub fn store_is_migrated(
    storage: &mut dyn Storage,
    asset_token: &CanonicalAddr,
    staker: &CanonicalAddr,
) -> StdResult<()> {
    Bucket::multilevel(storage, &[PREFIX_IS_MIGRATED, staker.as_slice()])
        .save(asset_token.as_slice(), &true)
}

pub fn read_is_migrated(
    storage: &dyn Storage,
    asset_token: &CanonicalAddr,
    staker: &CanonicalAddr,
) -> bool {
    ReadonlyBucket::multilevel(storage, &[PREFIX_IS_MIGRATED, staker.as_slice()])
        .load(asset_token.as_slice())
        .unwrap_or(false)
}
