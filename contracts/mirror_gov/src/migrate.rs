use cosmwasm_std::{CanonicalAddr, Decimal, DepsMut, StdResult, Uint128};
use cosmwasm_storage::{singleton, singleton_read, ReadonlySingleton, Singleton};
use mirror_protocol::gov::PollConfig;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{Config, KEY_CONFIG};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyConfig {
    pub owner: CanonicalAddr,
    pub mirror_token: CanonicalAddr,
    pub quorum: Decimal,
    pub threshold: Decimal,
    pub voting_period: u64,
    pub effective_delay: u64,
    pub expiration_period: u64, // deprecated, to remove on next state migration
    pub proposal_deposit: Uint128,
    pub voter_weight: Decimal,
    pub snapshot_period: u64,
}

pub fn migrate_config(
    deps: DepsMut,
    migration_poll_config: PollConfig,
    auth_admin_poll_config: PollConfig,
    admin_manager: String,
    poll_gas_limit: u64,
) -> StdResult<()> {
    let legacty_store: ReadonlySingleton<LegacyConfig> = singleton_read(deps.storage, KEY_CONFIG);
    let legacy_config: LegacyConfig = legacty_store.load()?;
    let config = Config {
        mirror_token: legacy_config.mirror_token,
        owner: legacy_config.owner,
        effective_delay: legacy_config.effective_delay,
        voter_weight: legacy_config.voter_weight,
        snapshot_period: legacy_config.snapshot_period,
        default_poll_config: PollConfig {
            proposal_deposit: legacy_config.proposal_deposit,
            voting_period: legacy_config.voting_period,
            quorum: legacy_config.quorum,
            threshold: legacy_config.threshold,
        },
        migration_poll_config,
        auth_admin_poll_config,
        admin_manager: deps.api.addr_canonicalize(&admin_manager)?,
        poll_gas_limit,
    };
    let mut store: Singleton<Config> = singleton(deps.storage, KEY_CONFIG);
    store.save(&config)?;
    Ok(())
}

#[cfg(test)]
mod migrate_tests {
    use crate::state::config_read;

    use super::*;
    use cosmwasm_std::{testing::mock_dependencies, Api, Storage};

    pub fn config_old_store(storage: &mut dyn Storage) -> Singleton<LegacyConfig> {
        Singleton::new(storage, KEY_CONFIG)
    }

    #[test]
    fn test_config_migration() {
        let mut deps = mock_dependencies(&[]);
        let mut legacy_config_store = config_old_store(&mut deps.storage);
        legacy_config_store
            .save(&LegacyConfig {
                mirror_token: deps.api.addr_canonicalize("mir0000").unwrap(),
                owner: deps.api.addr_canonicalize("owner0000").unwrap(),
                quorum: Decimal::one(),
                threshold: Decimal::one(),
                voting_period: 100u64,
                effective_delay: 100u64,
                expiration_period: 100u64,
                proposal_deposit: Uint128::from(100000u128),
                voter_weight: Decimal::percent(50),
                snapshot_period: 20u64,
            })
            .unwrap();

        let migration_poll_config = PollConfig {
            quorum: Decimal::percent(60),
            threshold: Decimal::percent(60),
            proposal_deposit: Uint128::from(99999u128),
            voting_period: 888u64,
        };
        let auth_admin_poll_config = PollConfig {
            quorum: Decimal::percent(70),
            threshold: Decimal::percent(70),
            proposal_deposit: Uint128::from(99999000u128),
            voting_period: 88800u64,
        };
        migrate_config(
            deps.as_mut(),
            migration_poll_config.clone(),
            auth_admin_poll_config.clone(),
            "admin_manager".to_string(),
            4_000_000u64,
        )
        .unwrap();

        let config: Config = config_read(&deps.storage).load().unwrap();
        assert_eq!(
            config,
            Config {
                mirror_token: deps.api.addr_canonicalize("mir0000").unwrap(),
                owner: deps.api.addr_canonicalize("owner0000").unwrap(),
                default_poll_config: PollConfig {
                    quorum: Decimal::one(),
                    threshold: Decimal::one(),
                    voting_period: 100u64,
                    proposal_deposit: Uint128::from(100000u128),
                },
                migration_poll_config,
                auth_admin_poll_config,
                effective_delay: 100u64,
                voter_weight: Decimal::percent(50u64),
                snapshot_period: 20u64,
                admin_manager: deps.api.addr_canonicalize("admin_manager").unwrap(),
                poll_gas_limit: 4_000_000u64,
            }
        )
    }
}
