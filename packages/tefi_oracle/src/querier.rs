use cosmwasm_std::{to_binary, Addr, QuerierWrapper, QueryRequest, StdResult, WasmQuery};

use crate::hub::{HubQueryMsg, PriceResponse};
use crate::proxy::{ProxyBaseQuery, ProxyPriceResponse, ProxyQueryMsg};

/// ## Description
/// Queries an asset token price from the orcle proxy contract, price is given in the base denomination
/// ## Parameters
/// * `proxy_addr` - Proxy contract address
/// * `symbol` - Symbol of the asset
pub fn query_proxy_symbol_price(
    querier: &QuerierWrapper,
    proxy_addr: &Addr,
    symbol: String,
) -> StdResult<ProxyPriceResponse> {
    let res: ProxyPriceResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: String::from(proxy_addr),
        msg: to_binary(&ProxyBaseQuery::Base(ProxyQueryMsg::Price { symbol }))?,
    }))?;

    Ok(res)
}

/// ## Description
/// Queries an asstet token price from hub. Hub contract will redirect the query to the corresponding price source.
/// ## Parameters
/// * `oracle_hub_addr` - Oracle hub contract address
/// * `asset_token` - Asset token address. Native assets are not supported
/// * `timeframe` - (optional) Valid price timeframe in seconds
pub fn query_asset_price(
    querier: &QuerierWrapper,
    oracle_hub_addr: &Addr,
    asset_token: &Addr,
    timeframe: Option<u64>,
) -> StdResult<PriceResponse> {
    let res: PriceResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: String::from(oracle_hub_addr),
        msg: to_binary(&HubQueryMsg::Price {
            asset_token: String::from(asset_token),
            timeframe,
        })?,
    }))?;

    Ok(res)
}
