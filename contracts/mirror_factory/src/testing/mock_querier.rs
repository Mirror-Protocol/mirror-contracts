use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_binary, from_slice, to_binary, Api, CanonicalAddr, Coin, ContractResult, Decimal, Empty,
    OwnedDeps, Querier, QuerierResult, QueryRequest, SystemError, SystemResult, WasmQuery,
};
use cosmwasm_storage::to_length_prefixed;

use crate::math::decimal_division;
use crate::querier::MintAssetConfig;
use mirror_protocol::oracle::PriceResponse;
use std::collections::HashMap;
use terraswap::asset::{AssetInfo, PairInfo};

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
    base: MockQuerier<Empty>,
    terraswap_factory_querier: TerraswapFactoryQuerier,
    oracle_querier: OracleQuerier,
    oracle_price_querier: OraclePriceQuerier,
    mint_querier: MintQuerier,
}

#[derive(Clone, Default)]
pub struct TerraswapFactoryQuerier {
    pairs: HashMap<String, String>,
}

impl TerraswapFactoryQuerier {
    pub fn new(pairs: &[(&String, &String)]) -> Self {
        TerraswapFactoryQuerier {
            pairs: pairs_to_map(pairs),
        }
    }
}

pub(crate) fn pairs_to_map(pairs: &[(&String, &String)]) -> HashMap<String, String> {
    let mut pairs_map: HashMap<String, String> = HashMap::new();
    for (key, pair) in pairs.iter() {
        pairs_map.insert(key.to_string(), pair.to_string());
    }
    pairs_map
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

#[derive(Clone, Default)]
pub struct MintQuerier {
    configs: HashMap<String, (Decimal, Decimal)>,
}

impl MintQuerier {
    pub fn new(configs: &[(&String, &(Decimal, Decimal))]) -> Self {
        MintQuerier {
            configs: configs_to_map(configs),
        }
    }
}

pub(crate) fn configs_to_map(
    configs: &[(&String, &(Decimal, Decimal))],
) -> HashMap<String, (Decimal, Decimal)> {
    let mut configs_map: HashMap<String, (Decimal, Decimal)> = HashMap::new();
    for (contract_addr, touple) in configs.iter() {
        configs_map.insert(contract_addr.to_string(), (touple.0, touple.1));
    }
    configs_map
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        // MockQuerier doesn't support Custom, so we ignore it completely here
        let request: QueryRequest<Empty> = match from_slice(bin_request) {
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
pub enum QueryMsg {
    Pair {
        asset_infos: [AssetInfo; 2],
    },
    Price {
        base_asset: String,
        quote_asset: String,
    },
}

impl WasmMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
        match &request {
            QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: _,
                msg,
            }) => match from_binary(msg).unwrap() {
                QueryMsg::Pair { asset_infos } => {
                    let key = asset_infos[0].to_string() + asset_infos[1].to_string().as_str();
                    match self.terraswap_factory_querier.pairs.get(&key) {
                        Some(v) => SystemResult::Ok(ContractResult::from(to_binary(&PairInfo {
                            contract_addr: "pair".to_string(),
                            liquidity_token: v.to_string(),
                            asset_infos: [
                                AssetInfo::NativeToken {
                                    denom: "uusd".to_string(),
                                },
                                AssetInfo::NativeToken {
                                    denom: "uusd".to_string(),
                                },
                            ],
                        }))),
                        None => SystemResult::Err(SystemError::InvalidRequest {
                            error: "No pair info exists".to_string(),
                            request: msg.as_slice().into(),
                        }),
                    }
                }
                QueryMsg::Price {
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
            },
            QueryRequest::Wasm(WasmQuery::Raw { contract_addr, key }) => {
                let key: &[u8] = key.as_slice();
                let prefix_asset_config = to_length_prefixed(b"asset_config").to_vec();
                let prefix_feeder = to_length_prefixed(b"feeder").to_vec();

                let api: MockApi = MockApi::default();
                if key.len() > prefix_feeder.len()
                    && key[..prefix_feeder.len()].to_vec() == prefix_feeder
                {
                    let api: MockApi = MockApi::default();
                    let rest_key: &[u8] = &key[prefix_feeder.len()..];

                    if contract_addr == "oracle0000" {
                        let asset_token: String = api
                            .addr_humanize(&(CanonicalAddr::from(rest_key.to_vec())))
                            .unwrap()
                            .to_string();

                        let feeder = match self.oracle_querier.feeders.get(&asset_token) {
                            Some(v) => v,
                            None => {
                                return SystemResult::Err(SystemError::InvalidRequest {
                                    error: format!(
                                        "Oracle Feeder is not found for {}",
                                        asset_token
                                    ),
                                    request: key.into(),
                                })
                            }
                        };

                        SystemResult::Ok(ContractResult::from(to_binary(
                            &api.addr_canonicalize(feeder).unwrap(),
                        )))
                    } else {
                        panic!("DO NOT ENTER HERE")
                    }
                } else if key.len() > prefix_asset_config.len()
                    && key[..prefix_asset_config.len()].to_vec() == prefix_asset_config
                {
                    let rest_key: &[u8] = &key[prefix_asset_config.len()..];
                    let asset_token: String = api
                        .addr_humanize(&(CanonicalAddr::from(rest_key.to_vec())))
                        .unwrap()
                        .to_string();

                    let config = match self.mint_querier.configs.get(&asset_token) {
                        Some(v) => v,
                        None => {
                            return SystemResult::Err(SystemError::InvalidRequest {
                                error: format!("Mint Config is not found for {}", asset_token),
                                request: key.into(),
                            })
                        }
                    };

                    SystemResult::Ok(ContractResult::from(to_binary(&MintAssetConfig {
                        token: api.addr_canonicalize(&asset_token).unwrap(),
                        auction_discount: config.0,
                        min_collateral_ratio: config.1,
                        ipo_params: None,
                    })))
                } else {
                    panic!("DO NOT ENTER HERE")
                }
            }
            _ => self.base.handle_query(request),
        }
    }
}

impl WasmMockQuerier {
    pub fn new(base: MockQuerier<Empty>) -> Self {
        WasmMockQuerier {
            base,
            terraswap_factory_querier: TerraswapFactoryQuerier::default(),
            mint_querier: MintQuerier::default(),
            oracle_querier: OracleQuerier::default(),
            oracle_price_querier: OraclePriceQuerier::default(),
        }
    }

    // configure the terraswap pair
    pub fn with_terraswap_pairs(&mut self, pairs: &[(&String, &String)]) {
        self.terraswap_factory_querier = TerraswapFactoryQuerier::new(pairs);
    }

    pub fn with_oracle_feeders(&mut self, feeders: &[(&String, &String)]) {
        self.oracle_querier = OracleQuerier::new(feeders);
    }

    pub fn with_mint_configs(&mut self, configs: &[(&String, &(Decimal, Decimal))]) {
        self.mint_querier = MintQuerier::new(configs);
    }

    // configure the oracle price mock querier
    pub fn with_oracle_price(&mut self, oracle_price: &[(&String, &Decimal)]) {
        self.oracle_price_querier = OraclePriceQuerier::new(oracle_price);
    }
}
