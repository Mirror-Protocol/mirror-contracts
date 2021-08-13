use crate::math::{decimal_division, decimal_subtraction};
use cosmwasm_std::{
    to_binary, Addr, Decimal, Deps, QuerierWrapper, QueryRequest, StdResult, Uint128, WasmQuery,
};
use mirror_protocol::oracle::{PriceResponse, QueryMsg as OracleQueryMsg};
use mirror_protocol::short_reward::{QueryMsg as ShortRewardQueryMsg, ShortRewardWeightResponse};
use terraswap::{
    asset::AssetInfo, asset::PairInfo, pair::PoolResponse, pair::QueryMsg as PairQueryMsg,
    querier::query_pair_info,
};

pub fn compute_premium_rate(
    deps: Deps,
    oracle_contract: Addr,
    factory_contract: Addr,
    asset_token: Addr,
    base_denom: String,
) -> StdResult<(Decimal, bool)> {
    let pair_info: PairInfo = query_pair_info(
        &deps.querier,
        factory_contract,
        &[
            AssetInfo::NativeToken {
                denom: base_denom.to_string(),
            },
            AssetInfo::Token {
                contract_addr: asset_token.to_string(),
            },
        ],
    )?;

    let pool: PoolResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: pair_info.contract_addr,
        msg: to_binary(&PairQueryMsg::Pool {})?,
    }))?;

    let terraswap_price: Decimal = if pool.assets[0].is_native_token() {
        if pool.assets[1].amount.is_zero() {
            Decimal::from_ratio(pool.assets[0].amount, Uint128::from(1u128))
        } else {
            Decimal::from_ratio(pool.assets[0].amount, pool.assets[1].amount)
        }
    } else if pool.assets[0].amount.is_zero() {
        Decimal::from_ratio(pool.assets[1].amount, Uint128::from(1u128))
    } else {
        Decimal::from_ratio(pool.assets[1].amount, pool.assets[0].amount)
    };

    let oracle_price: Decimal =
        query_price(deps, oracle_contract, asset_token.to_string(), base_denom)?;

    if oracle_price.is_zero() {
        Ok((Decimal::zero(), true))
    } else if terraswap_price > oracle_price {
        Ok((
            decimal_division(
                decimal_subtraction(terraswap_price, oracle_price),
                oracle_price,
            ),
            false,
        ))
    } else {
        Ok((Decimal::zero(), false))
    }
}

pub fn compute_short_reward_weight(
    querier: &QuerierWrapper,
    short_reward_contract: Addr,
    premium_rate: Decimal,
) -> StdResult<Decimal> {
    let res: ShortRewardWeightResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: short_reward_contract.to_string(),
        msg: to_binary(&ShortRewardQueryMsg::ShortRewardWeight { premium_rate })?,
    }))?;

    Ok(res.short_reward_weight)
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
