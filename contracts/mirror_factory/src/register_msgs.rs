use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, HumanAddr};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MintHandleMsg {
    RegisterAsset {
        asset_token: HumanAddr,
        auction_discount: Decimal,
        min_collateral_ratio: Decimal,
    },
    RegisterMigration {
        asset_token: HumanAddr,
        end_price: Decimal,
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
        asset_token: HumanAddr,
        feeder: HumanAddr,
    },
}
