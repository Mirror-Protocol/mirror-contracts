use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_slice, to_binary, Api, CanonicalAddr, Coin, Empty, Extern, HumanAddr, Querier,
    QuerierResult, QueryRequest, SystemError, WasmQuery,
};
use cosmwasm_storage::to_length_prefixed;

use std::collections::HashMap;
use terraswap::{AssetInfoRaw, PairConfigRaw, PairInfoRaw};

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
    terraswap_factory_querier: TerraswapFactoryQuerier,
    terraswap_pair_querier: TerraswapPairQuerier,
    canonical_length: usize,
}

#[derive(Clone, Default)]
pub struct TerraswapFactoryQuerier {
    pairs: HashMap<String, HumanAddr>,
}

impl TerraswapFactoryQuerier {
    pub fn new(pairs: &[(&String, &HumanAddr)]) -> Self {
        TerraswapFactoryQuerier {
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
pub struct TerraswapPairQuerier {
    staking_tokens: HashMap<HumanAddr, HumanAddr>,
}

impl TerraswapPairQuerier {
    pub fn new(staking_tokens: &[(&HumanAddr, &HumanAddr)]) -> Self {
        TerraswapPairQuerier {
            staking_tokens: staking_tokens_to_map(staking_tokens),
        }
    }
}

pub(crate) fn staking_tokens_to_map(
    staking_tokens: &[(&HumanAddr, &HumanAddr)],
) -> HashMap<HumanAddr, HumanAddr> {
    let mut staking_tokens_map: HashMap<HumanAddr, HumanAddr> = HashMap::new();
    for (contract_addr, staking_token) in staking_tokens.iter() {
        staking_tokens_map.insert(
            HumanAddr::from(contract_addr),
            HumanAddr::from(staking_token),
        );
    }
    staking_tokens_map
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

impl WasmMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
        match &request {
            QueryRequest::Wasm(WasmQuery::Raw { contract_addr, key }) => {
                let key: &[u8] = key.as_slice();
                let prefix_pair = to_length_prefixed(b"pair").to_vec();
                let prefix_config = to_length_prefixed(b"config").to_vec();

                if key.len() > prefix_config.len()
                    && key[..prefix_config.len()].to_vec() == prefix_config
                {
                    let item = match self
                        .terraswap_pair_querier
                        .staking_tokens
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
                    panic!("DO NOT ENTER HERE")
                }
            }
            _ => self.base.handle_query(request),
        }
    }
}

impl WasmMockQuerier {
    pub fn new<A: Api>(base: MockQuerier<Empty>, _api: A, canonical_length: usize) -> Self {
        WasmMockQuerier {
            base,
            terraswap_factory_querier: TerraswapFactoryQuerier::default(),
            terraswap_pair_querier: TerraswapPairQuerier::default(),
            canonical_length,
        }
    }

    // configure the terraswap pair
    pub fn with_terraswap_pairs(&mut self, pairs: &[(&String, &HumanAddr)]) {
        self.terraswap_factory_querier = TerraswapFactoryQuerier::new(pairs);
    }

    // configure the staking token mock querier
    pub fn with_terraswap_pair_staking_token(
        &mut self,
        staking_tokens: &[(&HumanAddr, &HumanAddr)],
    ) {
        self.terraswap_pair_querier = TerraswapPairQuerier::new(staking_tokens);
    }
}
