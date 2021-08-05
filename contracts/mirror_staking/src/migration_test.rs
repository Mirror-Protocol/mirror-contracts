#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::mock_dependencies;
    use cosmwasm_std::Api;
    use cosmwasm_std::{CanonicalAddr, Decimal, StdResult, Storage, Uint128};
    use cosmwasm_storage::{singleton, Bucket};

    use crate::migration::{migrate_config, migrate_pool_infos, LegacyConfig, LegacyPoolInfo};
    use crate::state::{
        read_config, read_pool_info, Config, PoolInfo, KEY_CONFIG, PREFIX_POOL_INFO,
    };

    fn store_legacy_config(storage: &mut dyn Storage, config: &LegacyConfig) -> StdResult<()> {
        singleton(storage, KEY_CONFIG).save(config)
    }

    fn store_legacy_pool_info(
        storage: &mut dyn Storage,
        asset_token: &CanonicalAddr,
        pool_info: &LegacyPoolInfo,
    ) -> StdResult<()> {
        Bucket::new(storage, PREFIX_POOL_INFO).save(asset_token.as_slice(), pool_info)
    }

    #[test]
    fn test_config_migration() {
        let mut deps = mock_dependencies(&[]);
        store_legacy_config(
            &mut deps.storage,
            &LegacyConfig {
                owner: deps.api.addr_canonicalize(&"owner").unwrap(),
                mirror_token: deps.api.addr_canonicalize(&"mirror").unwrap(),
            },
        )
        .unwrap();

        migrate_config(
            &mut deps.storage,
            deps.api.addr_canonicalize(&"mint").unwrap(),
            deps.api.addr_canonicalize(&"oracle").unwrap(),
            deps.api.addr_canonicalize(&"terraswap_factory").unwrap(),
            "uusd".to_string(),
            7200,
        )
        .unwrap();

        assert_eq!(
            Config {
                owner: deps.api.addr_canonicalize(&"owner").unwrap(),
                mirror_token: deps.api.addr_canonicalize(&"mirror").unwrap(),
                mint_contract: deps.api.addr_canonicalize(&"mint").unwrap(),
                oracle_contract: deps.api.addr_canonicalize(&"oracle").unwrap(),
                terraswap_factory: deps.api.addr_canonicalize(&"terraswap_factory").unwrap(),
                base_denom: "uusd".to_string(),
                premium_min_update_interval: 7200,
            },
            read_config(&deps.storage).unwrap()
        );
    }

    #[test]
    fn test_pool_infos_migration() {
        let mut deps = mock_dependencies(&[]);
        store_legacy_pool_info(
            &mut deps.storage,
            &deps.api.addr_canonicalize(&"asset1").unwrap(),
            &LegacyPoolInfo {
                staking_token: deps.api.addr_canonicalize(&"staking1").unwrap(),
                pending_reward: Uint128::zero(),
                total_bond_amount: Uint128::zero(),
                reward_index: Decimal::zero(),
            },
        )
        .unwrap();

        store_legacy_pool_info(
            &mut deps.storage,
            &deps.api.addr_canonicalize(&"asset2").unwrap(),
            &LegacyPoolInfo {
                staking_token: deps.api.addr_canonicalize(&"staking2").unwrap(),
                pending_reward: Uint128::zero(),
                total_bond_amount: Uint128::zero(),
                reward_index: Decimal::zero(),
            },
        )
        .unwrap();

        migrate_pool_infos(&mut deps.storage).unwrap();

        assert_eq!(
            PoolInfo {
                staking_token: deps.api.addr_canonicalize(&"staking1").unwrap(),
                pending_reward: Uint128::zero(),
                total_bond_amount: Uint128::zero(),
                reward_index: Decimal::zero(),
                short_pending_reward: Uint128::zero(),
                total_short_amount: Uint128::zero(),
                short_reward_index: Decimal::zero(),
                premium_rate: Decimal::zero(),
                short_reward_weight: Decimal::zero(),
                premium_updated_time: 0,
            },
            read_pool_info(
                &deps.storage,
                &deps.api.addr_canonicalize(&"asset1").unwrap(),
            )
            .unwrap()
        );

        assert_eq!(
            PoolInfo {
                staking_token: deps.api.addr_canonicalize(&"staking2").unwrap(),
                pending_reward: Uint128::zero(),
                total_bond_amount: Uint128::zero(),
                reward_index: Decimal::zero(),
                short_pending_reward: Uint128::zero(),
                total_short_amount: Uint128::zero(),
                short_reward_index: Decimal::zero(),
                premium_rate: Decimal::zero(),
                short_reward_weight: Decimal::zero(),
                premium_updated_time: 0,
            },
            read_pool_info(
                &deps.storage,
                &deps.api.addr_canonicalize(&"asset2").unwrap(),
            )
            .unwrap()
        );
    }
}
