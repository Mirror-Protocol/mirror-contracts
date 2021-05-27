use cosmwasm_std::{
    from_binary, Api, Decimal, Extern, Querier, QueryRequest, StdError, StdResult, Storage,
    Uint128, WasmQuery,
};
use std::str::FromStr;

use cosmwasm_bignumber::Decimal256;
use mirror_protocol::collateral_oracle::SourceType;
use serde::{Deserialize, Serialize};
use terra_cosmwasm::{ExchangeRatesResponse, TerraQuerier};
use terraswap::asset::{Asset, AssetInfo};

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
    price_source: SourceType,
    base_denom: String,
) -> StdResult<(Decimal, u64)> {
    match price_source {
        SourceType::BandOracle { band_oracle_query } => {
            let wasm_query: WasmQuery = from_binary(&band_oracle_query)?;
            let res: BandOracleResponse = deps.querier.query(&QueryRequest::Wasm(wasm_query))?;
            let rate: Decimal = parse_band_rate(res.rate)?;

            Ok((rate, res.last_updated_base))
        }
        SourceType::FixedPrice { price } => return Ok((price, u64::MAX)),
        SourceType::TerraOracle { terra_oracle_query } => {
            let wasm_query: WasmQuery = from_binary(&terra_oracle_query)?;
            let res: TerraOracleResponse = deps.querier.query(&QueryRequest::Wasm(wasm_query))?;

            Ok((res.rate, res.last_updated_base))
        }
        SourceType::Terraswap { terraswap_query } => {
            let wasm_query: WasmQuery = from_binary(&terraswap_query)?;
            let res: TerraswapResponse = deps.querier.query(&QueryRequest::Wasm(wasm_query))?;
            let assets: [Asset; 2] = res.assets;

            let rate: Decimal = if assets[0].info.equal(&AssetInfo::NativeToken {
                denom: base_denom.clone(),
            }) {
                Decimal::from_ratio(assets[0].amount, assets[1].amount)
            } else if assets[1].info.equal(&AssetInfo::NativeToken {
                denom: base_denom.clone(),
            }) {
                Decimal::from_ratio(assets[1].amount, assets[0].amount)
            } else {
                return Err(StdError::generic_err("Invalid pool"));
            };

            Ok((rate, u64::MAX))
        }
        SourceType::AnchorMarket {
            anchor_market_query,
        } => {
            let wasm_query: WasmQuery = from_binary(&anchor_market_query)?;
            let res: AnchorMarketResponse = deps.querier.query(&QueryRequest::Wasm(wasm_query))?;
            let rate: Decimal = res.exchange_rate.into();

            Ok((rate, u64::MAX))
        }
        SourceType::Native { native_denom } => {
            let terra_querier = TerraQuerier::new(&deps.querier);
            let res: ExchangeRatesResponse =
                terra_querier.query_exchange_rates(native_denom, vec![base_denom])?;

            Ok((res.exchange_rates[0].exchange_rate, u64::MAX))
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
