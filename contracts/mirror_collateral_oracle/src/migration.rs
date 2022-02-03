use cosmwasm_storage::Bucket;
use mirror_protocol::collateral_oracle::SourceType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, Order, StdResult, Storage};

use crate::state::{CollateralAssetInfo, PREFIX_COLLATERAL_ASSET_INFO};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyCollateralAssetInfo {
    pub asset: String,
    pub price_source: LegacySourceType,
    pub multiplier: Decimal,
    pub is_revoked: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LegacySourceType {
    MirrorOracle {},
    AnchorOracle {},
    BandOracle {},
    FixedPrice {
        price: Decimal,
    },
    Terraswap {
        terraswap_pair_addr: String,
        intermediate_denom: Option<String>,
    },
    AnchorMarket {
        anchor_market_addr: String,
    },
    Native {
        native_denom: String,
    },
}

pub fn migrate_collateral_infos(storage: &mut dyn Storage) -> StdResult<()> {
    let mut legacy_collateral_infos_bucket: Bucket<LegacyCollateralAssetInfo> =
        Bucket::new(storage, PREFIX_COLLATERAL_ASSET_INFO);

    let mut collateral_infos: Vec<(String, LegacyCollateralAssetInfo)> = vec![];
    for item in legacy_collateral_infos_bucket.range(None, None, Order::Ascending) {
        let (k, p) = item?;
        collateral_infos.push((String::from_utf8(k)?, p));
    }

    for (asset, _) in collateral_infos.clone().into_iter() {
        legacy_collateral_infos_bucket.remove(asset.as_bytes());
    }

    let mut new_pool_infos_bucket: Bucket<CollateralAssetInfo> =
        Bucket::new(storage, PREFIX_COLLATERAL_ASSET_INFO);

    for (_, legacy_collateral_info) in collateral_infos.into_iter() {
        let new_price_source: SourceType = match legacy_collateral_info.price_source {
            LegacySourceType::BandOracle {} => SourceType::BandOracle {},
            LegacySourceType::AnchorOracle {} => SourceType::AnchorOracle {},
            LegacySourceType::MirrorOracle {} => SourceType::MirrorOracle {},
            LegacySourceType::AnchorMarket { anchor_market_addr } => {
                SourceType::AnchorMarket { anchor_market_addr }
            }
            LegacySourceType::FixedPrice { price } => SourceType::FixedPrice { price },
            LegacySourceType::Native { native_denom } => SourceType::Native { native_denom },
            LegacySourceType::Terraswap {
                terraswap_pair_addr,
                intermediate_denom,
            } => SourceType::Terraswap {
                terraswap_pair_addr,
                intermediate_denom,
            },
        };

        let new_collateral_info = &CollateralAssetInfo {
            asset: legacy_collateral_info.asset,
            multiplier: legacy_collateral_info.multiplier,
            price_source: new_price_source,
            is_revoked: legacy_collateral_info.is_revoked,
        };
        new_pool_infos_bucket.save(new_collateral_info.asset.as_bytes(), new_collateral_info)?;
    }

    Ok(())
}
