use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, StdResult, Storage};
use cosmwasm_storage::{singleton, singleton_read};

static KEY_CONFIG: &[u8] = b"config";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub distribution_contract: CanonicalAddr, // collected rewards receiver
    pub terraswap_factory: CanonicalAddr,     // terraswap factory contract
    pub mirror_token: CanonicalAddr,
    pub base_denom: String,
    // aUST params
    pub aust_token: CanonicalAddr,
    pub anchor_market: CanonicalAddr,
    // bLuna params
    pub bluna_token: CanonicalAddr,
    pub bluna_swap_denom: String,
}

pub fn store_config(storage: &mut dyn Storage, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config(storage: &dyn Storage) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}
