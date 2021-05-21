use cosmwasm_std::{
    to_binary, Addr, Decimal, Deps, QueryRequest, StdResult, WasmQuery,
};

use terraswap::{
    asset::AssetInfo, asset::PairInfo, pair::PoolResponse, pair::QueryMsg as PairQueryMsg,
    querier::query_pair_info,
};

use crate::math::{decimal_division, decimal_subtraction};

use mirror_protocol::{oracle::PriceResponse, oracle::QueryMsg as OracleQueryMsg};

pub fn compute_premium_rate(
    deps: Deps,
    oracle_contract: Addr,
    factory_contract: Addr,
    asset_token: Addr,
    base_denom: String,
) -> StdResult<Decimal> {
    let pair_info: PairInfo = query_pair_info(
        &deps.querier,
        factory_contract.clone(),
        &[
            AssetInfo::NativeToken {
                denom: base_denom.to_string(),
            },
            AssetInfo::Token {
                contract_addr: asset_token.clone(),
            },
        ],
    )?;

    let pool: PoolResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: pair_info.contract_addr.to_string(),
        msg: to_binary(&PairQueryMsg::Pool {})?,
    }))?;

    let terraswap_price: Decimal = if pool.assets[0].is_native_token() {
        Decimal::from_ratio(pool.assets[0].amount, pool.assets[1].amount)
    } else {
        Decimal::from_ratio(pool.assets[1].amount, pool.assets[0].amount)
    };
    let oracle_price: Decimal =
        query_price(deps, oracle_contract, asset_token.to_string(), base_denom)?;

    if terraswap_price > oracle_price {
        Ok(decimal_division(
            decimal_subtraction(terraswap_price, oracle_price),
            oracle_price,
        ))
    } else {
        Ok(Decimal::zero())
    }
}

pub fn query_price(
    deps: Deps,
    oracle: Addr,
    base_asset: String,
    quote_asset: String,
) -> StdResult<Decimal> {
    let res: PriceResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: oracle.to_string(),
        msg: to_binary(&OracleQueryMsg::Price {
            base_asset,
            quote_asset,
        })?,
    }))?;

    Ok(res.rate)
}
