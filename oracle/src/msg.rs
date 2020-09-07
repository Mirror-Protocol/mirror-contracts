use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, HumanAddr};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub asset_token: HumanAddr,
    pub base_denom: String,
    pub quote_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    FeedPrice {
        /// The price of asset with base token
        price: Decimal,
    },
    UpdateConfig {
        owner: Option<HumanAddr>,
        asset_token: Option<HumanAddr>,
        base_denom: Option<String>,
        quote_denom: Option<String>,
        price_multiplier: Option<Decimal>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    Price {},
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: HumanAddr,
    pub asset_token: HumanAddr,
    pub base_denom: String,
    pub quote_denom: String,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PriceResponse {
    pub price: Decimal,
    pub price_multiplier: Decimal,
    pub last_update_time: u64,
}
