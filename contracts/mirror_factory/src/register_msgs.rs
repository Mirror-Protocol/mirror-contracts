use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, HumanAddr};
use uniswap::AssetInfo;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MintHandleMsg {
    RegisterAsset {
        asset_token: HumanAddr,
        auction_discount: Decimal,
        auction_threshold_ratio: Decimal,
        min_collateral_ratio: Decimal,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UniswapHandleMsg {
    CreatePair {
        /// Pair contract owner
        pair_owner: HumanAddr,
        /// Inactive commission collector
        commission_collector: HumanAddr,
        /// Commission rate for active liquidity provider
        active_commission: Decimal,
        /// Commission rate for owner controlled commission
        passive_commission: Decimal,
        /// Asset infos
        asset_infos: [AssetInfo; 2],
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StakingHandleMsg {
    RegisterAsset {
        asset_token: HumanAddr,
        staking_token: HumanAddr,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OracleHandleMsg {
    RegisterAsset {
        asset_info: AssetInfo,
        feeder: HumanAddr,
    },
}
