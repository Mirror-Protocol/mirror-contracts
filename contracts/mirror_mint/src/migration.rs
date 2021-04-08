use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, Decimal, Order, StdResult, Storage};
use cosmwasm_storage::ReadonlyBucket;

use crate::state::{store_asset_config, AssetConfig, PREFIX_ASSET_CONFIG};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyAssetConfig {
    pub token: CanonicalAddr,
    pub auction_discount: Decimal,
    pub min_collateral_ratio: Decimal,
    pub end_price: Option<Decimal>,
    pub mint_end: Option<u64>,
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
