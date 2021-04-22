use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Api, CanonicalAddr, Decimal, HumanAddr, Order, StdResult, Storage};
use cosmwasm_storage::{singleton, singleton_read, ReadonlyBucket, ReadonlySingleton, Singleton};

use crate::state::{store_asset_config, AssetConfig, Config};

#[cfg(test)]
use cosmwasm_storage::Bucket;

static PREFIX_ASSET_CONFIG: &[u8] = b"asset_config";
static KEY_CONFIG: &[u8] = b"config";

#[cfg(test)]
pub fn asset_config_old_store<'a, S: Storage>(
    storage: &'a mut S,
) -> Bucket<'a, S, LegacyAssetConfig> {
    Bucket::new(PREFIX_ASSET_CONFIG, storage)
}

#[cfg(test)]
pub fn config_old_store<'a, S: Storage>(storage: &'a mut S) -> Singleton<'a, S, LegacyConfig> {
    Singleton::new(storage, KEY_CONFIG)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyAssetConfig {
    pub token: CanonicalAddr,
    pub auction_discount: Decimal,
    pub min_collateral_ratio: Decimal,
    pub end_price: Option<Decimal>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyConfig {
    pub owner: CanonicalAddr,
    pub oracle: CanonicalAddr,
    pub collector: CanonicalAddr,
    pub base_denom: String,
    pub token_code_id: u64,
    pub protocol_fee_rate: Decimal,
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
                mint_end: None,
                min_collateral_ratio_after_migration: None,
            },
        )?
    }
    Ok(())
}

pub fn migrate_config<S: Storage, A: Api>(
    storage: &mut S,
    api: &A,
    collateral_oracle: HumanAddr,
) -> StdResult<()> {
    let legacy_store: ReadonlySingleton<S, LegacyConfig> = singleton_read(storage, KEY_CONFIG);
    let legacy_config: LegacyConfig = legacy_store.load()?;
    let config = Config {
        owner: legacy_config.owner,
        oracle: legacy_config.oracle,
        collector: legacy_config.collector,
        collateral_oracle: api.canonical_address(&collateral_oracle)?,
        base_denom: legacy_config.base_denom,
        token_code_id: legacy_config.token_code_id,
        protocol_fee_rate: legacy_config.protocol_fee_rate,
    };
    let mut store: Singleton<S, Config> = singleton(storage, KEY_CONFIG);
    store.save(&config)?;
    Ok(())
}

#[cfg(test)]
mod migrate_tests {
    use super::*;
    use crate::state::{read_asset_config, read_config};
    use cosmwasm_std::testing::mock_dependencies;

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
                mint_end: None,
                min_collateral_ratio_after_migration: None,
            }
        );
        assert_eq!(
            read_asset_config(&mut deps.storage, &asset_token_2).unwrap(),
            AssetConfig {
                token: legacy_asset_config_2.token.clone(),
                auction_discount: legacy_asset_config_2.auction_discount,
                min_collateral_ratio: legacy_asset_config_2.min_collateral_ratio,
                end_price: legacy_asset_config_2.end_price,
                mint_end: None,
                min_collateral_ratio_after_migration: None,
            }
        );
    }

    #[test]
    fn test_config_migration() {
        let mut deps = mock_dependencies(20, &[]);
        let mut legacy_config_store = config_old_store(&mut deps.storage);

        let legacy_config = LegacyConfig {
            owner: deps
                .api
                .canonical_address(&HumanAddr::from("owner"))
                .unwrap(),
            oracle: deps
                .api
                .canonical_address(&HumanAddr::from("oracle"))
                .unwrap(),
            collector: deps
                .api
                .canonical_address(&HumanAddr::from("collector"))
                .unwrap(),
            base_denom: "uusd".to_string(),
            token_code_id: 1u64,
            protocol_fee_rate: Decimal::percent(1),
        };

        legacy_config_store.save(&legacy_config).unwrap();

        let collateral_oracle: HumanAddr = HumanAddr::from("collateral_oracle");
        migrate_config(&mut deps.storage, &deps.api, collateral_oracle.clone()).unwrap();

        let config: Config = read_config(&deps.storage).unwrap();
        assert_eq!(
            config,
            Config {
                owner: legacy_config.owner,
                oracle: legacy_config.oracle,
                collector: legacy_config.collector,
                collateral_oracle: deps.api.canonical_address(&collateral_oracle).unwrap(),
                base_denom: legacy_config.base_denom,
                token_code_id: legacy_config.token_code_id,
                protocol_fee_rate: legacy_config.protocol_fee_rate,
            }
        )
    }
}
