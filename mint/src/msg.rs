use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, HumanAddr, Uint128};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    /// Collateral should be one of the coin
    pub collateral_denom: String,
    /// Auciton discount rates for the position liquidation
    pub auction_discount: Decimal,
    /// Auciton start condition; if the ratio between asset value <> collateral value
    /// exceed, the position auction will be open
    pub auction_threshold_rate: Decimal,
    /// Mint_capacity follows decimals
    pub mint_capacity: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    /// Post initize step to allow user to set controlled contract address after creating it
    PostInitialize {
        asset_token: HumanAddr,
        asset_oracle: HumanAddr,
        asset_symbol: String,
    },
    UpdateConfig {
        owner: Option<HumanAddr>,
        auction_discount: Option<Decimal>,
        auction_threshold_rate: Option<Decimal>,
        mint_capacity: Option<Decimal>,
    },
    /// Mint a user sends the collateral coins to mint an asset
    Mint {},
    /// Burn a user sends the asset tokens to the contract to get back the collteral tokens
    Burn { amount: Uint128 },
    /// Auction the contract sell the collteral token with discounted price of the asset tokens
    Auction { owner: HumanAddr, amount: Uint128 },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    ConfigGeneral {},
    ConfigAsset {},
    Position { address: HumanAddr },
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigGeneralResponse {
    pub owner: HumanAddr,
    pub collateral_denom: String,
    pub mint_capacity: Decimal,
    pub auction_discount: Decimal,
    pub auction_threshold_rate: Decimal,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigAssetResponse {
    pub symbol: String,
    pub oracle: HumanAddr,
    pub token: HumanAddr,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionResponse {
    pub collateral_amount: Uint128,
    pub asset_amount: Uint128,
    pub is_auction_open: bool,
}
