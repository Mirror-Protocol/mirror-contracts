use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    from_slice, to_vec, CanonicalAddr, Decimal, ReadonlyStorage, StdError, StdResult, Storage,
};

use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};

static PREFIX_CONFIG: &[u8] = b"config";

static KEY_ASSET: &[u8] = b"asset";
static KEY_GENERAL: &[u8] = b"general";
static KEY_SWAP: &[u8] = b"swap";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigGeneral {
    pub owner: CanonicalAddr,
    pub contract_addr: CanonicalAddr,
    pub liquidity_token: CanonicalAddr,
    pub commission_collector: CanonicalAddr,
    pub collateral_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigSwap {
    pub active_commission: Decimal,
    pub inactive_commission: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigAsset {
    pub token: CanonicalAddr,
    pub symbol: String,
}

pub fn store_config_general<S: Storage>(storage: &mut S, data: &ConfigGeneral) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_CONFIG, storage).set(KEY_GENERAL, &to_vec(data)?);
    Ok(())
}

pub fn read_config_general<S: Storage>(storage: &S) -> StdResult<ConfigGeneral> {
    let data = ReadonlyPrefixedStorage::new(PREFIX_CONFIG, storage).get(KEY_GENERAL);
    match data {
        Some(v) => from_slice(&v),
        None => Err(StdError::generic_err("no general config data stored")),
    }
}

pub fn store_config_swap<S: Storage>(storage: &mut S, data: &ConfigSwap) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_CONFIG, storage).set(KEY_SWAP, &to_vec(data)?);
    Ok(())
}

pub fn read_config_swap<S: Storage>(storage: &S) -> StdResult<ConfigSwap> {
    let data = ReadonlyPrefixedStorage::new(PREFIX_CONFIG, storage).get(KEY_SWAP);
    match data {
        Some(v) => from_slice(&v),
        None => Err(StdError::generic_err("no general swap data stored")),
    }
}

pub fn store_config_asset<S: Storage>(storage: &mut S, data: &ConfigAsset) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_CONFIG, storage).set(KEY_ASSET, &to_vec(data)?);
    Ok(())
}

pub fn read_config_asset<S: Storage>(storage: &S) -> StdResult<ConfigAsset> {
    let data = ReadonlyPrefixedStorage::new(PREFIX_CONFIG, storage).get(KEY_ASSET);
    match data {
        Some(v) => from_slice(&v),
        None => Err(StdError::generic_err("no asset config data stored")),
    }
}
