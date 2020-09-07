use cosmwasm_std::{
    Api, BalanceResponse, BankQuery, Binary, CanonicalAddr, Decimal, Extern, HumanAddr, Querier,
    QueryRequest, StdError, StdResult, Storage, Uint128, WasmQuery,
};

use crate::math::decimal_multiplication;
use cosmwasm_storage::to_length_prefixed;
use cw20::TokenInfoResponse;
use serde::{Deserialize, Serialize};

const PRICE_EXPIRE_TIME: u64 = 30;

/// ReverseSimulationResponse returns reverse swap simulation response
#[derive(Serialize, Deserialize)]
pub struct PriceInfo {
    pub price: Decimal,
    pub price_multiplier: Decimal,
    pub last_update_time: u64,
}

pub fn load_balance<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    account_addr: &HumanAddr,
    denom: String,
) -> StdResult<Uint128> {
    // load price form the oracle
    let balance: BalanceResponse = deps.querier.query(&QueryRequest::Bank(BankQuery::Balance {
        address: HumanAddr::from(account_addr),
        denom,
    }))?;
    Ok(balance.amount.amount)
}

pub fn load_price<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    contract_addr: &HumanAddr,
    block_time: Option<u64>,
) -> StdResult<Decimal> {
    // load price form the oracle
    let price_info: PriceInfo = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
        contract_addr: HumanAddr::from(contract_addr),
        key: Binary::from(b"price"),
    }))?;

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

pub fn load_token_balance<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    contract_addr: &HumanAddr,
    account_addr: &CanonicalAddr,
) -> StdResult<Uint128> {
    // load balance form the token contract
    let balance: Uint128 = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
        contract_addr: HumanAddr::from(contract_addr),
        key: Binary::from(concat(
            &to_length_prefixed(b"balances").to_vec(),
            account_addr.as_slice(),
        )),
    }))?;

    Ok(balance)
}

pub fn load_supply<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    contract_addr: &HumanAddr,
) -> StdResult<Uint128> {
    // load price form the oracle
    let token_info: TokenInfoResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
            contract_addr: HumanAddr::from(contract_addr),
            key: Binary::from(concat(
                &to_length_prefixed(b"config").to_vec(),
                b"total_supply",
            )),
        }))?;

    Ok(token_info.total_supply)
}

#[inline]
fn concat(namespace: &[u8], key: &[u8]) -> Vec<u8> {
    let mut k = namespace.to_vec();
    k.extend_from_slice(key);
    k
}
