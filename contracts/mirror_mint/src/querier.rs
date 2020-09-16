use cosmwasm_std::{
    from_binary, Api, Binary, Decimal, Extern, HumanAddr, Querier, QueryRequest, StdError,
    StdResult, Storage, Uint128, WasmQuery,
};

use cosmwasm_storage::to_length_prefixed;
use serde::{Deserialize, Serialize};
use uniswap::AssetInfoRaw;

const PRICE_EXPIRE_TIME: u64 = 60;

/// ReverseSimulationResponse returns reverse swap simulation response
#[derive(Serialize, Deserialize)]
pub struct PriceInfo {
    pub price: Decimal,
    pub price_multiplier: Decimal,
    pub last_update_time: u64,
}

pub fn load_price<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    contract_addr: &HumanAddr,
    asset_info: &AssetInfoRaw,
    block_time: Option<u64>,
) -> StdResult<Decimal> {
    // load price form the oracle
    let res: Binary = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
        contract_addr: HumanAddr::from(contract_addr),
        key: Binary::from(concat(&to_length_prefixed(b"price"), asset_info.as_bytes())),
    }))?;

    let price_info: PriceInfo = from_binary(&res)?;
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
