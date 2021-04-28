use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_binary, from_slice, to_binary, Api, CanonicalAddr, Coin, Decimal, Extern, HumanAddr,
    Querier, QuerierResult, QueryRequest, SystemError, Uint128, WasmQuery,
};
use cosmwasm_storage::to_length_prefixed;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::math::decimal_division;
use mirror_protocol::collateral_oracle::CollateralPriceResponse;
use mirror_protocol::oracle::PriceResponse;
use terra_cosmwasm::{TaxCapResponse, TaxRateResponse, TerraQuery, TerraQueryWrapper, TerraRoute};
use terraswap::{asset::AssetInfo, asset::PairInfo};

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
    base: MockQuerier<TerraQueryWrapper>,
    token_querier: TokenQuerier,
    tax_querier: TaxQuerier,
    oracle_price_querier: OraclePriceQuerier,
    collateral_oracle_querier: CollateralOracleQuerier,
    terraswap_pair_querier: TerraswapPairQuerier,
    canonical_length: usize,
}

#[derive(Clone, Default)]
pub struct TokenQuerier {
    // this lets us iterate over all pairs that match the first string
    balances: HashMap<HumanAddr, HashMap<HumanAddr, Uint128>>,
}

impl TokenQuerier {
    pub fn new(balances: &[(&HumanAddr, &[(&HumanAddr, &Uint128)])]) -> Self {
        TokenQuerier {
            balances: balances_to_map(balances),
        }
    }
}

pub(crate) fn balances_to_map(
    balances: &[(&HumanAddr, &[(&HumanAddr, &Uint128)])],
) -> HashMap<HumanAddr, HashMap<HumanAddr, Uint128>> {
    let mut balances_map: HashMap<HumanAddr, HashMap<HumanAddr, Uint128>> = HashMap::new();
    for (contract_addr, balances) in balances.iter() {
        let mut contract_balances_map: HashMap<HumanAddr, Uint128> = HashMap::new();
        for (addr, balance) in balances.iter() {
            contract_balances_map.insert(HumanAddr::from(addr), **balance);
        }

        balances_map.insert(HumanAddr::from(contract_addr), contract_balances_map);
    }
    balances_map
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
    for (collateral, collateral_price, collateral_premium, is_revoked) in collateral_infos.iter() {
        collateral_infos_map.insert(
            (*collateral).clone(),
            (**collateral_price, **collateral_premium, **is_revoked),
        );
    }

    collateral_infos_map
}

#[derive(Clone, Default)]
pub struct TerraswapPairQuerier {
    // this lets us iterate over all pairs that match the first string
    pairs: HashMap<String, HumanAddr>,
}

impl TerraswapPairQuerier {
    pub fn new(pairs: &[(&String, &String, &HumanAddr)]) -> Self {
        TerraswapPairQuerier {
            pairs: paris_to_map(pairs),
        }
    }
}

pub(crate) fn paris_to_map(pairs: &[(&String, &String, &HumanAddr)]) -> HashMap<String, HumanAddr> {
    let mut pairs_map: HashMap<String, HumanAddr> = HashMap::new();
    for (asset1, asset2, pair) in pairs.iter() {
        pairs_map.insert(
            (asset1.to_string() + &asset2.to_string()).clone(),
            HumanAddr::from(pair),
        );
    }

    pairs_map
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
            QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: _,
                msg,
            }) => match from_binary(&msg).unwrap() {
                MockQueryMsg::Price {
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
                MockQueryMsg::CollateralPrice { asset } => {
                    match self.collateral_oracle_querier.collateral_infos.get(&asset) {
                        Some(collateral_info) => Ok(to_binary(&CollateralPriceResponse {
                            asset,
                            rate: collateral_info.0,
                            collateral_premium: collateral_info.1,
                            is_revoked: collateral_info.2,
                        })),
                        None => Err(SystemError::InvalidRequest {
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
                        Some(pair) => Ok(to_binary(&PairInfo {
                            asset_infos,
                            contract_addr: pair.clone(),
                            liquidity_token: HumanAddr::from("liquidity"),
                        })),
                        None => Err(SystemError::InvalidRequest {
                            error: "No pair exists".to_string(),
                            request: msg.as_slice().into(),
                        }),
                    }
                }
            },
            QueryRequest::Wasm(WasmQuery::Raw { contract_addr, key }) => {
                let key: &[u8] = key.as_slice();
                let prefix_balance = to_length_prefixed(b"balance").to_vec();

                if key[..prefix_balance.len()].to_vec() == prefix_balance {
                    let balances: &HashMap<HumanAddr, Uint128> =
                        match self.token_querier.balances.get(contract_addr) {
                            Some(balances) => balances,
                            None => {
                                return Err(SystemError::InvalidRequest {
                                    error: format!(
                                        "No balance info exists for the contract {}",
                                        contract_addr
                                    ),
                                    request: key.into(),
                                })
                            }
                        };

                    let key_address: &[u8] = &key[prefix_balance.len()..];
                    let address_raw: CanonicalAddr = CanonicalAddr::from(key_address);

                    let api: MockApi = MockApi::new(self.canonical_length);
                    let address: HumanAddr = match api.human_address(&address_raw) {
                        Ok(v) => v,
                        Err(e) => {
                            return Err(SystemError::InvalidRequest {
                                error: format!("Parsing query request: {}", e),
                                request: key.into(),
                            })
                        }
                    };

                    let balance = match balances.get(&address) {
                        Some(v) => v,
                        None => {
                            return Err(SystemError::InvalidRequest {
                                error: "Balance not found".to_string(),
                                request: key.into(),
                            })
                        }
                    };

                    Ok(to_binary(&to_binary(&balance).unwrap()))
                } else {
                    panic!("DO NOT ENTER HERE")
                }
            }
            _ => self.base.handle_query(request),
        }
    }
}

impl WasmMockQuerier {
    pub fn new<A: Api>(
        base: MockQuerier<TerraQueryWrapper>,
        _api: A,
        canonical_length: usize,
    ) -> Self {
        WasmMockQuerier {
            base,
            token_querier: TokenQuerier::default(),
            tax_querier: TaxQuerier::default(),
            oracle_price_querier: OraclePriceQuerier::default(),
            collateral_oracle_querier: CollateralOracleQuerier::default(),
            terraswap_pair_querier: TerraswapPairQuerier::default(),
            canonical_length,
        }
    }

    // configure the mint whitelist mock querier
    pub fn with_token_balances(&mut self, balances: &[(&HumanAddr, &[(&HumanAddr, &Uint128)])]) {
        self.token_querier = TokenQuerier::new(balances);
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
    pub fn with_terraswap_pair(&mut self, pairs: &[(&String, &String, &HumanAddr)]) {
        self.terraswap_pair_querier = TerraswapPairQuerier::new(pairs);
    }
}
