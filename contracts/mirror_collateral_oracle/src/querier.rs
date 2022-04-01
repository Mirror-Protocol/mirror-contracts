use crate::math::decimal_multiplication;
use crate::state::Config;
use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{
    to_binary, Addr, Decimal, Deps, Env, QuerierWrapper, QueryRequest, StdError, StdResult,
    Timestamp, Uint128, WasmQuery,
};
use mirror_protocol::collateral_oracle::SourceType;
use serde::{Deserialize, Serialize};
use tefi_oracle::hub::{
    HubQueryMsg as TeFiOracleQueryMsg, PriceResponse as TeFiOraclePriceResponse,
};
use terra_cosmwasm::{ExchangeRatesResponse, TerraQuerier};
use terraswap::asset::{Asset, AssetInfo};
use terraswap::pair::QueryMsg as AMMPairQueryMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SourceQueryMsg {
    // Query message for terraswap pool
    Pool {},
    // Query message for anchor market
    EpochState {
        block_height: Option<u64>,
        distributed_interest: Option<Uint256>,
    },
    // Query message for lunax
    State {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AMMPairResponse {
    // queries return pool assets
    pub assets: [Asset; 2],
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AnchorMarketResponse {
    // anchor market queries return exchange rate in Decimal256
    pub exchange_rate: Decimal256,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LunaxState {
    pub total_staked: Uint128,
    pub exchange_rate: Decimal,
    pub last_reconciled_batch_id: u64,
    pub current_undelegation_batch_id: u64,
    pub last_undelegation_time: Timestamp,
    pub last_swap_time: Timestamp,
    pub last_reinvest_time: Timestamp,
    pub validators: Vec<Addr>,
    pub reconciled_funds_to_withdraw: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LunaxStateResponse {
    pub state: LunaxState,
}

#[allow(clippy::ptr_arg)]
pub fn query_price(
    deps: Deps,
    env: Env,
    config: &Config,
    asset: &String,
    timeframe: Option<u64>,
    price_source: &SourceType,
) -> StdResult<(Decimal, u64)> {
    match price_source {
        SourceType::FixedPrice { price } => Ok((*price, u64::MAX)),
        SourceType::TefiOracle { oracle_addr } => {
            let res: TeFiOraclePriceResponse =
                deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: oracle_addr.to_string(),
                    msg: to_binary(&TeFiOracleQueryMsg::Price {
                        asset_token: asset.to_string(),
                        timeframe,
                    })
                    .unwrap(),
                }))?;

            Ok((res.rate, res.last_updated))
        }
        SourceType::AmmPair {
            pair_addr,
            intermediate_denom,
        } => {
            let res: AMMPairResponse =
                deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: pair_addr.to_string(),
                    msg: to_binary(&AMMPairQueryMsg::Pool {}).unwrap(),
                }))?;
            let assets: [Asset; 2] = res.assets;

            // query intermediate denom if it exists
            let query_denom: String = match intermediate_denom.clone() {
                Some(v) => v,
                None => config.base_denom.clone(),
            };

            let queried_rate: Decimal = if assets[0].info.equal(&AssetInfo::NativeToken {
                denom: query_denom.clone(),
            }) {
                Decimal::from_ratio(assets[0].amount, assets[1].amount)
            } else if assets[1].info.equal(&AssetInfo::NativeToken {
                denom: query_denom.clone(),
            }) {
                Decimal::from_ratio(assets[1].amount, assets[0].amount)
            } else {
                return Err(StdError::generic_err("Invalid pool"));
            };
            // if intermediate denom exists, calculate final rate
            let rate: Decimal = if intermediate_denom.is_some() {
                // (query_denom / intermediate_denom) * (intermedaite_denom / base_denom) = (query_denom / base_denom)
                let native_rate: Decimal =
                    query_native_rate(&deps.querier, query_denom, config.base_denom.clone())?;
                decimal_multiplication(queried_rate, native_rate)
            } else {
                queried_rate
            };

            Ok((rate, u64::MAX))
        }
        SourceType::AnchorMarket { anchor_market_addr } => {
            let res: AnchorMarketResponse =
                deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: anchor_market_addr.to_string(),
                    msg: to_binary(&SourceQueryMsg::EpochState {
                        block_height: Some(env.block.height),
                        distributed_interest: None,
                    })
                    .unwrap(),
                }))?;
            let rate: Decimal = res.exchange_rate.into();

            Ok((rate, u64::MAX))
        }
        SourceType::Lunax {
            staking_contract_addr,
        } => {
            let res: LunaxStateResponse =
                deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: staking_contract_addr.to_string(),
                    msg: to_binary(&SourceQueryMsg::State {}).unwrap(),
                }))?;
            // get lunax price in ust
            let luna_ust_price: Decimal = query_native_rate(
                &deps.querier,
                "uluna".to_string(),
                config.base_denom.clone(),
            )?;
            let rate = decimal_multiplication(res.state.exchange_rate, luna_ust_price);

            Ok((rate, u64::MAX))
        }
        SourceType::Native { native_denom } => {
            let rate: Decimal = query_native_rate(
                &deps.querier,
                native_denom.clone(),
                config.base_denom.clone(),
            )?;

            Ok((rate, u64::MAX))
        }
    }
}

fn query_native_rate(
    querier: &QuerierWrapper,
    base_denom: String,
    quote_denom: String,
) -> StdResult<Decimal> {
    let terra_querier = TerraQuerier::new(querier);
    let res: ExchangeRatesResponse =
        terra_querier.query_exchange_rates(base_denom, vec![quote_denom])?;

    Ok(res.exchange_rates[0].exchange_rate)
}
