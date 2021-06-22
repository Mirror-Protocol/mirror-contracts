use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, Decimal, Order, StdResult, Storage};
use cosmwasm_storage::{singleton_read, ReadonlyBucket};

use crate::state::{store_asset_config, store_config, AssetConfig, Config};

static PREFIX_ASSET_CONFIG: &[u8] = b"asset_config";
static KEY_CONFIG: &[u8] = b"config";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyConfig {
    pub owner: CanonicalAddr,
    pub oracle: CanonicalAddr,
    pub collector: CanonicalAddr,
    pub base_denom: String,
    pub token_code_id: u64,
    pub protocol_fee_rate: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyAssetConfig {
    pub token: CanonicalAddr,
    pub auction_discount: Decimal,
    pub min_collateral_ratio: Decimal,
    pub end_price: Option<Decimal>,
}

fn read_legacy_config<S: Storage>(storage: &S) -> StdResult<LegacyConfig> {
    singleton_read(storage, KEY_CONFIG).load()
}

pub fn migrate_config<S: Storage>(
    storage: &mut S,
    staking: CanonicalAddr,
    terraswap_factory: CanonicalAddr,
    collateral_oracle: CanonicalAddr,
    lock: CanonicalAddr,
) -> StdResult<()> {
    let legacy_config: LegacyConfig = read_legacy_config(storage)?;
    store_config(
        storage,
        &Config {
            staking,
            terraswap_factory,
            collateral_oracle,
            lock,
            owner: legacy_config.owner,
            oracle: legacy_config.oracle,
            collector: legacy_config.collector,
            base_denom: legacy_config.base_denom,
            token_code_id: legacy_config.token_code_id,
            protocol_fee_rate: legacy_config.protocol_fee_rate,
        },
    )?;

    Ok(())
}

fn read_legacy_asset_configs<S: Storage>(storage: &S) -> StdResult<Vec<LegacyAssetConfig>> {
    let asset_config_bucket: ReadonlyBucket<S, LegacyAssetConfig> =
        ReadonlyBucket::new(PREFIX_ASSET_CONFIG, storage);
    asset_config_bucket
        .range(None, None, Order::Ascending)
        .map(|item| {
            let (_, v) = item?;
            Ok(v)
        })
        .collect()
}

pub fn migrate_asset_configs<S: Storage>(storage: &mut S) -> StdResult<()> {
    let legacy_asset_configs: Vec<LegacyAssetConfig> = read_legacy_asset_configs(storage)?;

    for legacy_config in legacy_asset_configs {
        store_asset_config(
            storage,
            &legacy_config.token,
            &AssetConfig {
                token: legacy_config.token.clone(),
                auction_discount: legacy_config.auction_discount,
                min_collateral_ratio: legacy_config.min_collateral_ratio,
                end_price: legacy_config.end_price,
                ipo_params: None,
            },
        )?
    }
    Ok(())
}

#[cfg(test)]
mod migrate_tests {
    use super::*;
    use crate::state::{read_asset_config, read_config};
    use cosmwasm_std::testing::mock_dependencies;
    use cosmwasm_std::{Api, HumanAddr};
    use cosmwasm_storage::{singleton, Bucket};

    pub fn asset_config_old_store<'a, S: Storage>(
        storage: &'a mut S,
    ) -> Bucket<'a, S, LegacyAssetConfig> {
        Bucket::new(PREFIX_ASSET_CONFIG, storage)
    }

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
        let oracle = deps
            .api
            .canonical_address(&HumanAddr::from("oracle0000"))
            .unwrap();
        let collector = deps
            .api
            .canonical_address(&HumanAddr::from("collector0000"))
            .unwrap();
        let staking = deps
            .api
            .canonical_address(&HumanAddr::from("staking0000"))
            .unwrap();
        let terraswap_factory = deps
            .api
            .canonical_address(&HumanAddr::from("terraswap_factory"))
            .unwrap();
        let collateral_oracle = deps
            .api
            .canonical_address(&HumanAddr::from("collateral_oracle"))
            .unwrap();
        let lock = deps
            .api
            .canonical_address(&HumanAddr::from("lock0000"))
            .unwrap();
        store_legacy_config(
            &mut deps.storage,
            &LegacyConfig {
                owner: owner.clone(),
                oracle: oracle.clone(),
                collector: collector.clone(),
                base_denom: "uusd".to_string(),
                token_code_id: 10,
                protocol_fee_rate: Decimal::percent(1),
            },
        )
        .unwrap();

        migrate_config(
            &mut deps.storage,
            staking.clone(),
            terraswap_factory.clone(),
            collateral_oracle.clone(),
            lock.clone(),
        )
        .unwrap();
        assert_eq!(
            read_config(&deps.storage).unwrap(),
            Config {
                owner,
                oracle,
                staking,
                collector,
                terraswap_factory,
                lock,
                collateral_oracle,
                base_denom: "uusd".to_string(),
                token_code_id: 10,
                protocol_fee_rate: Decimal::percent(1),
            }
        );
    }

    #[test]
    fn test_asset_config_migration() {
        let mut deps = mock_dependencies(20, &[]);

        let asset_token = deps
            .api
            .canonical_address(&HumanAddr::from("token0001"))
            .unwrap();
        let legacy_asset_config = LegacyAssetConfig {
            token: asset_token.clone(),
            auction_discount: Decimal::percent(10),
            min_collateral_ratio: Decimal::percent(150),
            end_price: None,
        };
        let asset_token_2 = deps
            .api
            .canonical_address(&HumanAddr::from("token0002"))
            .unwrap();
        let legacy_asset_config_2 = LegacyAssetConfig {
            token: asset_token_2.clone(),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(200),
            end_price: Some(Decimal::percent(1)),
        };

        asset_config_old_store(&mut deps.storage)
            .save(asset_token.as_slice(), &legacy_asset_config)
            .unwrap();
        asset_config_old_store(&mut deps.storage)
            .save(asset_token_2.as_slice(), &legacy_asset_config_2)
            .unwrap();

        migrate_asset_configs(&mut deps.storage).unwrap();

        assert_eq!(
            read_asset_config(&mut deps.storage, &asset_token).unwrap(),
            AssetConfig {
                token: legacy_asset_config.token.clone(),
                auction_discount: legacy_asset_config.auction_discount,
                min_collateral_ratio: legacy_asset_config.min_collateral_ratio,
                end_price: legacy_asset_config.end_price,
                ipo_params: None,
            }
        );
        assert_eq!(
            read_asset_config(&mut deps.storage, &asset_token_2).unwrap(),
            AssetConfig {
                token: legacy_asset_config_2.token.clone(),
                auction_discount: legacy_asset_config_2.auction_discount,
                min_collateral_ratio: legacy_asset_config_2.min_collateral_ratio,
                end_price: legacy_asset_config_2.end_price,
                ipo_params: None,
            }
        );
    }
}
