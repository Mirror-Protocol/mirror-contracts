use cosmwasm_bignumber::Decimal256;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::Decimal;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub base_denom: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum HubExecuteMsg {
    /// Owner operation to update the owner parameter
    UpdateOwner { owner: String },
    /// Registers a new proxy contract for an asset_token
    RegisterProxy {
        asset_token: String,
        proxy_addr: String,
        priority: Option<u8>,
    },
    /// Updates the priority paramter of an existing proxy
    UpdatePriority {
        asset_token: String,
        proxy_addr: String,
        priority: u8,
    },
    /// Remves an already whitelisted proxy
    RemoveProxy {
        asset_token: String,
        proxy_addr: String,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum HubQueryMsg {
    /// Queries contract configuration
    Config {},
    /// Queries the information of all registered proxies for the provided asset_token
    ProxyList { asset_token: String },
    /// Queries the highes priority available price within the timeframe
    /// If timeframe is not provided, it will ignore the price age
    Price {
        asset_token: String,
        timeframe: Option<u64>,
    },
    /// Queries all registered proxy prices for the provied asset_token
    PriceList { asset_token: String },
    /// Anchor legacy query interface for oracle prices
    LegacyPrice { base: String, quote: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub base_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PriceResponse {
    pub rate: Decimal,
    pub last_updated: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum PriceQueryResult {
    Success(PriceResponse),
    Fail,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PriceListResponse {
    pub price_list: Vec<(u8, PriceQueryResult)>, // (priority, result)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ProxyListResponse {
    pub asset_token: String,
    pub proxies: Vec<(u8, String)>,
}

impl From<crate::proxy::ProxyPriceResponse> for PriceResponse {
    fn from(proxy_res: crate::proxy::ProxyPriceResponse) -> Self {
        PriceResponse {
            rate: proxy_res.rate,
            last_updated: proxy_res.last_updated,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyPriceResponse {
    pub rate: Decimal256,
    pub last_updated_base: u64,
    pub last_updated_quote: u64,
}
