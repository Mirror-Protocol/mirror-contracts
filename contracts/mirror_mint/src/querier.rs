use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    to_binary, Api, Decimal, Extern, HumanAddr, Querier, QueryRequest, StdError, StdResult,
    Storage, WasmQuery,
};

use crate::math::decimal_division;
use crate::state::{read_config, read_end_price, Config};
use terraswap::AssetInfoRaw;

const PRICE_EXPIRE_TIME: u64 = 60;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleQueryMsg {
    Price {
        base_asset: String,
        quote_asset: String,
    },
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize)]
pub struct PriceResponse {
    pub rate: Decimal,
    pub last_updated_base: u64,
    pub last_updated_quote: u64,
}

pub fn load_price<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    oracle: &HumanAddr,
    base_asset: &AssetInfoRaw,
    quote_asset: &AssetInfoRaw,
    block_time: Option<u64>,
) -> StdResult<Decimal> {
    let config: Config = read_config(&deps.storage)?;

    let base_end_price = read_end_price(&deps.storage, &base_asset);
    let quote_end_price = read_end_price(&deps.storage, &quote_asset);
    let base_asset = (base_asset.to_normal(&deps)?).to_string();
    let quote_asset = (quote_asset.to_normal(&deps)?).to_string();

    // load price form the oracle
    let price: Decimal =
        if let (Some(base_end_price), Some(quote_end_price)) = (base_end_price, quote_end_price) {
            decimal_division(base_end_price, quote_end_price)
        } else if let Some(base_end_price) = base_end_price {
            let quote_price = if config.base_denom == quote_asset {
                Decimal::one()
            } else {
                query_price(deps, oracle, config.base_denom, quote_asset, block_time)?
            };

            decimal_division(base_end_price, quote_price)
        } else if let Some(quote_end_price) = quote_end_price {
            let base_price = if config.base_denom == base_asset {
                Decimal::one()
            } else {
                query_price(deps, oracle, config.base_denom, base_asset, block_time)?
            };

            decimal_division(base_price, quote_end_price)
        } else {
            query_price(deps, oracle, base_asset, quote_asset, block_time)?
        };

    Ok(price)
}

pub fn query_price<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    oracle: &HumanAddr,
    base_asset: String,
    quote_asset: String,
    block_time: Option<u64>,
) -> StdResult<Decimal> {
    let res: PriceResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: HumanAddr::from(oracle),
        msg: to_binary(&OracleQueryMsg::Price {
            base_asset,
            quote_asset,
        })?,
    }))?;

    if let Some(block_time) = block_time {
        if res.last_updated_base < (block_time - PRICE_EXPIRE_TIME)
            || res.last_updated_quote < (block_time - PRICE_EXPIRE_TIME)
        {
            return Err(StdError::generic_err("Price is too old"));
        }
    }

    Ok(res.rate)
}
