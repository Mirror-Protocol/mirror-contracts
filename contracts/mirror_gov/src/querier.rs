use cosmwasm_std::{
    Binary, CanonicalAddr, QuerierWrapper, QueryRequest, StdResult, Uint128, WasmQuery,
};

use cosmwasm_storage::to_length_prefixed;

pub fn load_token_balance(
    querier: &QuerierWrapper,
    contract_addr: String,
    account_addr: &CanonicalAddr,
) -> StdResult<Uint128> {
    // load balance form the token contract
    let res: Uint128 = querier
        .query(&QueryRequest::Wasm(WasmQuery::Raw {
            contract_addr,
            key: Binary::from(concat(
                &to_length_prefixed(b"balance").to_vec(),
                account_addr.as_slice(),
            )),
        }))
        .unwrap_or_else(|_| Uint128::zero());

    Ok(res)
}

#[inline]
fn concat(namespace: &[u8], key: &[u8]) -> Vec<u8> {
    let mut k = namespace.to_vec();
    k.extend_from_slice(key);
    k
}
