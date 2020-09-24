use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_slice, to_binary, Coin, Decimal, Extern, HumanAddr, Querier, QuerierResult, QueryRequest,
    StdError, SystemError, Uint128, WasmQuery,
};
use cosmwasm_storage::to_length_prefixed;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::querier::PriceInfo;
use terra_cosmwasm::{TaxCapResponse, TaxRateResponse, TerraQuery, TerraQueryWrapper, TerraRoute};
use uniswap::AssetInfoRaw;

/// mock_dependencies is a drop-in replacement for cosmwasm_std::testing::mock_dependencies
/// this uses our CustomQuerier.
pub fn mock_dependencies(
    canonical_length: usize,
    contract_balance: &[Coin],
) -> Extern<MockStorage, MockApi, WasmMockQuerier> {
    let contract_addr = HumanAddr::from(MOCK_CONTRACT_ADDR);
    let custom_querier: WasmMockQuerier =
        WasmMockQuerier::new(MockQuerier::new(&[(&contract_addr, contract_balance)]));

    Extern {
        storage: MockStorage::default(),
        api: MockApi::new(canonical_length),
        querier: custom_querier,
    }
}

pub struct WasmMockQuerier {
    base: MockQuerier<TerraQueryWrapper>,
    tax_querier: TaxQuerier,
    oracle_querier: OracleQuerier,
}

#[derive(Clone, Default)]
pub struct TaxQuerier {
    rate: Decimal,
    // this lets us iterate over all pairs that match the first string
    caps: HashMap<String, Uint128>,
}

impl TaxQuerier {
    pub fn new(rate: Decimal, caps: &[(&String, &Uint128)]) -> Self {
        TaxQuerier {
            rate,
            caps: caps_to_map(caps),
        }
    }
}

pub(crate) fn caps_to_map(caps: &[(&String, &Uint128)]) -> HashMap<String, Uint128> {
    let mut owner_map: HashMap<String, Uint128> = HashMap::new();
    for (denom, cap) in caps.iter() {
        owner_map.insert(denom.to_string(), **cap);
    }
    owner_map
}

#[derive(Clone, Default)]
pub struct OracleQuerier {
    // this lets us iterate over all pairs that match the first string
    prices: HashMap<String, Decimal>,
}

impl OracleQuerier {
    pub fn new(prices: &[(&AssetInfoRaw, &Decimal)]) -> Self {
        OracleQuerier {
            prices: prices_to_map(prices),
        }
    }
}

pub(crate) fn prices_to_map(prices: &[(&AssetInfoRaw, &Decimal)]) -> HashMap<String, Decimal> {
    let mut price_map: HashMap<String, Decimal> = HashMap::new();
    for (asset_info, price) in prices.iter() {
        price_map.insert(
            String::from_utf8(asset_info.as_bytes().to_vec()).unwrap(),
            **price,
        );
    }
    price_map
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        // MockQuerier doesn't support Custom, so we ignore it completely here
        let request: QueryRequest<TerraQueryWrapper> = match from_slice(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {}", e),
                    request: bin_request.into(),
                })
            }
        };
        self.handle_query(&request)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MockQueryMsg {
    Price {},
}

impl WasmMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<TerraQueryWrapper>) -> QuerierResult {
        match &request {
            QueryRequest::Custom(TerraQueryWrapper { route, query_data }) => {
                if route == &TerraRoute::Treasury {
                    match query_data {
                        TerraQuery::TaxRate {} => {
                            let res = TaxRateResponse {
                                rate: self.tax_querier.rate,
                            };
                            Ok(to_binary(&res))
                        }
                        TerraQuery::TaxCap { denom } => {
                            let cap = self
                                .tax_querier
                                .caps
                                .get(denom)
                                .copied()
                                .unwrap_or_default();
                            let res = TaxCapResponse { cap };
                            Ok(to_binary(&res))
                        }
                        _ => panic!("DO NOT ENTER HERE"),
                    }
                } else {
                    panic!("DO NOT ENTER HERE")
                }
            }
            QueryRequest::Wasm(WasmQuery::Raw { contract_addr, key }) => {
                let key: &[u8] = key.as_slice();
                let prefix_price = to_length_prefixed(b"price").to_vec();
                if key[..prefix_price.len()].to_vec() == prefix_price {
                    let key_asset_info: &[u8] = &key[prefix_price.len()..];

                    let price = match self
                        .oracle_querier
                        .prices
                        .get(&String::from_utf8(key_asset_info.to_vec()).unwrap())
                    {
                        Some(price) => price,
                        None => {
                            return Ok(Err(StdError::generic_err(format!(
                                "No price registered for {}",
                                contract_addr,
                            ))))
                        }
                    };

                    let price_info = PriceInfo {
                        price: *price,
                        price_multiplier: Decimal::one(),
                        last_update_time: 1000u64,
                    };

                    Ok(to_binary(&to_binary(&price_info).unwrap()))
                } else {
                    panic!("DO NOT ENTER HERE")
                }
            }
            _ => self.base.handle_query(request),
        }
    }
}

impl WasmMockQuerier {
    pub fn new(base: MockQuerier<TerraQueryWrapper>) -> Self {
        WasmMockQuerier {
            base,
            tax_querier: TaxQuerier::default(),
            oracle_querier: OracleQuerier::default(),
        }
    }

    // configure the token owner mock querier
    pub fn with_tax(&mut self, rate: Decimal, caps: &[(&String, &Uint128)]) {
        self.tax_querier = TaxQuerier::new(rate, caps);
    }

    // configure the oracle price mock querier
    pub fn with_oracle_price(&mut self, prices: &[(&AssetInfoRaw, &Decimal)]) {
        self.oracle_querier = OracleQuerier::new(prices);
    }
}
