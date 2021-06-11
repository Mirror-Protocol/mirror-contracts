use cosmwasm_std::{
    to_binary, Api, Decimal, Extern, HumanAddr, Querier, QueryRequest, StdResult, Storage, Uint128,
    WasmQuery,
};

use terraswap::{
    asset::AssetInfo, asset::PairInfo, pair::PoolResponse, pair::QueryMsg as PairQueryMsg,
    querier::query_pair_info,
};

use crate::math::{decimal_division, decimal_subtraction};

use mirror_protocol::{oracle::PriceResponse, oracle::QueryMsg as OracleQueryMsg};

pub fn compute_premium_rate<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    oracle_contract: &HumanAddr,
    factory_contract: &HumanAddr,
    asset_token: &HumanAddr,
    base_denom: String,
) -> StdResult<(Decimal, bool)> {
    let pair_info: PairInfo = query_pair_info(
        deps,
        &factory_contract,
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
        contract_addr: pair_info.contract_addr,
        msg: to_binary(&PairQueryMsg::Pool {})?,
    }))?;

    let terraswap_price: Decimal = if pool.assets[0].is_native_token() {
        if pool.assets[1].amount.is_zero() {
            Decimal::from_ratio(pool.assets[0].amount, Uint128(1))
        } else {
            Decimal::from_ratio(pool.assets[0].amount, pool.assets[1].amount)
        }
    } else {
        if pool.assets[0].amount.is_zero() {
            Decimal::from_ratio(pool.assets[1].amount, Uint128(1))
        } else {
            Decimal::from_ratio(pool.assets[1].amount, pool.assets[0].amount)
        }
    };
    let oracle_price: Decimal =
        query_price(deps, oracle_contract, asset_token.to_string(), base_denom)?;

    if terraswap_price > oracle_price {
        Ok((
            decimal_division(
                decimal_subtraction(terraswap_price, oracle_price),
                oracle_price,
            ),
            false,
        ))
    } else if oracle_price.is_zero() {
        Ok((Decimal::zero(), true))
    } else {
        Ok((Decimal::zero(), false))
    }
}

pub fn query_price<S: Storage, A: Api, Q: Querier>(
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
