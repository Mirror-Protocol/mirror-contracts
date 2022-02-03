use cosmwasm_std::{CanonicalAddr, Decimal, Order, StdResult, Storage};
use cosmwasm_storage::{singleton, singleton_read, Bucket, ReadonlyBucket};
use mirror_protocol::collateral_oracle::{CollateralInfoResponse, SourceType};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub static PREFIX_COLLATERAL_ASSET_INFO: &[u8] = b"collateral_asset_info";
static KEY_CONFIG: &[u8] = b"config";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub mint_contract: CanonicalAddr,
    pub base_denom: String,
    pub mirror_oracle: CanonicalAddr,
    pub anchor_oracle: CanonicalAddr,
    pub band_oracle: CanonicalAddr,
}

pub fn store_config(storage: &mut dyn Storage, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config(storage: &dyn Storage) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CollateralAssetInfo {
    pub asset: String,
    pub price_source: SourceType,
    pub multiplier: Decimal,
    pub is_revoked: bool,
}

pub fn store_collateral_info(
    storage: &mut dyn Storage,
    collateral: &CollateralAssetInfo,
) -> StdResult<()> {
    let mut collaterals_bucket: Bucket<CollateralAssetInfo> =
        Bucket::new(storage, PREFIX_COLLATERAL_ASSET_INFO);
    collaterals_bucket.save(collateral.asset.as_bytes(), collateral)
}

#[allow(clippy::ptr_arg)]
pub fn read_collateral_info(storage: &dyn Storage, id: &String) -> StdResult<CollateralAssetInfo> {
    let price_bucket: ReadonlyBucket<CollateralAssetInfo> =
        ReadonlyBucket::new(storage, PREFIX_COLLATERAL_ASSET_INFO);
    price_bucket.load(id.as_bytes())
}

pub fn read_collateral_infos(storage: &dyn Storage) -> StdResult<Vec<CollateralInfoResponse>> {
    let price_bucket: ReadonlyBucket<CollateralAssetInfo> =
        ReadonlyBucket::new(storage, PREFIX_COLLATERAL_ASSET_INFO);

    price_bucket
        .range(None, None, Order::Ascending)
        .map(|item| {
            let (_, v) = item?;
            Ok(CollateralInfoResponse {
                asset: v.asset,
                source_type: v.price_source.to_string(),
                multiplier: v.multiplier,
                is_revoked: v.is_revoked,
            })
        })
        .collect()
}
