use cosmwasm_std::{to_binary, Addr, QuerierWrapper, QueryRequest, StdResult, WasmQuery};

use crate::hub::{HubQueryMsg, PriceResponse};
use crate::proxy::{ProxyPriceResponse, ProxyQueryMsg};

/// @dev Queries an asset token price from the orcle proxy contract, price is given in the base denomination
/// @param proxy_addr : Proxy contract address
/// @param asset_token : Asset token address. Native assets are not supported
pub fn query_proxy_asset_price(
    querier: &QuerierWrapper,
    proxy_addr: &Addr,
    asset_token: &Addr,
) -> StdResult<ProxyPriceResponse> {
    let res: ProxyPriceResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: String::from(proxy_addr),
        msg: to_binary(&ProxyQueryMsg::Price {
            asset_token: String::from(asset_token),
        })?,
    }))?;

    Ok(res)
}

/// @dev Queries an asstet token price from hub. Hub contract will redirect the query to the corresponding price source.
/// @param oracle_hub_addr : Oracle hub contract address
/// @param asset_token : Asset token address. Native assets are not supported
/// @param timeframe : (optional) Valid price timeframe in seconds
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
