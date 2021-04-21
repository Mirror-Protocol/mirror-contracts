use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_binary, from_slice, to_binary, Api, Coin, Decimal, Empty, Extern, HumanAddr, Querier,
    QuerierResult, QueryRequest, SystemError, Uint128, WasmQuery,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::math::decimal_division;
use mirror_protocol::oracle::PriceResponse;
use terraswap::asset::{Asset, AssetInfo};
use terraswap::pair::PoolResponse;

/// mock_dependencies is a drop-in replacement for cosmwasm_std::testing::mock_dependencies
/// this uses our CustomQuerier.
pub fn mock_dependencies(
    canonical_length: usize,
    contract_balance: &[Coin],
) -> Extern<MockStorage, MockApi, WasmMockQuerier> {
    let contract_addr = HumanAddr::from(MOCK_CONTRACT_ADDR);
    let custom_querier: WasmMockQuerier = WasmMockQuerier::new(
        MockQuerier::new(&[(&contract_addr, contract_balance)]),
        MockApi::new(canonical_length),
        canonical_length,
    );

    Extern {
        storage: MockStorage::default(),
        api: MockApi::new(canonical_length),
        querier: custom_querier,
    }
}

pub struct WasmMockQuerier {
    base: MockQuerier<Empty>,
    oracle_price_querier: OraclePriceQuerier,
    terraswap_pools_querier: TerraswapPoolsQuerier,
    canonical_length: usize,
}

#[derive(Clone, Default)]
pub struct OraclePriceQuerier {
    // this lets us iterate over all pairs that match the first string
    oracle_price: HashMap<String, Decimal>,
}

impl OraclePriceQuerier {
    pub fn new(oracle_price: &[(&String, &Decimal)]) -> Self {
        OraclePriceQuerier {
            oracle_price: oracle_price_to_map(oracle_price),
        }
    }
}

pub(crate) fn oracle_price_to_map(
    oracle_price: &[(&String, &Decimal)],
) -> HashMap<String, Decimal> {
    let mut oracle_price_map: HashMap<String, Decimal> = HashMap::new();
    for (base_quote, oracle_price) in oracle_price.iter() {
        oracle_price_map.insert((*base_quote).clone(), **oracle_price);
    }

    oracle_price_map
}

#[derive(Clone, Default)]
pub struct TerraswapPoolsQuerier {
    pools: HashMap<HumanAddr, (Uint128, Uint128)>,
}

impl TerraswapPoolsQuerier {
    pub fn new(pools: &[(&HumanAddr, (&Uint128, &Uint128))]) -> Self {
        TerraswapPoolsQuerier {
            pools: pools_to_map(pools),
        }
    }
}

pub(crate) fn pools_to_map(
    pools: &[(&HumanAddr, (&Uint128, &Uint128))],
) -> HashMap<HumanAddr, (Uint128, Uint128)> {
    let mut pools_map: HashMap<HumanAddr, (Uint128, Uint128)> = HashMap::new();
    for (key, pool) in pools.into_iter() {
        pools_map.insert(HumanAddr::from(key), (*pool.0, *pool.1));
    }
    pools_map
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        // MockQuerier doesn't support Custom, so we ignore it completely here
        let request: QueryRequest<Empty> = match from_slice(bin_request) {
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
pub enum QueryMsg {
    Price {
        base_asset: String,
        quote_asset: String,
    },
    Pool {},
}

impl WasmMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
        match &request {
            QueryRequest::Wasm(WasmQuery::Smart { contract_addr, msg }) => match from_binary(&msg)
                .unwrap()
            {
                QueryMsg::Price {
                    base_asset,
                    quote_asset,
                } => match self.oracle_price_querier.oracle_price.get(&base_asset) {
                    Some(base_price) => {
                        match self.oracle_price_querier.oracle_price.get(&quote_asset) {
                            Some(quote_price) => Ok(to_binary(&PriceResponse {
                                rate: decimal_division(*base_price, *quote_price),
                                last_updated_base: 1000u64,
                                last_updated_quote: 1000u64,
                            })),
                            None => Err(SystemError::InvalidRequest {
                                error: "No oracle price exists".to_string(),
                                request: msg.as_slice().into(),
                            }),
                        }
                    }
                    None => Err(SystemError::InvalidRequest {
                        error: "No oracle price exists".to_string(),
                        request: msg.as_slice().into(),
                    }),
                },
                QueryMsg::Pool {} => match self.terraswap_pools_querier.pools.get(&contract_addr) {
                    Some(v) => Ok(to_binary(&PoolResponse {
                        assets: [
                            Asset {
                                amount: v.0,
                                info: AssetInfo::NativeToken {
                                    denom: "uusd".to_string(),
                                },
                            },
                            Asset {
                                amount: v.1,
                                info: AssetInfo::Token {
                                    contract_addr: HumanAddr::from("token"),
                                },
                            },
                        ],
                        total_share: Uint128::zero(),
                    })),
                    None => Err(SystemError::InvalidRequest {
                        error: "No pair info exists".to_string(),
                        request: msg.as_slice().into(),
                    }),
                },
            },
            _ => self.base.handle_query(request),
        }
    }
}

impl WasmMockQuerier {
    pub fn new<A: Api>(base: MockQuerier<Empty>, _api: A, canonical_length: usize) -> Self {
        WasmMockQuerier {
            base,
            oracle_price_querier: OraclePriceQuerier::default(),
            terraswap_pools_querier: TerraswapPoolsQuerier::default(),
            canonical_length,
        }
    }

    // configure the oracle price mock querier
    pub fn with_oracle_price(&mut self, oracle_price: &[(&String, &Decimal)]) {
        self.oracle_price_querier = OraclePriceQuerier::new(oracle_price);
    }

    pub fn with_terraswap_pools(&mut self, pairs: &[(&HumanAddr, (&Uint128, &Uint128))]) {
        self.terraswap_pools_querier = TerraswapPoolsQuerier::new(pairs);
    }
}