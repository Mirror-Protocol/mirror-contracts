use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_binary, from_slice, to_binary, Coin, ContractResult, Decimal, OwnedDeps, Querier,
    QuerierResult, QueryRequest, SystemError, SystemResult, Uint128, WasmQuery,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::math::decimal_division;
use mirror_protocol::collateral_oracle::CollateralPriceResponse;
use mirror_protocol::oracle::{FeederResponse, PriceResponse};
use terra_cosmwasm::{TaxCapResponse, TaxRateResponse, TerraQuery, TerraQueryWrapper, TerraRoute};
use terraswap::{asset::AssetInfo, asset::PairInfo};

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
    tax_querier: TaxQuerier,
    oracle_price_querier: OraclePriceQuerier,
    collateral_oracle_querier: CollateralOracleQuerier,
    terraswap_pair_querier: TerraswapPairQuerier,
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
pub struct CollateralOracleQuerier {
    // this lets us iterate over all pairs that match the first string
    collateral_infos: HashMap<String, (Decimal, Decimal, bool)>,
}

impl CollateralOracleQuerier {
    pub fn new(collateral_infos: &[(&String, &Decimal, &Decimal, &bool)]) -> Self {
        CollateralOracleQuerier {
            collateral_infos: collateral_infos_to_map(collateral_infos),
        }
    }
}

pub(crate) fn collateral_infos_to_map(
    collateral_infos: &[(&String, &Decimal, &Decimal, &bool)],
) -> HashMap<String, (Decimal, Decimal, bool)> {
    let mut collateral_infos_map: HashMap<String, (Decimal, Decimal, bool)> = HashMap::new();
    for (collateral, collateral_price, collateral_multiplier, is_revoked) in collateral_infos.iter()
    {
        collateral_infos_map.insert(
            (*collateral).clone(),
            (**collateral_price, **collateral_multiplier, **is_revoked),
        );
    }

    collateral_infos_map
}

#[derive(Clone, Default)]
pub struct TerraswapPairQuerier {
    // this lets us iterate over all pairs that match the first string
    pairs: HashMap<String, String>,
}

impl TerraswapPairQuerier {
    pub fn new(pairs: &[(&String, &String, &String)]) -> Self {
        TerraswapPairQuerier {
            pairs: paris_to_map(pairs),
        }
    }
}

pub(crate) fn paris_to_map(pairs: &[(&String, &String, &String)]) -> HashMap<String, String> {
    let mut pairs_map: HashMap<String, String> = HashMap::new();
    for (asset1, asset2, pair) in pairs.iter() {
        pairs_map.insert((asset1.to_string() + asset2).clone(), pair.to_string());
    }

    pairs_map
}

#[derive(Clone, Default)]
pub struct OracleQuerier {
    feeders: HashMap<String, String>,
}

impl OracleQuerier {
    pub fn new(feeders: &[(&String, &String)]) -> Self {
        OracleQuerier {
            feeders: address_pair_to_map(feeders),
        }
    }
}

pub(crate) fn address_pair_to_map(address_pair: &[(&String, &String)]) -> HashMap<String, String> {
    let mut address_pair_map: HashMap<String, String> = HashMap::new();
    for (addr1, addr2) in address_pair.iter() {
        address_pair_map.insert(addr1.to_string(), addr2.to_string());
    }
    address_pair_map
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
pub enum MockQueryMsg {
    Price {
        base_asset: String,
        quote_asset: String,
    },
    CollateralPrice {
        asset: String,
    },
    Pair {
        asset_infos: [AssetInfo; 2],
    },
    Feeder {
        asset_token: String,
    },
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
                            SystemResult::Ok(ContractResult::from(to_binary(&res)))
                        }
                        TerraQuery::TaxCap { denom } => {
                            let cap = self
                                .tax_querier
                                .caps
                                .get(denom)
                                .copied()
                                .unwrap_or_default();
                            let res = TaxCapResponse { cap };
                            SystemResult::Ok(ContractResult::from(to_binary(&res)))
                        }
                        _ => panic!("DO NOT ENTER HERE"),
                    }
                } else {
                    panic!("DO NOT ENTER HERE")
                }
            }
            QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: _,
                msg,
            }) => match from_binary(msg).unwrap() {
                MockQueryMsg::Price {
                    base_asset,
                    quote_asset,
                } => match self.oracle_price_querier.oracle_price.get(&base_asset) {
                    Some(base_price) => {
                        match self.oracle_price_querier.oracle_price.get(&quote_asset) {
                            Some(quote_price) => {
                                SystemResult::Ok(ContractResult::from(to_binary(&PriceResponse {
                                    rate: decimal_division(*base_price, *quote_price),
                                    last_updated_base: 1000u64,
                                    last_updated_quote: 1000u64,
                                })))
                            }
                            None => SystemResult::Err(SystemError::InvalidRequest {
                                error: "No oracle price exists".to_string(),
                                request: msg.as_slice().into(),
                            }),
                        }
                    }
                    None => SystemResult::Err(SystemError::InvalidRequest {
                        error: "No oracle price exists".to_string(),
                        request: msg.as_slice().into(),
                    }),
                },
                MockQueryMsg::CollateralPrice { asset } => {
                    match self.collateral_oracle_querier.collateral_infos.get(&asset) {
                        Some(collateral_info) => SystemResult::Ok(ContractResult::from(to_binary(
                            &CollateralPriceResponse {
                                asset,
                                rate: collateral_info.0,
                                last_updated: 1000u64,
                                multiplier: collateral_info.1,
                                is_revoked: collateral_info.2,
                            },
                        ))),
                        None => SystemResult::Err(SystemError::InvalidRequest {
                            error: "Collateral info does not exist".to_string(),
                            request: msg.as_slice().into(),
                        }),
                    }
                }
                MockQueryMsg::Pair { asset_infos } => {
                    match self
                        .terraswap_pair_querier
                        .pairs
                        .get(&(asset_infos[0].to_string() + &asset_infos[1].to_string()))
                    {
                        Some(pair) => {
                            SystemResult::Ok(ContractResult::from(to_binary(&PairInfo {
                                asset_infos,
                                contract_addr: pair.to_string(),
                                liquidity_token: "liquidity".to_string(),
                            })))
                        }
                        None => SystemResult::Err(SystemError::InvalidRequest {
                            error: "No pair exists".to_string(),
                            request: msg.as_slice().into(),
                        }),
                    }
                }
                MockQueryMsg::Feeder { asset_token } => {
                    match self.oracle_querier.feeders.get(&asset_token) {
                        Some(v) => {
                            SystemResult::Ok(ContractResult::from(to_binary(&FeederResponse {
                                asset_token,
                                feeder: v.to_string(),
                            })))
                        }
                        None => {
                            return SystemResult::Err(SystemError::InvalidRequest {
                                error: format!("Oracle Feeder is not found for {}", asset_token),
                                request: msg.as_slice().into(),
                            })
                        }
                    }
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
            tax_querier: TaxQuerier::default(),
            oracle_price_querier: OraclePriceQuerier::default(),
            collateral_oracle_querier: CollateralOracleQuerier::default(),
            terraswap_pair_querier: TerraswapPairQuerier::default(),
            oracle_querier: OracleQuerier::default(),
        }
    }

    // configure the token owner mock querier
    pub fn with_tax(&mut self, rate: Decimal, caps: &[(&String, &Uint128)]) {
        self.tax_querier = TaxQuerier::new(rate, caps);
    }

    // configure the oracle price mock querier
    pub fn with_oracle_price(&mut self, oracle_price: &[(&String, &Decimal)]) {
        self.oracle_price_querier = OraclePriceQuerier::new(oracle_price);
    }

    // configure the collateral oracle mock querier
    pub fn with_collateral_infos(
        &mut self,
        collateral_infos: &[(&String, &Decimal, &Decimal, &bool)],
    ) {
        self.collateral_oracle_querier = CollateralOracleQuerier::new(collateral_infos);
    }

    // configure the terraswap factory pair mock querier
    pub fn with_terraswap_pair(&mut self, pairs: &[(&String, &String, &String)]) {
        self.terraswap_pair_querier = TerraswapPairQuerier::new(pairs);
    }

    pub fn with_oracle_feeders(&mut self, feeders: &[(&String, &String)]) {
        self.oracle_querier = OracleQuerier::new(feeders);
    }
}
