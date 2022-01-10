use cosmwasm_std::{CanonicalAddr, Decimal, Order, StdResult, Storage, Uint128};
use cosmwasm_storage::Bucket;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{PoolInfo, PREFIX_POOL_INFO};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyPoolInfo {
    pub staking_token: CanonicalAddr,
    pub pending_reward: Uint128,
    pub short_pending_reward: Uint128,
    pub total_bond_amount: Uint128,
    pub total_short_amount: Uint128,
    pub reward_index: Decimal,
    pub short_reward_index: Decimal,
    pub premium_rate: Decimal,
    pub short_reward_weight: Decimal,
    pub premium_updated_time: u64,
}

pub fn migrate_pool_infos(storage: &mut dyn Storage) -> StdResult<()> {
    let mut legacy_pool_infos_bucket: Bucket<LegacyPoolInfo> =
        Bucket::new(storage, PREFIX_POOL_INFO);

    let mut pools: Vec<(CanonicalAddr, LegacyPoolInfo)> = vec![];
    for item in legacy_pool_infos_bucket.range(None, None, Order::Ascending) {
        let (k, p) = item?;
        pools.push((CanonicalAddr::from(k), p));
    }

    for (asset, _) in pools.clone().into_iter() {
        legacy_pool_infos_bucket.remove(asset.as_slice());
    }

    let mut new_pool_infos_bucket: Bucket<PoolInfo> = Bucket::new(storage, PREFIX_POOL_INFO);

    for (asset, legacy_pool_info) in pools.into_iter() {
        let new_pool_info = &PoolInfo {
            staking_token: legacy_pool_info.staking_token,
            total_bond_amount: legacy_pool_info.total_bond_amount,
            total_short_amount: legacy_pool_info.total_short_amount,
            reward_index: legacy_pool_info.reward_index,
            short_reward_index: legacy_pool_info.short_reward_index,
            pending_reward: legacy_pool_info.pending_reward,
            short_pending_reward: legacy_pool_info.short_pending_reward,
            premium_rate: legacy_pool_info.premium_rate,
            short_reward_weight: legacy_pool_info.short_reward_weight,
            premium_updated_time: legacy_pool_info.premium_updated_time,
            migration_params: None,
        };
        new_pool_infos_bucket.save(asset.as_slice(), new_pool_info)?;
    }

    Ok(())
}

#[cfg(test)]
mod migrate_tests {
    use crate::state::read_pool_info;

    use super::*;
    use cosmwasm_std::{testing::mock_dependencies, Api};

    pub fn pool_infos_old_store(storage: &mut dyn Storage) -> Bucket<LegacyPoolInfo> {
        Bucket::new(storage, PREFIX_POOL_INFO)
    }

    #[test]
    fn test_pool_infos_migration() {
        let mut deps = mock_dependencies(&[]);
        let mut legacy_store = pool_infos_old_store(&mut deps.storage);

        let asset_1 = deps.api.addr_canonicalize("asset1").unwrap();
        let pool_info_1 = LegacyPoolInfo {
            staking_token: deps.api.addr_canonicalize("staking1").unwrap(),
            total_bond_amount: Uint128::from(1u128),
            total_short_amount: Uint128::from(1u128),
            reward_index: Decimal::percent(1),
            short_reward_index: Decimal::percent(1),
            pending_reward: Uint128::from(1u128),
            short_pending_reward: Uint128::from(1u128),
            premium_rate: Decimal::percent(1),
            short_reward_weight: Decimal::percent(1),
            premium_updated_time: 1,
        };
        let asset_2 = deps.api.addr_canonicalize("asset2").unwrap();
        let pool_info_2 = LegacyPoolInfo {
            staking_token: deps.api.addr_canonicalize("staking2").unwrap(),
            total_bond_amount: Uint128::from(2u128),
            total_short_amount: Uint128::from(2u128),
            reward_index: Decimal::percent(2),
            short_reward_index: Decimal::percent(2),
            pending_reward: Uint128::from(2u128),
            short_pending_reward: Uint128::from(2u128),
            premium_rate: Decimal::percent(2),
            short_reward_weight: Decimal::percent(2),
            premium_updated_time: 2,
        };

        legacy_store.save(asset_1.as_slice(), &pool_info_1).unwrap();
        legacy_store.save(asset_2.as_slice(), &pool_info_2).unwrap();

        migrate_pool_infos(deps.as_mut().storage).unwrap();

        let new_pool_info_1: PoolInfo = read_pool_info(deps.as_mut().storage, &asset_1).unwrap();
        let new_pool_info_2: PoolInfo = read_pool_info(deps.as_mut().storage, &asset_2).unwrap();

        assert_eq!(
            new_pool_info_1,
            PoolInfo {
                staking_token: deps.api.addr_canonicalize("staking1").unwrap(),
                total_bond_amount: Uint128::from(1u128),
                total_short_amount: Uint128::from(1u128),
                reward_index: Decimal::percent(1),
                short_reward_index: Decimal::percent(1),
                pending_reward: Uint128::from(1u128),
                short_pending_reward: Uint128::from(1u128),
                premium_rate: Decimal::percent(1),
                short_reward_weight: Decimal::percent(1),
                premium_updated_time: 1,
                migration_params: None,
            }
        );
        assert_eq!(
            new_pool_info_2,
            PoolInfo {
                staking_token: deps.api.addr_canonicalize("staking2").unwrap(),
                total_bond_amount: Uint128::from(2u128),
                total_short_amount: Uint128::from(2u128),
                reward_index: Decimal::percent(2),
                short_reward_index: Decimal::percent(2),
                pending_reward: Uint128::from(2u128),
                short_pending_reward: Uint128::from(2u128),
                premium_rate: Decimal::percent(2),
                short_reward_weight: Decimal::percent(2),
                premium_updated_time: 2,
                migration_params: None,
            }
        )
    }
}
