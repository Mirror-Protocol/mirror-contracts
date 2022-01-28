use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub distribution_contract: String, // collected rewards receiver
    pub terraswap_factory: String,
    pub mirror_token: String,
    pub base_denom: String,
    // aUST params
    pub aust_token: String,
    pub anchor_market: String,
    // bLuna params
    pub bluna_token: String,
    // Lunax params
    pub lunax_token: String,
    // when set, use this address instead of querying from terraswap
    pub mir_ust_pair: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum ExecuteMsg {
    UpdateConfig {
        owner: Option<String>,
        distribution_contract: Option<String>,
        terraswap_factory: Option<String>,
        mirror_token: Option<String>,
        base_denom: Option<String>,
        aust_token: Option<String>,
        anchor_market: Option<String>,
        bluna_token: Option<String>,
        mir_ust_pair: Option<String>,
        lunax_token: Option<String>,
    },
    Convert {
        asset_token: String,
    },
    Distribute {},
    /// Internal operation to swap Luna for UST
    LunaSwapHook {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MoneyMarketCw20HookMsg {
    /// Return stable coins to a user
    /// according to exchange rate
    RedeemStable {},
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub distribution_contract: String, // collected rewards receiver
    pub terraswap_factory: String,
    pub mirror_token: String,
    pub base_denom: String,
    pub aust_token: String,
    pub anchor_market: String,
    pub bluna_token: String,
    pub lunax_token: String,
    pub mir_ust_pair: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {
    pub lunax_token: String,
}
