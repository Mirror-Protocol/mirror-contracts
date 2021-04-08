use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, Decimal, Order, StdResult, Storage, Api, HumanAddr};
use cosmwasm_storage::{ReadonlyBucket, Bucket};

use crate::state::{store_asset_config, AssetConfig};

static PREFIX_ASSET_CONFIG: &[u8] = b"asset_config";

#[cfg(test)]
pub fn asset_config_old_store<'a, S: Storage>(
    storage: &'a mut S,
) -> Bucket<'a, S, LegacyAssetConfig> {
    Bucket::new(PREFIX_ASSET_CONFIG, storage)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyAssetConfig {
    pub token: CanonicalAddr,
    pub auction_discount: Decimal,
    pub min_collateral_ratio: Decimal,
    pub end_price: Option<Decimal>,
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
            },
        )?
    }
    Ok(())
}

#[cfg(test)]
mod migrate_tests {
    use super::*;
    use crate::state::read_asset_config;
    use cosmwasm_std::testing::mock_dependencies;

    #[test]
    fn test_asset_config_migration() {
        let mut deps = mock_dependencies(20, &[]);

        let asset_token = deps.api.canonical_address(&HumanAddr::from("token0001")).unwrap(); 
        let legacy_asset_config = LegacyAssetConfig {
            token: asset_token.clone(),
            auction_discount: Decimal::percent(10),
            min_collateral_ratio: Decimal::percent(150),
            end_price: None,
        };
        let asset_token_2 = deps.api.canonical_address(&HumanAddr::from("token0002")).unwrap(); 
        let legacy_asset_config_2 = LegacyAssetConfig {
            token: asset_token_2.clone(),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(200),
            end_price: Some(Decimal::percent(1)),
        };

        asset_config_old_store(&mut deps.storage).save(asset_token.as_slice(), &legacy_asset_config).unwrap();
        asset_config_old_store(&mut deps.storage).save(asset_token_2.as_slice(), &legacy_asset_config_2).unwrap();

        migrate_asset_configs(&mut deps.storage).unwrap();

        assert_eq!(
            read_asset_config(&mut deps.storage, &asset_token).unwrap(),
            AssetConfig {
                token: legacy_asset_config.token.clone(),
                auction_discount: legacy_asset_config.auction_discount,
                min_collateral_ratio: legacy_asset_config.min_collateral_ratio,
                end_price: legacy_asset_config.end_price,
                mint_end: None,
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
            }
        );
    }
}
