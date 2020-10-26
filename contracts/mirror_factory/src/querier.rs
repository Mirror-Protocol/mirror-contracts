use cosmwasm_std::{
    from_binary, Api, Binary, CanonicalAddr, Decimal, Extern, HumanAddr, Querier, QueryRequest,
    StdError, StdResult, Storage, WasmQuery,
};

use cosmwasm_storage::to_length_prefixed;
use serde::{Deserialize, Serialize};

/// ReverseSimulationResponse returns reverse swap simulation response
#[derive(Serialize, Deserialize)]
pub struct OracleAssetConfig {
    pub asset_token: CanonicalAddr,
    pub feeder: CanonicalAddr,
}

pub fn load_oracle_feeder<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    contract_addr: &HumanAddr,
    asset_token: &CanonicalAddr,
) -> StdResult<CanonicalAddr> {
    let res: StdResult<Binary> = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
        contract_addr: HumanAddr::from(contract_addr),
        key: Binary::from(concat(
            &to_length_prefixed(b"asset"),
            asset_token.as_slice(),
        )),
    }));

    let res = match res {
        Ok(v) => v,
        Err(_) => {
            return Err(StdError::generic_err("Falied to fetch the oracle feeder"));
        }
    };

    let asset_config: StdResult<OracleAssetConfig> = from_binary(&res);
    let asset_config: OracleAssetConfig = match asset_config {
        Ok(v) => v,
        Err(_) => {
            return Err(StdError::generic_err("Falied to fetch the oracle feeder"));
        }
    };

    Ok(asset_config.feeder)
}

#[derive(Serialize, Deserialize)]
pub struct PairConfigSwap {
    pub lp_commission: Decimal,
    pub owner_commission: Decimal,
}

pub fn load_commissions<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    contract_addr: &HumanAddr,
) -> StdResult<(Decimal, Decimal)> {
    let res: StdResult<Binary> = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
        contract_addr: HumanAddr::from(contract_addr),
        key: Binary::from(concat(&to_length_prefixed(b"config"), b"swap")),
    }));

    let res = match res {
        Ok(v) => v,
        Err(_) => {
            return Err(StdError::generic_err("Falied to fetch the commissions"));
        }
    };

    let asset_config: StdResult<PairConfigSwap> = from_binary(&res);
    let asset_config: PairConfigSwap = match asset_config {
        Ok(v) => v,
        Err(_) => {
            return Err(StdError::generic_err("Falied to fetch the commissions"));
        }
    };

    Ok((asset_config.lp_commission, asset_config.owner_commission))
}

#[derive(Serialize, Deserialize)]
pub struct MintAssetConfig {
    pub token: CanonicalAddr,
    pub auction_discount: Decimal,
    pub min_collateral_ratio: Decimal,
}

pub fn load_mint_asset_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    contract_addr: &HumanAddr,
    asset_token: &CanonicalAddr,
) -> StdResult<(Decimal, Decimal)> {
    let res: StdResult<Binary> = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
        contract_addr: HumanAddr::from(contract_addr),
        key: Binary::from(concat(
            &to_length_prefixed(b"asset"),
            asset_token.as_slice(),
        )),
    }));

    let res = match res {
        Ok(v) => v,
        Err(_) => {
            return Err(StdError::generic_err("Falied to fetch the mint asset config"));
        }
    };

    let asset_config: StdResult<MintAssetConfig> = from_binary(&res);
    let asset_config: MintAssetConfig = match asset_config {
        Ok(v) => v,
        Err(_) => {
            return Err(StdError::generic_err("Falied to fetch the mint asset config"));
        }
    };

    Ok((
        asset_config.auction_discount,
        asset_config.min_collateral_ratio,
    ))
}

#[inline]
fn concat(namespace: &[u8], key: &[u8]) -> Vec<u8> {
    let mut k = namespace.to_vec();
    k.extend_from_slice(key);
    k
}
