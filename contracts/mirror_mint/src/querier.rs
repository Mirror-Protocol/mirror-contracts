use cosmwasm_std::{
    from_binary, Api, Binary, CanonicalAddr, Decimal, Extern, HumanAddr, Querier, QueryRequest,
    StdError, StdResult, Storage, WasmQuery,
};

use crate::state::{read_asset_config, Config};
use cosmwasm_storage::to_length_prefixed;
use serde::{Deserialize, Serialize};
use terraswap::AssetInfoRaw;

const PRICE_EXPIRE_TIME: u64 = 60;

/// ReverseSimulationResponse returns reverse swap simulation response
#[derive(Serialize, Deserialize)]
pub struct PriceInfo {
    pub price: Decimal,
    pub last_update_time: u64,
    pub asset_token: CanonicalAddr,
}

pub fn load_prices<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    config: &Config,
    collateral_info: &AssetInfoRaw,
    asset_info: &AssetInfoRaw,
    time: Option<u64>,
) -> StdResult<(Decimal, Decimal)> {
    let collateral_price = if collateral_info.equal(&config.base_asset_info) {
        Decimal::one()
    } else {
        // load collateral price form the oracle
        load_price(
            &deps,
            &deps.api.human_address(&config.oracle)?,
            &collateral_info,
            time,
        )?
    };

    // load asset price from the oracle
    let asset_price = load_price(
        &deps,
        &deps.api.human_address(&config.oracle)?,
        &asset_info,
        time,
    )?;

    Ok((collateral_price, asset_price))
}

pub fn load_price<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    contract_addr: &HumanAddr,
    asset_info: &AssetInfoRaw,
    block_time: Option<u64>,
) -> StdResult<Decimal> {
    let asset_token_raw = match &asset_info {
        AssetInfoRaw::Token { contract_addr } => contract_addr,
        _ => panic!("DO NOT ENTER HERE"),
    };

    // return static price for the deprecated asset
    let asset_config = read_asset_config(&deps.storage, &asset_token_raw)?;
    if let Some(end_price) = asset_config.end_price {
        return Ok(end_price);
    }

    // load price form the oracle
    let res: StdResult<Binary> = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
        contract_addr: HumanAddr::from(contract_addr),
        key: Binary::from(concat(&to_length_prefixed(b"price"), asset_info.as_bytes())),
    }));

    let res = match res {
        Ok(v) => v,
        Err(_) => {
            return Err(StdError::generic_err("Falied to fetch the price"));
        }
    };

    let price_info: StdResult<PriceInfo> = from_binary(&res);
    let price_info = match price_info {
        Ok(v) => v,
        Err(_) => {
            return Err(StdError::generic_err("Falied to fetch the price"));
        }
    };

    if let Some(block_time) = block_time {
        if price_info.last_update_time < (block_time - PRICE_EXPIRE_TIME) {
            return Err(StdError::generic_err("Price is too old".to_string()));
        }
    }

    Ok(price_info.price)
}

#[inline]
fn concat(namespace: &[u8], key: &[u8]) -> Vec<u8> {
    let mut k = namespace.to_vec();
    k.extend_from_slice(key);
    k
}
