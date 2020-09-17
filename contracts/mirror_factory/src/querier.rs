use cosmwasm_std::{
    from_binary, Api, Binary, CanonicalAddr, Extern, HumanAddr, Querier, QueryRequest, StdResult,
    Storage, WasmQuery,
};

use cosmwasm_storage::to_length_prefixed;
use serde::Deserialize;

use uniswap::{AssetInfoRaw, PairInfoRaw};

// need to query uniswap contract to uniswap factory contract
// and liquidity token to uniswap contract

pub fn load_pair_contract<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    contract_addr: &HumanAddr,
    asset_infos: &[AssetInfoRaw; 2],
) -> StdResult<CanonicalAddr> {
    let mut asset_infos = asset_infos.clone().to_vec();
    asset_infos.sort_by(|a, b| a.as_bytes().cmp(&b.as_bytes()));

    let res: Binary = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
        contract_addr: HumanAddr::from(contract_addr),
        key: Binary::from(concat(
            &to_length_prefixed(b"pair").to_vec(),
            &[asset_infos[0].as_bytes(), asset_infos[1].as_bytes()].concat(),
        )),
    }))?;

    let pair_info: PairInfoRaw = from_binary(&res)?;

    Ok(pair_info.contract_addr)
}

#[derive(Deserialize)]
pub struct UniswapPairConfig {
    pub owner: CanonicalAddr,
    pub contract_addr: CanonicalAddr,
    pub liquidity_token: CanonicalAddr,
    pub commission_collector: CanonicalAddr,
}

pub fn load_staking_token<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    contract_addr: &HumanAddr,
) -> StdResult<CanonicalAddr> {
    // load price form the oracle
    let res: Binary = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
        contract_addr: contract_addr.clone(),
        key: Binary::from(concat(&to_length_prefixed(b"config"), b"general")),
    }))?;

    let config: UniswapPairConfig = from_binary(&res)?;
    Ok(config.liquidity_token)
}

#[inline]
fn concat(namespace: &[u8], key: &[u8]) -> Vec<u8> {
    let mut k = namespace.to_vec();
    k.extend_from_slice(key);
    k
}
