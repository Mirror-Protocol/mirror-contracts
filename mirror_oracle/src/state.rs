use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    from_slice, to_vec, CanonicalAddr, Decimal, ReadonlyStorage, StdError, StdResult, Storage,
};
use cosmwasm_storage::{singleton, singleton_read, PrefixedStorage, ReadonlyPrefixedStorage};

static PREFIX_ASSET: &[u8] = b"asset";
static PREFIX_PRICE: &[u8] = b"price";

static KEY_CONFIG: &[u8] = b"config";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub base_denom: String,
}

pub fn store_config<S: Storage>(storage: &mut S, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config<S: Storage>(storage: &S) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Asset {
    pub symbol: String,
    pub feeder: CanonicalAddr,
    pub token: CanonicalAddr,
}

pub fn store_asset<S: Storage>(storage: &mut S, symbol: String, asset: &Asset) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_ASSET, storage).set(symbol.as_bytes(), &to_vec(&asset)?);
    Ok(())
}

pub fn read_asset<S: Storage>(storage: &S, symbol: String) -> StdResult<Asset> {
    let res = ReadonlyPrefixedStorage::new(PREFIX_ASSET, storage).get(symbol.as_bytes());
    match res {
        Some(data) => from_slice(&data),
        None => Err(StdError::generic_err("no asset data stored")),
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Price {
    pub price: Decimal,
    pub price_multiplier: Decimal,
    pub last_update_time: u64,
}

pub fn store_price<S: Storage>(storage: &mut S, symbol: String, price: &Price) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_PRICE, storage).set(symbol.as_bytes(), &to_vec(&price)?);
    Ok(())
}

pub fn read_price<S: Storage>(storage: &S, symbol: String) -> StdResult<Price> {
    let res = ReadonlyPrefixedStorage::new(PREFIX_PRICE, storage).get(symbol.as_bytes());
    match res {
        Some(data) => from_slice(&data),
        None => Err(StdError::generic_err("no asset data stored")),
    }
}
