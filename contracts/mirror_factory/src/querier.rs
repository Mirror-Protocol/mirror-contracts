use cosmwasm_std::{
    from_binary, to_binary, Api, Binary, CanonicalAddr, Decimal, Extern, HumanAddr, Querier,
    QueryRequest, StdError, StdResult, Storage, WasmQuery,
};

use cosmwasm_storage::to_length_prefixed;
use mirror_protocol::oracle::{PriceResponse, QueryMsg as OracleQueryMsg};
use serde::{Deserialize, Serialize};

pub fn load_oracle_feeder<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    contract_addr: &HumanAddr,
    asset_token: &CanonicalAddr,
) -> StdResult<CanonicalAddr> {
    let res: StdResult<Binary> = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
        contract_addr: HumanAddr::from(contract_addr),
        key: Binary::from(concat(
            &to_length_prefixed(b"feeder"),
            asset_token.as_slice(),
        )),
    }));

    let res = match res {
        Ok(v) => v,
        Err(_) => {
            return Err(StdError::generic_err("Failed to fetch the oracle feeder"));
        }
    };

    let feeder: StdResult<CanonicalAddr> = from_binary(&res);
    let feeder: CanonicalAddr = match feeder {
        Ok(v) => v,
        Err(_) => {
            return Err(StdError::generic_err("Failed to fetch the oracle feeder"));
        }
    };

    Ok(feeder)
}

/// Query asset price igonoring price age
pub fn query_last_price<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    oracle: &HumanAddr,
    base_asset: String,
    quote_asset: String,
) -> StdResult<Decimal> {
    let res: PriceResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: HumanAddr::from(oracle),
        msg: to_binary(&OracleQueryMsg::Price {
            base_asset,
            quote_asset,
        })?,
    }))?;

    Ok(res.rate)
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
            &to_length_prefixed(b"asset_config"),
            asset_token.as_slice(),
        )),
    }));

    let res = match res {
        Ok(v) => v,
        Err(_) => {
            return Err(StdError::generic_err(
                "Failed to fetch the mint asset config",
            ));
        }
    };

    let asset_config: StdResult<MintAssetConfig> = from_binary(&res);
    let asset_config: MintAssetConfig = match asset_config {
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
