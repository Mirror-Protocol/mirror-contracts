use cosmwasm_std::{CanonicalAddr, StdResult, Storage};
use cosmwasm_storage::{singleton, singleton_read, ReadonlySingleton, Singleton};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{Config, KEY_CONFIG};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyConfig {
    pub owner: CanonicalAddr,
    pub distribution_contract: CanonicalAddr,
    pub terraswap_factory: CanonicalAddr,
    pub mirror_token: CanonicalAddr,
    pub base_denom: String,
    pub aust_token: CanonicalAddr,
    pub anchor_market: CanonicalAddr,
    pub bluna_token: CanonicalAddr,
    pub bluna_swap_denom: String,
}

pub fn migrate_config(storage: &mut dyn Storage) -> StdResult<()> {
    let legacy_store: ReadonlySingleton<LegacyConfig> = singleton_read(storage, KEY_CONFIG);
    let legacy_config: LegacyConfig = legacy_store.load()?;
    let config = Config {
        owner: legacy_config.owner,
        distribution_contract: legacy_config.distribution_contract,
        terraswap_factory: legacy_config.terraswap_factory,
        mirror_token: legacy_config.mirror_token,
        base_denom: legacy_config.base_denom,
        aust_token: legacy_config.aust_token,
        anchor_market: legacy_config.anchor_market,
        bluna_token: legacy_config.bluna_token,
        bluna_swap_denom: legacy_config.bluna_swap_denom,
        mir_ust_pair: None,
    };
    let mut store: Singleton<Config> = singleton(storage, KEY_CONFIG);
    store.save(&config)?;
    Ok(())
}

#[cfg(test)]
mod migrate_tests {
    use crate::state::read_config;

    use super::*;
    use cosmwasm_std::{testing::mock_dependencies, Api};

    pub fn config_old_store(storage: &mut dyn Storage) -> Singleton<LegacyConfig> {
        Singleton::new(storage, KEY_CONFIG)
    }

    #[test]
    fn test_config_migration() {
        let mut deps = mock_dependencies(&[]);
        let mut legacy_config_store = config_old_store(&mut deps.storage);
        legacy_config_store
            .save(&LegacyConfig {
                owner: deps.api.addr_canonicalize("owner0000").unwrap(),
                terraswap_factory: deps.api.addr_canonicalize("terraswapfactory").unwrap(),
                distribution_contract: deps.api.addr_canonicalize("gov0000").unwrap(),
                mirror_token: deps.api.addr_canonicalize("mirror0000").unwrap(),
                base_denom: "uusd".to_string(),
                aust_token: deps.api.addr_canonicalize("aust0000").unwrap(),
                anchor_market: deps.api.addr_canonicalize("anchormarket0000").unwrap(),
                bluna_token: deps.api.addr_canonicalize("bluna0000").unwrap(),
                bluna_swap_denom: "uluna".to_string(),
            })
            .unwrap();

        migrate_config(&mut deps.storage).unwrap();

        let config: Config = read_config(&deps.storage).unwrap();
        assert_eq!(
            config,
            Config {
                owner: deps.api.addr_canonicalize("owner0000").unwrap(),
                terraswap_factory: deps.api.addr_canonicalize("terraswapfactory").unwrap(),
                distribution_contract: deps.api.addr_canonicalize("gov0000").unwrap(),
                mirror_token: deps.api.addr_canonicalize("mirror0000").unwrap(),
                base_denom: "uusd".to_string(),
                aust_token: deps.api.addr_canonicalize("aust0000").unwrap(),
                anchor_market: deps.api.addr_canonicalize("anchormarket0000").unwrap(),
                bluna_token: deps.api.addr_canonicalize("bluna0000").unwrap(),
                bluna_swap_denom: "uluna".to_string(),
                mir_ust_pair: None,
            }
        )
    }
}
