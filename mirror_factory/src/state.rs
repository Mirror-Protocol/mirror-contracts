use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    from_slice, to_vec, CanonicalAddr, Decimal, ReadonlyStorage, StdError, StdResult, Storage,
    Uint128,
};
use cosmwasm_storage::{singleton, singleton_read, PrefixedStorage, ReadonlyPrefixedStorage};

static KEY_CONFIG: &[u8] = b"config";
static PREFIX_WHITELIST: &[u8] = b"whitelist";
static PREFIX_DISTRIBUTION: &[u8] = b"distribution";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub mirror_token: CanonicalAddr,
    pub mint_per_block: Uint128,
}

pub fn store_config<S: Storage>(storage: &mut S, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config<S: Storage>(storage: &S) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct WhitelistInfo {
    pub token_contract: CanonicalAddr,
    pub mint_contract: CanonicalAddr,
    pub market_contract: CanonicalAddr,
    pub oracle_contract: CanonicalAddr,
    pub staking_contract: CanonicalAddr,
}

pub fn store_whitelist_info<S: Storage>(
    storage: &mut S,
    symbol: String,
    data: &WhitelistInfo,
) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_WHITELIST, storage).set(symbol.as_bytes(), &to_vec(data)?);
    Ok(())
}

pub fn read_whitelist_info<S: Storage>(storage: &S, symbol: String) -> StdResult<WhitelistInfo> {
    let data = ReadonlyPrefixedStorage::new(PREFIX_WHITELIST, storage).get(symbol.as_bytes());
    match data {
        Some(v) => from_slice(&v),
        None => Err(StdError::generic_err("No whitelist info stored")),
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DistributionInfo {
    pub weight: Decimal,
    pub last_height: u64,
}

pub fn store_distribution_info<S: Storage>(
    storage: &mut S,
    symbol: String,
    data: &DistributionInfo,
) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_DISTRIBUTION, storage).set(symbol.as_bytes(), &to_vec(data)?);
    Ok(())
}

pub fn read_distribution_info<S: Storage>(
    storage: &S,
    symbol: String,
) -> StdResult<DistributionInfo> {
    let data = ReadonlyPrefixedStorage::new(PREFIX_DISTRIBUTION, storage).get(symbol.as_bytes());
    match data {
        Some(v) => from_slice(&v),
        None => Err(StdError::generic_err("No distribution info stored")),
    }
}
