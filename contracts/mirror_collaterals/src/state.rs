use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    from_binary, Binary, CanonicalAddr, Decimal, Order, StdResult, Storage, WasmQuery,
};
use cosmwasm_storage::{singleton, singleton_read, Bucket, ReadonlyBucket};

use mirror_protocol::collateral_oracle::CollateralInfoResponse;

static PREFIX_COLLATERAL_ASSET_INFO: &[u8] = b"collateral_asset_info";
static KEY_CONFIG: &[u8] = b"config";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub base_denom: String,
}

pub fn store_config<S: Storage>(storage: &mut S, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config<S: Storage>(storage: &S) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CollateralAssetInfo {
    pub asset: String,
    pub query_request: Binary,
    pub collateral_premium: Decimal,
}

pub fn store_collateral_info<S: Storage>(
    storage: &mut S,
    collateral: &CollateralAssetInfo,
) -> StdResult<()> {
    let mut collaterals_bucket: Bucket<S, CollateralAssetInfo> =
        Bucket::new(PREFIX_COLLATERAL_ASSET_INFO, storage);
    collaterals_bucket.save(collateral.asset.as_bytes(), collateral)
}

pub fn read_collateral_info<S: Storage>(
    storage: &S,
    id: &String,
) -> StdResult<CollateralAssetInfo> {
    let price_bucket: ReadonlyBucket<S, CollateralAssetInfo> =
        ReadonlyBucket::new(PREFIX_COLLATERAL_ASSET_INFO, storage);
    price_bucket.load(id.as_bytes())
}

pub fn read_collateral_infos<S: Storage>(storage: &S) -> StdResult<Vec<CollateralInfoResponse>> {
    let price_bucket: ReadonlyBucket<S, CollateralAssetInfo> =
        ReadonlyBucket::new(PREFIX_COLLATERAL_ASSET_INFO, storage);

    price_bucket
        .range(None, None, Order::Ascending)
        .map(|item| {
            let (_, v) = item?;
            let wasm_query: WasmQuery = from_binary(&v.query_request)?;
            Ok(CollateralInfoResponse {
                asset: v.asset,
                query_request: wasm_query,
                collateral_premium: v.collateral_premium,
            })
        })
        .collect()
}
