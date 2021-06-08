use cosmwasm_std::{
    to_binary, Api, Decimal, Extern, HumanAddr, Querier, QueryRequest, StdError, StdResult,
    Storage, Uint128, WasmQuery,
};
use std::str::FromStr;

use crate::math::decimal_multiplication;
use crate::state::Config;
use cosmwasm_bignumber::Decimal256;
use cosmwasm_bignumber::Uint256;
use mirror_protocol::collateral_oracle::SourceType;
use serde::{Deserialize, Serialize};
use terra_cosmwasm::{ExchangeRatesResponse, TerraQuerier};
use terraswap::asset::{Asset, AssetInfo};
use terraswap::pair::QueryMsg as TerraswapPairQueryMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SourceQueryMsg {
    Price {
        base_asset: String,
        quote_asset: String,
    },
    Pool {},
    GetReferenceData {
        base_symbol: String,
        quote_symbol: String,
    },
    EpochState {
        block_heigth: Option<u64>,
        distributed_interest: Option<Uint256>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TerraOracleResponse {
    // oracle queries returns rate
    pub rate: Decimal,
    pub last_updated_base: u64,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TerraswapResponse {
    // terraswap queries return pool assets
    pub assets: [Asset; 2],
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BandOracleResponse {
    // band oracle queries returns rate (uint128)
    pub rate: Uint128,
    pub last_updated_base: u64,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AnchorMarketResponse {
    // anchor market queries return exchange rate in Decimal256
    pub exchange_rate: Decimal256,
}

pub fn query_price<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    config: &Config,
    asset: &String,
    price_source: &SourceType,
) -> StdResult<(Decimal, u64)> {
    match price_source {
        SourceType::BandOracle {} => {
            let res: BandOracleResponse =
                deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: deps.api.human_address(&config.band_oracle)?,
                    msg: to_binary(&SourceQueryMsg::GetReferenceData {
                        base_symbol: asset.to_string(),
                        quote_symbol: config.base_denom.clone(),
                    })
                    .unwrap(),
                }))?;
            let rate: Decimal = parse_band_rate(res.rate)?;

            Ok((rate, res.last_updated_base))
        }
        SourceType::FixedPrice { price } => return Ok((*price, u64::MAX)),
        SourceType::MirrorOracle {} => {
            let res: TerraOracleResponse =
                deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: deps.api.human_address(&config.mirror_oracle)?,
                    msg: to_binary(&SourceQueryMsg::Price {
                        base_asset: asset.to_string(),
                        quote_asset: config.base_denom.clone(),
                    })
                    .unwrap(),
                }))?;

            Ok((res.rate, res.last_updated_base))
        }
        SourceType::AnchorOracle {} => {
            let res: TerraOracleResponse =
                deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: deps.api.human_address(&config.anchor_oracle)?,
                    msg: to_binary(&SourceQueryMsg::Price {
                        base_asset: asset.to_string(),
                        quote_asset: config.base_denom.clone(),
                    })
                    .unwrap(),
                }))?;

            Ok((res.rate, res.last_updated_base))
        }
        SourceType::Terraswap {
            terraswap_pair_addr,
            intermediate_denom,
        } => {
            let res: TerraswapResponse =
                deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: HumanAddr::from(terraswap_pair_addr),
                    msg: to_binary(&TerraswapPairQueryMsg::Pool {}).unwrap(),
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
                    contract_addr: HumanAddr::from(anchor_market_addr),
                    msg: to_binary(&SourceQueryMsg::EpochState {
                        block_heigth: None,
                        distributed_interest: None,
                    })
                    .unwrap(),
                }))?;
            let rate: Decimal = res.exchange_rate.into();

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

/// Parses a uint that contains the price multiplied by 1e18
fn parse_band_rate(uint_rate: Uint128) -> StdResult<Decimal> {
    // manipulate the uint as a string to prevent overflow
    let mut rate_uint_string: String = uint_rate.to_string();

    let uint_len = rate_uint_string.len();
    if uint_len > 18 {
        let dec_point = rate_uint_string.len() - 18;
        rate_uint_string.insert(dec_point, '.');
    } else {
        let mut prefix: String = "0.".to_owned();
        let dec_zeros = 18 - uint_len;
        for _ in 0..dec_zeros {
            prefix.push('0');
        }
        rate_uint_string = prefix + rate_uint_string.as_str();
    }

    Decimal::from_str(rate_uint_string.as_str())
}

fn query_native_rate<Q: Querier>(
    querier: &Q,
    base_denom: String,
    quote_denom: String,
) -> StdResult<Decimal> {
    let terra_querier = TerraQuerier::new(querier);
    let res: ExchangeRatesResponse =
        terra_querier.query_exchange_rates(quote_denom, vec![base_denom])?;

    Ok(res.exchange_rates[0].exchange_rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_band_rate() {
        let rate_dec_1: Decimal = parse_band_rate(Uint128(3493968700000000000000u128)).unwrap();
        assert_eq!(
            rate_dec_1,
            Decimal::from_str("3493.968700000000000000").unwrap()
        );

        let rate_dec_2: Decimal = parse_band_rate(Uint128(1234u128)).unwrap();
        assert_eq!(
            rate_dec_2,
            Decimal::from_str("0.000000000000001234").unwrap()
        );

        let rate_dec_3: Decimal = parse_band_rate(Uint128(100000000000000001u128)).unwrap();
        assert_eq!(
            rate_dec_3,
            Decimal::from_str("0.100000000000000001").unwrap()
        );
    }
}
