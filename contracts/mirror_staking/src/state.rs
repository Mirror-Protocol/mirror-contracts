use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    from_slice, to_vec, CanonicalAddr, Decimal, ReadonlyStorage, StdResult, Storage, Uint128,
};
use cosmwasm_storage::{singleton, singleton_read, PrefixedStorage, ReadonlyPrefixedStorage};

static KEY_CONFIG: &[u8] = b"config";
static KEY_POOL: &[u8] = b"pool";
static KEY_PREFIX_REWARD: &[u8] = b"reward";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub staking_token: CanonicalAddr,
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
    pub total_bond_amount: Uint128,
    pub reward_index: Decimal,
}

pub fn store_pool_info<S: Storage>(storage: &mut S, pool_info: &PoolInfo) -> StdResult<()> {
    singleton(storage, KEY_POOL).save(pool_info)
}

pub fn read_pool_info<S: Storage>(storage: &S) -> StdResult<PoolInfo> {
    singleton_read(storage, KEY_POOL).load()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfo {
    pub index: Decimal,
    pub bond_amount: Uint128,
    pub pending_reward: Uint128,
}

pub fn store_reward_info<S: Storage>(
    storage: &mut S,
    addr: &CanonicalAddr,
    reward_info: &RewardInfo,
) -> StdResult<()> {
    PrefixedStorage::new(KEY_PREFIX_REWARD, storage).set(&addr.as_slice(), &to_vec(&reward_info)?);
    Ok(())
}

pub fn read_reward_info<S: Storage>(storage: &S, address: &CanonicalAddr) -> StdResult<RewardInfo> {
    let reward_storage = ReadonlyPrefixedStorage::new(KEY_PREFIX_REWARD, storage);
    let result = reward_storage.get(address.as_slice());

    match result {
        Some(data) => from_slice(&data),
        None => Ok(RewardInfo {
            index: Decimal::zero(),
            bond_amount: Uint128::zero(),
            pending_reward: Uint128::zero(),
        }),
    }
}
