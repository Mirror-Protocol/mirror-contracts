use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    from_slice, to_vec, CanonicalAddr, Decimal, ReadonlyStorage, StdError, StdResult, Storage,
    Uint128,
};
use cosmwasm_storage::{
    singleton, singleton_read, PrefixedStorage, ReadonlyPrefixedStorage, Singleton,
};

static KEY_CONFIG: &[u8] = b"config";
static KEY_PARAMS: &[u8] = b"params";

static PREFIX_DISTRIBUTION: &[u8] = b"distribution";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub mirror_token: CanonicalAddr,
    pub mint_contract: CanonicalAddr,
    pub oracle_contract: CanonicalAddr,
    pub terraswap_factory: CanonicalAddr,
    pub staking_contract: CanonicalAddr,
    pub commission_collector: CanonicalAddr,
    pub mint_per_block: Uint128,
    pub token_code_id: u64, // used to create asset token
    pub base_denom: String,
}

pub fn store_config<S: Storage>(storage: &mut S, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config<S: Storage>(storage: &S) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Params {
    /// inflation weight
    pub weight: Decimal,
    /// Commission rate for active liquidity provider
    pub lp_commission: Decimal,
    /// Commission rate for owner controlled commission
    pub owner_commission: Decimal,
    /// Auction discount rate applied to asset mint
    pub auction_discount: Decimal,
    /// Minium collateral ratio applied to asset mint
    pub min_collateral_ratio: Decimal,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DistributionInfo {
    pub weight: Decimal,
    pub last_height: u64,
}

pub fn store_distribution_info<S: Storage>(
    storage: &mut S,
    asset_token: &CanonicalAddr,
    data: &DistributionInfo,
) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_DISTRIBUTION, storage).set(asset_token.as_slice(), &to_vec(data)?);
    Ok(())
}

pub fn read_distribution_info<S: Storage>(
    storage: &S,
    asset_token: &CanonicalAddr,
) -> StdResult<DistributionInfo> {
    let data =
        ReadonlyPrefixedStorage::new(PREFIX_DISTRIBUTION, storage).get(asset_token.as_slice());
    match data {
        Some(v) => from_slice(&v),
        None => Err(StdError::generic_err("No distribution info stored")),
    }
}
