use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, Decimal, Storage};
use cosmwasm_storage::{singleton, singleton_read, ReadonlySingleton, Singleton};

static CONFIG_KEY: &[u8] = b"config";
static PRICE_KEY: &[u8] = b"price";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigState {
    pub owner: CanonicalAddr,
    pub asset_token: CanonicalAddr,
    pub base_denom: String,
    pub quote_denom: String,
}

pub fn config_store<S: Storage>(storage: &mut S) -> Singleton<S, ConfigState> {
    singleton(storage, CONFIG_KEY)
}

pub fn config_read<S: Storage>(storage: &S) -> ReadonlySingleton<S, ConfigState> {
    singleton_read(storage, CONFIG_KEY)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PriceState {
    pub price: Decimal,
    pub price_multiplier: Decimal,
    pub last_update_time: u64,
}

pub fn price_store<S: Storage>(storage: &mut S) -> Singleton<S, PriceState> {
    singleton(storage, PRICE_KEY)
}

pub fn price_read<S: Storage>(storage: &S) -> ReadonlySingleton<S, PriceState> {
    singleton_read(storage, PRICE_KEY)
}
