use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, Decimal, Order, StdResult, Storage, Uint128};
use cosmwasm_storage::{singleton_read, ReadonlyBucket};

use crate::state::{store_config, store_pool_info, Config, PoolInfo, KEY_CONFIG, PREFIX_POOL_INFO};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MainnetLegacyConfig {
    pub owner: CanonicalAddr,
    pub mirror_token: CanonicalAddr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TestnetLegacyConfig {
    pub owner: CanonicalAddr,
    pub mirror_token: CanonicalAddr,
    pub mint_contract: CanonicalAddr,
    pub oracle_contract: CanonicalAddr,
    pub terraswap_factory: CanonicalAddr,
    pub base_denom: String,
    pub premium_min_update_interval: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyPoolInfo {
    pub staking_token: CanonicalAddr,
    pub pending_reward: Uint128, // not distributed amount due to zero bonding
    pub total_bond_amount: Uint128,
    pub reward_index: Decimal,
}

fn read_testnet_legacy_config<S: Storage>(storage: &S) -> StdResult<TestnetLegacyConfig> {
    singleton_read(storage, KEY_CONFIG).load()
}

fn read_mainnet_legacy_config<S: Storage>(storage: &S) -> StdResult<MainnetLegacyConfig> {
    singleton_read(storage, KEY_CONFIG).load()
}

fn read_legacy_pool_infos<S: Storage>(
    storage: &S,
) -> StdResult<Vec<(CanonicalAddr, LegacyPoolInfo)>> {
    let pool_info_bucket: ReadonlyBucket<S, LegacyPoolInfo> =
        ReadonlyBucket::new(PREFIX_POOL_INFO, storage);
    pool_info_bucket
        .range(None, None, Order::Ascending)
        .map(|item| {
            let (k, v) = item?;
            Ok((CanonicalAddr::from(k), v))
        })
        .collect()
}

pub fn migrate_mainnet_config<S: Storage>(
    storage: &mut S,
    mint_contract: CanonicalAddr,
    oracle_contract: CanonicalAddr,
    terraswap_factory: CanonicalAddr,
    base_denom: String,
    premium_min_update_interval: u64,
    short_reward_contract: CanonicalAddr,
) -> StdResult<()> {
    let legacy_config: MainnetLegacyConfig = read_mainnet_legacy_config(storage)?;

    store_config(
        storage,
        &Config {
            owner: legacy_config.owner,
            mirror_token: legacy_config.mirror_token,
            mint_contract,
            oracle_contract,
            terraswap_factory,
            base_denom,
            premium_min_update_interval,
            short_reward_contract,
        },
    )
}

pub fn migrate_testnet_config<S: Storage>(
    storage: &mut S,
    short_reward_contract: CanonicalAddr,
) -> StdResult<()> {
    let legacy_config: TestnetLegacyConfig = read_testnet_legacy_config(storage)?;

    store_config(
        storage,
        &Config {
            owner: legacy_config.owner,
            mirror_token: legacy_config.mirror_token,
            mint_contract: legacy_config.mint_contract,
            oracle_contract: legacy_config.oracle_contract,
            terraswap_factory: legacy_config.terraswap_factory,
            base_denom: legacy_config.base_denom,
            premium_min_update_interval: legacy_config.premium_min_update_interval,
            short_reward_contract,
        },
    )
}

pub fn migrate_pool_infos<S: Storage>(storage: &mut S) -> StdResult<()> {
    let legacy_pool_infos: Vec<(CanonicalAddr, LegacyPoolInfo)> = read_legacy_pool_infos(storage)?;
    for (asset_token, legacy_pool_info) in legacy_pool_infos.iter() {
        store_pool_info(
            storage,
            &asset_token,
            &PoolInfo {
                staking_token: legacy_pool_info.staking_token.clone(),
                pending_reward: legacy_pool_info.pending_reward,
                total_bond_amount: legacy_pool_info.total_bond_amount,
                reward_index: legacy_pool_info.reward_index,
                short_pending_reward: Uint128::zero(),
                total_short_amount: Uint128::zero(),
                short_reward_index: Decimal::zero(),
                premium_rate: Decimal::zero(),
                short_reward_weight: Decimal::zero(),
                premium_updated_time: 0,
            },
        )?;
    }

    Ok(())
}
