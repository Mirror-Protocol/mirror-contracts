use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    to_binary, Api, Decimal, Extern, HumanAddr, Querier, QueryRequest, StdError, StdResult,
    Storage, WasmQuery,
};

use crate::math::decimal_division;
use crate::state::{read_asset_config, read_config, AssetConfig, Config};

const PRICE_EXPIRE_TIME: u64 = 60;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleQueryMsg {
    Price { base: String, quote: String },
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
    base: String,
    quote: String,
    block_time: Option<u64>,
) -> StdResult<Decimal> {
    let config: Config = read_config(&deps.storage)?;
    let (base_migrated, base_end_price) = if config.base_denom != base {
        let base_config: AssetConfig = read_asset_config(&deps.storage, &base)?;
        if let Some(end_price) = base_config.end_price {
            (true, end_price)
        } else {
            (false, Decimal::one())
        }
    } else {
        (false, Decimal::one())
    };

    let (quote_migrated, quote_end_price) = if config.base_denom != quote {
        let quote_config: AssetConfig = read_asset_config(&deps.storage, &quote)?;
        if let Some(end_price) = quote_config.end_price {
            (true, end_price)
        } else {
            (false, Decimal::one())
        }
    } else {
        (false, Decimal::one())
    };

    // load price form the oracle
    let price: Decimal = if !base_migrated && !quote_migrated {
        query_price(deps, oracle, base, quote, block_time)?
    } else if base_migrated {
        let quote_price = if config.base_denom == quote {
            Decimal::one()
        } else {
            query_price(deps, oracle, config.base_denom, quote, block_time)?
        };

        decimal_division(quote_price, base_end_price)
    } else if quote_migrated {
        let base_price = if config.base_denom == base {
            Decimal::one()
        } else {
            query_price(deps, oracle, config.base_denom, base, block_time)?
        };

        decimal_division(base_price, quote_end_price)
    } else {
        decimal_division(quote_end_price, base_end_price)
    };

    Ok(price)
}

pub fn query_price<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    oracle: &HumanAddr,
    base: String,
    quote: String,
    block_time: Option<u64>,
) -> StdResult<Decimal> {
    let res: PriceResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: HumanAddr::from(oracle),
        msg: to_binary(&OracleQueryMsg::Price { base, quote })?,
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
