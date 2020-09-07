use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    from_slice, to_vec, CanonicalAddr, Decimal, ReadonlyStorage, StdError, StdResult, Storage,
    Uint128,
};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};

static PREFIX_CONFIG: &[u8] = b"config";
static PREFIX_POSITION: &[u8] = b"position";

static KEY_GENERAL: &[u8] = b"general";
static KEY_ASSET: &[u8] = b"asset";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigGeneral {
    pub owner: CanonicalAddr,
    pub collateral_denom: String,
    pub auction_discount: Decimal,
    pub auction_threshold_rate: Decimal,
    pub mint_capacity: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigAsset {
    pub oracle: CanonicalAddr,
    pub token: CanonicalAddr,
    pub symbol: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionState {
    pub collateral_amount: Uint128,
    pub asset_amount: Uint128,
}

pub fn store_config_general<S: Storage>(
    storage: &mut S,
    config_general: &ConfigGeneral,
) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_CONFIG, storage).set(KEY_GENERAL, &to_vec(config_general)?);
    Ok(())
}

pub fn read_config_general<S: Storage>(storage: &S) -> StdResult<ConfigGeneral> {
    match ReadonlyPrefixedStorage::new(PREFIX_CONFIG, storage).get(KEY_GENERAL) {
        Some(config) => from_slice(&config),
        None => Err(StdError::generic_err("failed to fetch genral config data")),
    }
}

pub fn store_config_asset<S: Storage>(
    storage: &mut S,
    config_asset: &ConfigAsset,
) -> StdResult<()> {
    PrefixedStorage::new(PREFIX_CONFIG, storage).set(KEY_ASSET, &to_vec(config_asset)?);
    Ok(())
}

pub fn read_config_asset<S: Storage>(storage: &S) -> StdResult<ConfigAsset> {
    match ReadonlyPrefixedStorage::new(PREFIX_CONFIG, storage).get(KEY_ASSET) {
        Some(config) => from_slice(&config),
        None => Err(StdError::generic_err("failed to fetch asset config data")),
    }
}

pub fn store_position<S: Storage>(storage: &mut S) -> PrefixedStorage<S> {
    PrefixedStorage::new(PREFIX_POSITION, storage)
}

pub fn read_position<S: Storage>(storage: &S, address: &CanonicalAddr) -> StdResult<PositionState> {
    let position_storage = ReadonlyPrefixedStorage::new(PREFIX_POSITION, storage);
    let result = position_storage.get(address.as_slice());

    match result {
        Some(data) => from_slice(&data),
        None => Ok(PositionState {
            collateral_amount: Uint128::from(0u128),
            asset_amount: Uint128::from(0u128),
        }),
    }
}
