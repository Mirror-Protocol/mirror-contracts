use cosmwasm_std::{
    from_binary, Api, Binary, CanonicalAddr, Decimal, Extern, HumanAddr, Querier, QueryRequest,
    StdError, StdResult, Storage, Uint128, WasmQuery,
};

use crate::state::Config;
use cosmwasm_storage::to_length_prefixed;
use serde::{Deserialize, Serialize};
use terraswap::AssetInfoRaw;

const PRICE_EXPIRE_TIME: u64 = 60;

/// ReverseSimulationResponse returns reverse swap simulation response
#[derive(Serialize, Deserialize)]
pub struct PriceInfo {
    pub price: Decimal,
    pub price_multiplier: Decimal,
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

    Ok(decimal_multiplication(
        price_info.price,
        price_info.price_multiplier,
    ))
}

const DECIMAL_FRACTIONAL: Uint128 = Uint128(1_000_000_000u128);

pub fn decimal_multiplication(a: Decimal, b: Decimal) -> Decimal {
    Decimal::from_ratio(a * DECIMAL_FRACTIONAL * b, DECIMAL_FRACTIONAL)
}

#[inline]
fn concat(namespace: &[u8], key: &[u8]) -> Vec<u8> {
    let mut k = namespace.to_vec();
    k.extend_from_slice(key);
    k
}
