use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_slice, to_binary, Api, CanonicalAddr, Coin, Decimal, Extern, HumanAddr, Querier,
    QuerierResult, QueryRequest, SystemError, Uint128, WasmQuery,
};
use cosmwasm_storage::to_length_prefixed;
use std::collections::HashMap;

use crate::asset::{AssetInfoRaw, PairInfoRaw};
use crate::init::PairConfigRaw;
use cw20::TokenInfoResponse;
use terra_cosmwasm::{TaxCapResponse, TaxRateResponse, TerraQuery, TerraQueryWrapper, TerraRoute};

/// mock_dependencies is a drop-in replacement for cosmwasm_std::testing::mock_dependencies
/// this uses our CustomQuerier.
pub fn mock_dependencies(
    canonical_length: usize,
    contract_balance: &[Coin],
) -> Extern<MockStorage, MockApi, WasmMockQuerier> {
    let contract_addr = HumanAddr::from(MOCK_CONTRACT_ADDR);
    let custom_querier: WasmMockQuerier = WasmMockQuerier::new(
        MockQuerier::new(&[(&contract_addr, contract_balance)]),
        canonical_length,
        MockApi::new(canonical_length),
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
    terraswap_factory_querier: TerraSwapFactoryQuerier,
    terraswap_pair_querier: TerraSwapPairQuerier,
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
pub struct TerraSwapFactoryQuerier {
    pairs: HashMap<String, HumanAddr>,
}

impl TerraSwapFactoryQuerier {
    pub fn new(pairs: &[(&String, &HumanAddr)]) -> Self {
        TerraSwapFactoryQuerier {
            pairs: pairs_to_map(pairs),
        }
    }
}

pub(crate) fn pairs_to_map(pairs: &[(&String, &HumanAddr)]) -> HashMap<String, HumanAddr> {
    let mut pairs_map: HashMap<String, HumanAddr> = HashMap::new();
    for (key, pair) in pairs.iter() {
        pairs_map.insert(key.to_string(), HumanAddr::from(pair));
    }
    pairs_map
}

#[derive(Clone, Default)]
pub struct TerraSwapPairQuerier {
    liquidity_tokens: HashMap<HumanAddr, HumanAddr>,
}

impl TerraSwapPairQuerier {
    pub fn new(liquidity_tokens: &[(&HumanAddr, &HumanAddr)]) -> Self {
        TerraSwapPairQuerier {
            liquidity_tokens: liquidity_tokens_to_map(liquidity_tokens),
        }
    }
}

pub(crate) fn liquidity_tokens_to_map(
    liquidity_tokens: &[(&HumanAddr, &HumanAddr)],
) -> HashMap<HumanAddr, HumanAddr> {
    let mut liquidity_tokens_map: HashMap<HumanAddr, HumanAddr> = HashMap::new();
    for (contract_addr, lp_token) in liquidity_tokens.iter() {
        liquidity_tokens_map.insert(HumanAddr::from(contract_addr), HumanAddr::from(lp_token));
    }
    liquidity_tokens_map
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

impl WasmMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<TerraQueryWrapper>) -> QuerierResult {
        match &request {
            QueryRequest::Custom(TerraQueryWrapper { route, query_data }) => {
                if &TerraRoute::Treasury == route {
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

                let prefix_token_info = to_length_prefixed(b"token_info").to_vec();
                let prefix_balance = to_length_prefixed(b"balance").to_vec();
                let prefix_pair = to_length_prefixed(b"pair").to_vec();
                let prefix_config = to_length_prefixed(b"config").to_vec();

                if key.len() > prefix_config.len()
                    && key[..prefix_config.len()].to_vec() == prefix_config
                {
                    let item = match self
                        .terraswap_pair_querier
                        .liquidity_tokens
                        .get(contract_addr)
                    {
                        Some(v) => v,
                        None => {
                            return Err(SystemError::InvalidRequest {
                                error: format!(
                                    "Pair config is not found for {}",
                                    contract_addr.to_string()
                                ),
                                request: key.into(),
                            })
                        }
                    };

                    let api: MockApi = MockApi::new(self.canonical_length);
                    Ok(to_binary(
                        &to_binary(&PairConfigRaw {
                            owner: CanonicalAddr::default(),
                            contract_addr: CanonicalAddr::default(),
                            commission_collector: CanonicalAddr::default(),
                            liquidity_token: api.canonical_address(&item).unwrap(),
                        })
                        .unwrap(),
                    ))
                } else if key.len() > prefix_pair.len()
                    && key[..prefix_pair.len()].to_vec() == prefix_pair
                {
                    let rest_key: &[u8] = &key[prefix_pair.len()..];
                    let key_str: String = match String::from_utf8(rest_key.to_vec()) {
                        Ok(v) => v,
                        Err(e) => {
                            return Err(SystemError::InvalidRequest {
                                error: format!("Parsing query request: {}", e),
                                request: key.into(),
                            })
                        }
                    };

                    let pair_contract = match self.terraswap_factory_querier.pairs.get(&key_str) {
                        Some(v) => v,
                        None => {
                            return Err(SystemError::InvalidRequest {
                                error: format!("PairInfo is not found for {}", key_str),
                                request: key.into(),
                            })
                        }
                    };

                    let api: MockApi = MockApi::new(self.canonical_length);
                    Ok(to_binary(
                        &to_binary(&PairInfoRaw {
                            contract_addr: api.canonical_address(&pair_contract).unwrap(),
                            asset_infos: [
                                AssetInfoRaw::NativeToken {
                                    denom: "uusd".to_string(),
                                },
                                AssetInfoRaw::NativeToken {
                                    denom: "uusd".to_string(),
                                },
                            ],
                        })
                        .unwrap(),
                    ))
                } else {
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

                    if key.to_vec() == prefix_token_info {
                        let mut total_supply = Uint128::zero();

                        for balance in balances {
                            total_supply += *balance.1;
                        }

                        Ok(to_binary(
                            &to_binary(&TokenInfoResponse {
                                name: "mAPPL".to_string(),
                                symbol: "mAPPL".to_string(),
                                decimals: 6,
                                total_supply: total_supply,
                            })
                            .unwrap(),
                        ))
                    } else if key[..prefix_balance.len()].to_vec() == prefix_balance {
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
            }
            _ => self.base.handle_query(request),
        }
    }
}

impl WasmMockQuerier {
    pub fn new<A: Api>(
        base: MockQuerier<TerraQueryWrapper>,
        canonical_length: usize,
        _api: A,
    ) -> Self {
        WasmMockQuerier {
            base,
            token_querier: TokenQuerier::default(),
            tax_querier: TaxQuerier::default(),
            terraswap_pair_querier: TerraSwapPairQuerier::default(),
            terraswap_factory_querier: TerraSwapFactoryQuerier::default(),
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

    // configure the terraswap pair
    pub fn with_terraswap_pairs(&mut self, pairs: &[(&String, &HumanAddr)]) {
        self.terraswap_factory_querier = TerraSwapFactoryQuerier::new(pairs);
    }

    // configure the lp token mock querier
    pub fn with_terraswap_pair_lp_token(&mut self, liquidity_tokens: &[(&HumanAddr, &HumanAddr)]) {
        self.terraswap_pair_querier = TerraSwapPairQuerier::new(liquidity_tokens);
    }

    // pub fn with_balance(&mut self, balances: &[(&HumanAddr, &[Coin])]) {
    //     for (addr, balance) in balances {
    //         self.base.update_balance(addr, balance.to_vec());
    //     }
    // }
}
