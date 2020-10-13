use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    from_slice, to_vec, CanonicalAddr, Decimal, Order, ReadonlyStorage, StdError, StdResult,
    Storage,
};
use cosmwasm_storage::{singleton, singleton_read, PrefixedStorage, ReadonlyPrefixedStorage};
use terraswap::AssetInfoRaw;

static PREFIX_ASSET: &[u8] = b"asset";
static PREFIX_PRICE: &[u8] = b"price";

static KEY_CONFIG: &[u8] = b"config";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub base_asset_info: AssetInfoRaw,
}

pub fn store_config<S: Storage>(storage: &mut S, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config<S: Storage>(storage: &S) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AssetConfig {
    pub asset_token: CanonicalAddr,
    pub feeder: CanonicalAddr,
}

pub fn store_asset_config<S: Storage>(
    storage: &mut S,
    asset_token: &CanonicalAddr,
    asset: &AssetConfig,
) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_ASSET, storage).set(asset_token.as_slice(), &to_vec(&asset)?);
    Ok(())
}

pub fn read_asset_config<S: Storage>(
    storage: &S,
    asset_token: &CanonicalAddr,
) -> StdResult<AssetConfig> {
    let res = ReadonlyPrefixedStorage::new(PREFIX_ASSET, storage).get(asset_token.as_slice());
    match res {
        Some(data) => from_slice(&data),
        None => Err(StdError::generic_err("no asset data stored")),
    }
}

pub fn remove_asset_config<S: Storage>(
    storage: &mut S,
    asset_token: &CanonicalAddr,
) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_ASSET, storage).remove(asset_token.as_slice());
    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PriceInfoRaw {
    pub price: Decimal,
    pub last_update_time: u64,
    pub asset_token: CanonicalAddr,
}

pub fn store_price<S: Storage>(
    storage: &mut S,
    asset_token: &CanonicalAddr,
    price: &PriceInfoRaw,
) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_PRICE, storage).set(asset_token.as_slice(), &to_vec(&price)?);
    Ok(())
}

pub fn read_price<S: Storage>(storage: &S, asset_token: &CanonicalAddr) -> StdResult<PriceInfoRaw> {
    let res = ReadonlyPrefixedStorage::new(PREFIX_PRICE, storage).get(asset_token.as_slice());
    match res {
        Some(data) => from_slice(&data),
        None => Err(StdError::generic_err("no asset data stored")),
    }
}

pub fn remove_price<S: Storage>(storage: &mut S, asset_token: &CanonicalAddr) {
    PrefixedStorage::new(PREFIX_PRICE, storage).remove(asset_token.as_slice());
}

pub fn read_prices<S: Storage>(storage: &S) -> StdResult<Vec<PriceInfoRaw>> {
    ReadonlyPrefixedStorage::new(PREFIX_PRICE, storage)
        .range(None, None, Order::Ascending)
        .map(|item| {
            let (_, v) = item;

            from_slice(&v)
        })
        .collect()
}
