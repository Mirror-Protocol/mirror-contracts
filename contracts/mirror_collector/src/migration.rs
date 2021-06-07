use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, StdResult, Storage};
use cosmwasm_storage::singleton_read;

use crate::state::{store_config, Config};

static KEY_CONFIG: &[u8] = b"config";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyConfig {
    pub distribution_contract: CanonicalAddr,
    pub terraswap_factory: CanonicalAddr,
    pub mirror_token: CanonicalAddr,
    pub base_denom: String,
}

fn read_legacy_config<S: Storage>(storage: &S) -> StdResult<LegacyConfig> {
    singleton_read(storage, KEY_CONFIG).load()
}

pub fn migrate_config<S: Storage>(
    storage: &mut S,
    owner: CanonicalAddr,
    aust_token: CanonicalAddr,
    anchor_market: CanonicalAddr,
    bluna_token: CanonicalAddr,
    bluna_swap_denom: String,
) -> StdResult<()> {
    let legacy_config: LegacyConfig = read_legacy_config(storage)?;
    store_config(
        storage,
        &Config {
            owner,
            distribution_contract: legacy_config.distribution_contract,
            terraswap_factory: legacy_config.terraswap_factory,
            mirror_token: legacy_config.mirror_token,
            base_denom: legacy_config.base_denom,
            aust_token,
            anchor_market,
            bluna_swap_denom,
            bluna_token,
        },
    )?;

    Ok(())
}

#[cfg(test)]
mod migrate_tests {
    use super::*;
    use crate::state::read_config;
    use cosmwasm_std::testing::mock_dependencies;
    use cosmwasm_std::{Api, HumanAddr};
    use cosmwasm_storage::singleton;

    pub fn store_legacy_config<S: Storage>(
        storage: &mut S,
        config: &LegacyConfig,
    ) -> StdResult<()> {
        singleton(storage, KEY_CONFIG).save(config)
    }

    #[test]
    fn test_config_migration() {
        let mut deps = mock_dependencies(20, &[]);

        let owner = deps
            .api
            .canonical_address(&HumanAddr::from("owner0000"))
            .unwrap();
        let distribution_contract = deps
            .api
            .canonical_address(&HumanAddr::from("distribution0000"))
            .unwrap();
        let terraswap_factory = deps
            .api
            .canonical_address(&HumanAddr::from("terraswapfactory0000"))
            .unwrap();
        let mirror_token = deps
            .api
            .canonical_address(&HumanAddr::from("mir0000"))
            .unwrap();
        let aust_token = deps
            .api
            .canonical_address(&HumanAddr::from("aust0000"))
            .unwrap();
        let anchor_market = deps
            .api
            .canonical_address(&HumanAddr::from("anchormarket0000"))
            .unwrap();
        let bluna_token = deps
            .api
            .canonical_address(&HumanAddr::from("bluna0000"))
            .unwrap();

        store_legacy_config(
            &mut deps.storage,
            &LegacyConfig {
                distribution_contract: distribution_contract.clone(),
                terraswap_factory: terraswap_factory.clone(),
                mirror_token: mirror_token.clone(),
                base_denom: "uusd".to_string(),
            },
        )
        .unwrap();

        migrate_config(
            &mut deps.storage,
            owner.clone(),
            aust_token.clone(),
            anchor_market.clone(),
            bluna_token.clone(),
            "uluna".to_string(),
        )
        .unwrap();
        assert_eq!(
            read_config(&deps.storage).unwrap(),
            Config {
                owner,
                distribution_contract,
                terraswap_factory,
                mirror_token,
                base_denom: "uusd".to_string(),
                aust_token,
                anchor_market,
                bluna_token,
                bluna_swap_denom: "uluna".to_string(),
            }
        );
    }
}
