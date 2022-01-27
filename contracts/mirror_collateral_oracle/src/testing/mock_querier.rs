use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_binary, from_slice, to_binary, Addr, Coin, ContractResult, Decimal, OwnedDeps, Querier,
    QuerierResult, QueryRequest, SystemError, SystemResult, Timestamp, Uint128, WasmQuery,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use tefi_oracle::hub::PriceResponse as TeFiOraclePriceResponse;
use terra_cosmwasm::{
    ExchangeRateItem, ExchangeRatesResponse, TerraQuery, TerraQueryWrapper, TerraRoute,
};
use terraswap::asset::{Asset, AssetInfo};
use terraswap::pair::PoolResponse;

/// mock_dependencies is a drop-in replacement for cosmwasm_std::testing::mock_dependencies
/// this uses our CustomQuerier.
pub fn mock_dependencies(
    contract_balance: &[Coin],
) -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let custom_querier: WasmMockQuerier =
        WasmMockQuerier::new(MockQuerier::new(&[(MOCK_CONTRACT_ADDR, contract_balance)]));

    OwnedDeps {
        api: MockApi::default(),
        storage: MockStorage::default(),
        querier: custom_querier,
    }
}

pub struct WasmMockQuerier {
    base: MockQuerier<TerraQueryWrapper>,
    oracle_price_querier: OraclePriceQuerier,
    terraswap_pools_querier: TerraswapPoolsQuerier,
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
    pools: HashMap<String, (String, Uint128, String, Uint128)>,
}

impl TerraswapPoolsQuerier {
    #[allow(clippy::type_complexity)]
    pub fn new(pools: &[(&String, (&String, &Uint128, &String, &Uint128))]) -> Self {
        TerraswapPoolsQuerier {
            pools: pools_to_map(pools),
        }
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn pools_to_map(
    pools: &[(&String, (&String, &Uint128, &String, &Uint128))],
) -> HashMap<String, (String, Uint128, String, Uint128)> {
    let mut pools_map: HashMap<String, (String, Uint128, String, Uint128)> = HashMap::new();
    for (key, pool) in pools.iter() {
        pools_map.insert(
            key.to_string(),
            (pool.0.clone(), *pool.1, pool.2.clone(), *pool.3),
        );
    }
    pools_map
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        // MockQuerier doesn't support Custom, so we ignore it completely here
        let request: QueryRequest<TerraQueryWrapper> = match from_slice(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
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
pub struct EpochStateResponse {
    exchange_rate: Decimal256,
    aterra_supply: Uint256,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Price {
        asset_token: String,
        timeframe: Option<u64>,
    },
    Pool {},
    EpochState {
        block_heigth: Option<u64>,
        distributed_interest: Option<Uint256>,
    },
    State {},
}

impl WasmMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<TerraQueryWrapper>) -> QuerierResult {
        match &request {
            QueryRequest::Custom(TerraQueryWrapper { route, query_data }) => {
                if &TerraRoute::Oracle == route {
                    match query_data {
                        TerraQuery::ExchangeRates {
                            base_denom: _,
                            quote_denoms: _,
                        } => {
                            let res = ExchangeRatesResponse {
                                exchange_rates: vec![ExchangeRateItem {
                                    quote_denom: "uusd".to_string(),
                                    exchange_rate: Decimal::from_ratio(5u128, 1u128),
                                }],
                                base_denom: "uluna".to_string(),
                            };
                            SystemResult::Ok(ContractResult::from(to_binary(&res)))
                        }
                        _ => panic!("DO NOT ENTER HERE"),
                    }
                } else {
                    panic!("DO NOT ENTER HERE")
                }
            }
            QueryRequest::Wasm(WasmQuery::Smart { contract_addr, msg }) => match from_binary(msg)
                .unwrap()
            {
                QueryMsg::Price {
                    asset_token,
                    timeframe: _,
                } => match self.oracle_price_querier.oracle_price.get(&asset_token) {
                    Some(base_price) => SystemResult::Ok(ContractResult::from(to_binary(
                        &TeFiOraclePriceResponse {
                            rate: *base_price,
                            last_updated: 1000u64,
                        },
                    ))),
                    None => SystemResult::Err(SystemError::InvalidRequest {
                        error: "No oracle price exists".to_string(),
                        request: msg.as_slice().into(),
                    }),
                },
                QueryMsg::Pool {} => match self.terraswap_pools_querier.pools.get(contract_addr) {
                    Some(v) => SystemResult::Ok(ContractResult::from(to_binary(&PoolResponse {
                        assets: [
                            Asset {
                                amount: v.1,
                                info: AssetInfo::NativeToken { denom: v.0.clone() },
                            },
                            Asset {
                                amount: v.3,
                                info: AssetInfo::Token {
                                    contract_addr: v.2.to_string(),
                                },
                            },
                        ],
                        total_share: Uint128::zero(),
                    }))),
                    None => SystemResult::Err(SystemError::InvalidRequest {
                        error: "No pair info exists".to_string(),
                        request: msg.as_slice().into(),
                    }),
                },
                QueryMsg::EpochState { .. } => {
                    SystemResult::Ok(ContractResult::from(to_binary(&EpochStateResponse {
                        exchange_rate: Decimal256::from_ratio(10, 3),
                        aterra_supply: Uint256::from_str("123123123").unwrap(),
                    })))
                }
                QueryMsg::State {} => {
                    SystemResult::Ok(ContractResult::from(to_binary(&LunaxStateResponse {
                        state: LunaxState {
                            total_staked: Uint128::new(10000000),
                            exchange_rate: Decimal::from_ratio(11_u128, 10_u128), // 1 lunax = 1.1 luna
                            last_reconciled_batch_id: 1,
                            current_undelegation_batch_id: 3,
                            last_undelegation_time: Timestamp::from_seconds(1642932518),
                            last_swap_time: Timestamp::from_seconds(1643116118),
                            last_reinvest_time: Timestamp::from_seconds(1643105318),
                            validators: vec![
                                Addr::unchecked("valid0001"),
                                Addr::unchecked("valid0002"),
                                Addr::unchecked("valid0003"),
                            ],
                            reconciled_funds_to_withdraw: Uint128::new(150_u128),
                        },
                    })))
                }
            },
            _ => self.base.handle_query(request),
        }
    }
}

impl WasmMockQuerier {
    pub fn new(base: MockQuerier<TerraQueryWrapper>) -> Self {
        WasmMockQuerier {
            base,
            oracle_price_querier: OraclePriceQuerier::default(),
            terraswap_pools_querier: TerraswapPoolsQuerier::default(),
        }
    }

    // configure the oracle price mock querier
    pub fn with_oracle_price(&mut self, oracle_price: &[(&String, &Decimal)]) {
        self.oracle_price_querier = OraclePriceQuerier::new(oracle_price);
    }

    #[allow(clippy::type_complexity)]
    pub fn with_terraswap_pools(
        &mut self,
        pairs: &[(&String, (&String, &Uint128, &String, &Uint128))],
    ) {
        self.terraswap_pools_querier = TerraswapPoolsQuerier::new(pairs);
    }
}
