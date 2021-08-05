use cosmwasm_std::{
    Addr, Binary, CanonicalAddr, Decimal, Deps, QueryRequest, StdError, StdResult, WasmQuery,
};

use cosmwasm_storage::to_length_prefixed;
use serde::{Deserialize, Serialize};

pub fn load_oracle_feeder(
    deps: Deps,
    contract_addr: Addr,
    asset_token: &CanonicalAddr,
) -> StdResult<CanonicalAddr> {
    let res: StdResult<CanonicalAddr> = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
        contract_addr: contract_addr.to_string(),
        key: Binary::from(concat(
            &to_length_prefixed(b"feeder"),
            asset_token.as_slice(),
        )),
    }));

    let feeder: CanonicalAddr = match res {
        Ok(v) => v,
        Err(_) => {
            return Err(StdError::generic_err("Failed to fetch the oracle feeder"));
        }
    };

    Ok(feeder)
}

#[derive(Serialize, Deserialize)]
pub struct MintAssetConfig {
    pub token: CanonicalAddr,
    pub auction_discount: Decimal,
    pub min_collateral_ratio: Decimal,
}

pub fn load_mint_asset_config(
    deps: Deps,
    contract_addr: Addr,
    asset_token: &CanonicalAddr,
) -> StdResult<(Decimal, Decimal)> {
    let res: StdResult<MintAssetConfig> = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
        contract_addr: contract_addr.to_string(),
        key: Binary::from(concat(
            &to_length_prefixed(b"asset_config"),
            asset_token.as_slice(),
        )),
    }));

    // let asset_config: StdResult<MintAssetConfig> = from_binary(&res);
    let asset_config: MintAssetConfig = match res {
        Ok(v) => v,
        Err(_) => {
            return Err(StdError::generic_err(
                "Failed to fetch the mint asset config",
            ));
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
