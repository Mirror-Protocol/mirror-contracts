#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::mock_dependencies;
    use cosmwasm_std::Api;
    use cosmwasm_std::{CanonicalAddr, Decimal, HumanAddr, StdResult, Storage, Uint128};
    use cosmwasm_storage::{singleton, Bucket};

    use crate::migration::{migrate_config, migrate_pool_infos, LegacyConfig, LegacyPoolInfo};
    use crate::state::{
        read_config, read_pool_info, Config, PoolInfo, KEY_CONFIG, PREFIX_POOL_INFO,
    };

    fn store_legacy_config<S: Storage>(storage: &mut S, config: &LegacyConfig) -> StdResult<()> {
        singleton(storage, KEY_CONFIG).save(config)
    }

    fn store_legacy_pool_info<S: Storage>(
        storage: &mut S,
        asset_token: &CanonicalAddr,
        pool_info: &LegacyPoolInfo,
    ) -> StdResult<()> {
        Bucket::new(PREFIX_POOL_INFO, storage).save(asset_token.as_slice(), pool_info)
    }

    #[test]
    fn test_config_migration() {
        let mut deps = mock_dependencies(20, &[]);
        store_legacy_config(
            &mut deps.storage,
            &LegacyConfig {
                owner: deps
                    .api
                    .canonical_address(&HumanAddr::from("owner"))
                    .unwrap(),
                mirror_token: deps
                    .api
                    .canonical_address(&HumanAddr::from("mirror"))
                    .unwrap(),
            },
        )
        .unwrap();

        migrate_config(
            &mut deps.storage,
            deps.api
                .canonical_address(&HumanAddr::from("mint"))
                .unwrap(),
            deps.api
                .canonical_address(&HumanAddr::from("oracle"))
                .unwrap(),
            deps.api
                .canonical_address(&HumanAddr::from("terraswap_factory"))
                .unwrap(),
            "uusd".to_string(),
            7200,
        )
        .unwrap();

        assert_eq!(
            Config {
                owner: deps
                    .api
                    .canonical_address(&HumanAddr::from("owner"))
                    .unwrap(),
                mirror_token: deps
                    .api
                    .canonical_address(&HumanAddr::from("mirror"))
                    .unwrap(),
                mint_contract: deps
                    .api
                    .canonical_address(&HumanAddr::from("mint"))
                    .unwrap(),
                oracle_contract: deps
                    .api
                    .canonical_address(&HumanAddr::from("oracle"))
                    .unwrap(),
                terraswap_factory: deps
                    .api
                    .canonical_address(&HumanAddr::from("terraswap_factory"))
                    .unwrap(),
                base_denom: "uusd".to_string(),
                premium_min_update_interval: 7200,
            },
            read_config(&deps.storage).unwrap()
        );
    }

    #[test]
    fn test_pool_infos_migration() {
        let mut deps = mock_dependencies(20, &[]);
        store_legacy_pool_info(
            &mut deps.storage,
            &deps
                .api
                .canonical_address(&HumanAddr::from("asset1"))
                .unwrap(),
            &LegacyPoolInfo {
                staking_token: deps
                    .api
                    .canonical_address(&HumanAddr::from("staking1"))
                    .unwrap(),
                pending_reward: Uint128::zero(),
                total_bond_amount: Uint128::zero(),
                reward_index: Decimal::zero(),
            },
        )
        .unwrap();

        store_legacy_pool_info(
            &mut deps.storage,
            &deps
                .api
                .canonical_address(&HumanAddr::from("asset2"))
                .unwrap(),
            &LegacyPoolInfo {
                staking_token: deps
                    .api
                    .canonical_address(&HumanAddr::from("staking2"))
                    .unwrap(),
                pending_reward: Uint128::zero(),
                total_bond_amount: Uint128::zero(),
                reward_index: Decimal::zero(),
            },
        )
        .unwrap();

        migrate_pool_infos(&mut deps.storage).unwrap();

        assert_eq!(
            PoolInfo {
                staking_token: deps
                    .api
                    .canonical_address(&HumanAddr::from("staking1"))
                    .unwrap(),
                pending_reward: Uint128::zero(),
                total_bond_amount: Uint128::zero(),
                reward_index: Decimal::zero(),
                short_pending_reward: Uint128::zero(),
                total_short_amount: Uint128::zero(),
                short_reward_index: Decimal::zero(),
                premium_rate: Decimal::zero(),
                premium_updated_time: 0,
            },
            read_pool_info(
                &deps.storage,
                &deps
                    .api
                    .canonical_address(&HumanAddr::from("asset1"))
                    .unwrap(),
            )
            .unwrap()
        );

        assert_eq!(
            PoolInfo {
                staking_token: deps
                    .api
                    .canonical_address(&HumanAddr::from("staking2"))
                    .unwrap(),
                pending_reward: Uint128::zero(),
                total_bond_amount: Uint128::zero(),
                reward_index: Decimal::zero(),
                short_pending_reward: Uint128::zero(),
                total_short_amount: Uint128::zero(),
                short_reward_index: Decimal::zero(),
                premium_rate: Decimal::zero(),
                premium_updated_time: 0,
            },
            read_pool_info(
                &deps.storage,
                &deps
                    .api
                    .canonical_address(&HumanAddr::from("asset2"))
                    .unwrap(),
            )
            .unwrap()
        );
    }
}
