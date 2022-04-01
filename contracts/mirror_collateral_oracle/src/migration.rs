use cosmwasm_storage::{singleton, singleton_read, Bucket, ReadonlySingleton, Singleton};
use mirror_protocol::collateral_oracle::SourceType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, Decimal, Order, StdError, StdResult, Storage};

use crate::state::{CollateralAssetInfo, Config, KEY_CONFIG, PREFIX_COLLATERAL_ASSET_INFO};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyConfig {
    pub owner: CanonicalAddr,
    pub mint_contract: CanonicalAddr,
    pub base_denom: String,
    pub mirror_oracle: CanonicalAddr,
    pub anchor_oracle: CanonicalAddr,
    pub band_oracle: CanonicalAddr,
}

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
    Lunax {
        staking_contract_addr: String,
    },
}

pub fn migrate_config(storage: &mut dyn Storage) -> StdResult<()> {
    let legacty_store: ReadonlySingleton<LegacyConfig> = singleton_read(storage, KEY_CONFIG);
    let legacy_config: LegacyConfig = legacty_store.load()?;
    let config = Config {
        owner: legacy_config.owner,
        mint_contract: legacy_config.mint_contract,
        base_denom: legacy_config.base_denom,
    };
    let mut store: Singleton<Config> = singleton(storage, KEY_CONFIG);
    store.save(&config)?;
    Ok(())
}

pub fn migrate_collateral_infos(
    storage: &mut dyn Storage,
    mirror_tefi_oracle_addr: String,
) -> StdResult<()> {
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
            LegacySourceType::BandOracle { .. } => {
                return Err(StdError::generic_err("not supported"))
            } // currently there are no assets with this config
            LegacySourceType::AnchorOracle { .. } => SourceType::TefiOracle {
                oracle_addr: mirror_tefi_oracle_addr.clone(),
            },
            LegacySourceType::MirrorOracle { .. } => SourceType::TefiOracle {
                oracle_addr: mirror_tefi_oracle_addr.clone(),
            },
            LegacySourceType::AnchorMarket { anchor_market_addr } => {
                SourceType::AnchorMarket { anchor_market_addr }
            }
            LegacySourceType::FixedPrice { price } => SourceType::FixedPrice { price },
            LegacySourceType::Native { native_denom } => SourceType::Native { native_denom },
            LegacySourceType::Terraswap {
                terraswap_pair_addr,
                intermediate_denom,
            } => SourceType::AmmPair {
                pair_addr: terraswap_pair_addr,
                intermediate_denom,
            },
            LegacySourceType::Lunax {
                staking_contract_addr,
            } => SourceType::Lunax {
                staking_contract_addr,
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

#[cfg(test)]
mod migrate_tests {
    use crate::state::read_collateral_info;

    use super::*;
    use cosmwasm_std::testing::mock_dependencies;

    pub fn collateral_infos_old_store(
        storage: &mut dyn Storage,
    ) -> Bucket<LegacyCollateralAssetInfo> {
        Bucket::new(storage, PREFIX_COLLATERAL_ASSET_INFO)
    }

    #[test]
    fn test_collateral_infos_migration() {
        let mut deps = mock_dependencies(&[]);
        let mut legacy_store = collateral_infos_old_store(&mut deps.storage);

        let col_info_1 = LegacyCollateralAssetInfo {
            asset: "mAPPL0000".to_string(),
            multiplier: Decimal::one(),
            price_source: LegacySourceType::MirrorOracle {},
            is_revoked: false,
        };
        let col_info_2 = LegacyCollateralAssetInfo {
            asset: "anc0000".to_string(),
            multiplier: Decimal::one(),
            price_source: LegacySourceType::Terraswap {
                terraswap_pair_addr: "pair0000".to_string(),
                intermediate_denom: None,
            },
            is_revoked: false,
        };
        let col_info_3 = LegacyCollateralAssetInfo {
            asset: "bluna0000".to_string(),
            multiplier: Decimal::one(),
            price_source: LegacySourceType::AnchorOracle {},
            is_revoked: false,
        };

        legacy_store
            .save(col_info_1.asset.as_bytes(), &col_info_1)
            .unwrap();
        legacy_store
            .save(col_info_2.asset.as_bytes(), &col_info_2)
            .unwrap();
        legacy_store
            .save(col_info_3.asset.as_bytes(), &col_info_3)
            .unwrap();

        migrate_collateral_infos(deps.as_mut().storage, "mirrortefi0000".to_string()).unwrap();

        let new_col_info_1: CollateralAssetInfo =
            read_collateral_info(deps.as_mut().storage, &col_info_1.asset).unwrap();
        let new_col_info_2: CollateralAssetInfo =
            read_collateral_info(deps.as_mut().storage, &col_info_2.asset).unwrap();
        let new_col_info_3: CollateralAssetInfo =
            read_collateral_info(deps.as_mut().storage, &col_info_3.asset).unwrap();

        assert_eq!(
            new_col_info_1,
            CollateralAssetInfo {
                asset: "mAPPL0000".to_string(),
                multiplier: Decimal::one(),
                price_source: SourceType::TefiOracle {
                    oracle_addr: "mirrortefi0000".to_string(),
                },
                is_revoked: false,
            }
        );
        assert_eq!(
            new_col_info_2,
            CollateralAssetInfo {
                asset: "anc0000".to_string(),
                multiplier: Decimal::one(),
                price_source: SourceType::AmmPair {
                    pair_addr: "pair0000".to_string(),
                    intermediate_denom: None,
                },
                is_revoked: false,
            }
        );
        assert_eq!(
            new_col_info_3,
            CollateralAssetInfo {
                asset: "bluna0000".to_string(),
                multiplier: Decimal::one(),
                price_source: SourceType::TefiOracle {
                    oracle_addr: "mirrortefi0000".to_string(),
                },
                is_revoked: false,
            }
        )
    }
}
