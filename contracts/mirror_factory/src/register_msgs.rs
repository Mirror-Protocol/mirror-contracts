use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, HumanAddr};
use terraswap::{AssetInfo, InitHook};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MintHandleMsg {
    RegisterAsset {
        asset_token: HumanAddr,
        auction_discount: Decimal,
        min_collateral_ratio: Decimal,
    },
    RegisterMigration {
        from_token: HumanAddr,
        to_token: HumanAddr,
        conversion_rate: Decimal,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TerraswapFactoryHandleMsg {
    CreatePair {
        /// Pair contract owner
        pair_owner: HumanAddr,
        /// Inactive commission collector
        commission_collector: HumanAddr,
        /// Commission rate for active liquidity provider
        lp_commission: Decimal,
        /// Commission rate for owner controlled commission
        owner_commission: Decimal,
        /// Asset infos
        asset_infos: [AssetInfo; 2],
        /// Init hook
        init_hook: Option<InitHook>,
    },
    MigrateAsset {
        from_asset_infos: [AssetInfo; 2],
        to_asset_infos: [AssetInfo; 2],
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TerraswapPairHandleMsg {
    MigrateAsset {
        from_asset: AssetInfo,
        to_asset: AssetInfo,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StakingHandleMsg {
    RegisterAsset {
        asset_token: HumanAddr,
        staking_token: HumanAddr,
    },
    RegisterMigration {
        from_token: HumanAddr,
        to_token: HumanAddr,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OracleHandleMsg {
    RegisterAsset {
        asset_token: HumanAddr,
        feeder: HumanAddr,
    },
    MigrateAsset {
        from_token: HumanAddr,
        to_token: HumanAddr,
    },
}
