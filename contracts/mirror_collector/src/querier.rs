use cosmwasm_std::{
    from_binary, to_binary, Api, BalanceResponse, BankQuery, Binary, CanonicalAddr, Extern,
    HumanAddr, Querier, QueryRequest, StdResult, Storage, Uint128, WasmQuery,
};

use cosmwasm_storage::to_length_prefixed;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WhitelistInfo {
    pub token_contract: CanonicalAddr,
    pub mint_contract: CanonicalAddr,
    pub market_contract: CanonicalAddr,
    pub oracle_contract: CanonicalAddr,
    pub staking_contract: CanonicalAddr,
}

pub fn load_whitelist_info<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    contract_addr: &HumanAddr,
    symbol: String,
) -> StdResult<WhitelistInfo> {
    deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
        contract_addr: HumanAddr::from(contract_addr),
        key: Binary::from(concat(
            &to_length_prefixed(b"whitelist").to_vec(),
            symbol.as_bytes(),
        )),
    }))
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

pub fn load_token_balance<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    contract_addr: &HumanAddr,
    account_addr: &CanonicalAddr,
) -> StdResult<Uint128> {
    // load balance form the token contract
    let res: Binary = deps
        .querier
        .query(&QueryRequest::Wasm(WasmQuery::Raw {
            contract_addr: HumanAddr::from(contract_addr),
            key: Binary::from(concat(
                &to_length_prefixed(b"balance").to_vec(),
                account_addr.as_slice(),
            )),
        }))
        .unwrap_or_else(|_| to_binary(&Uint128::zero()).unwrap());

    from_binary(&res)
}

#[inline]
fn concat(namespace: &[u8], key: &[u8]) -> Vec<u8> {
    let mut k = namespace.to_vec();
    k.extend_from_slice(key);
    k
}
