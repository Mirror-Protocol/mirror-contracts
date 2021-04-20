use cosmwasm_std::{
    from_binary, Api, Binary, Decimal, Extern, Querier, QueryRequest, StdError, StdResult, Storage,
    WasmQuery,
};

use serde::{Deserialize, Serialize};
use terraswap::asset::Asset;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GenericPriceResponse {
    // oracle queries returns rate
    pub rate: Option<Decimal>,
    // terraswap queries return pool assets
    pub assets: Option<[Asset; 2]>,
}

pub fn query_price<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    query_request: Binary,
) -> StdResult<Decimal> {
    // try to deserialize wasm query
    let wasm_query: WasmQuery = from_binary(&query_request)?;

    // execute generic query
    let res: GenericPriceResponse = deps.querier.query(&QueryRequest::Wasm(wasm_query))?;

    if let Some(rate) = res.rate {
        Ok(rate)
    } else {
        if let Some(assets) = res.assets {
            Ok(Decimal::from_ratio(assets[0].amount, assets[1].amount))
        } else {
            Err(StdError::generic_err("Collateral query_request returned unexpected response"))
        }
    }
}
