use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_slice, to_binary, Api, CanonicalAddr, Coin, Decimal, Empty, Extern, HumanAddr, Querier,
    QuerierResult, QueryRequest, SystemError, WasmQuery,
};
use cosmwasm_storage::to_length_prefixed;

use crate::querier::{MintAssetConfig, OracleAssetConfig, PairConfigSwap};
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
    oracle_querier: OracleQuerier,
    mint_querier: MintQuerier,
    canonical_length: usize,
}

#[derive(Clone, Default)]
pub struct TerraswapFactoryQuerier {
    pairs: HashMap<String, (HumanAddr, HumanAddr)>,
}

impl TerraswapFactoryQuerier {
    pub fn new(pairs: &[(&String, (&HumanAddr, &HumanAddr))]) -> Self {
        TerraswapFactoryQuerier {
            pairs: pairs_to_map(pairs),
        }
    }
}

pub(crate) fn pairs_to_map(
    pairs: &[(&String, (&HumanAddr, &HumanAddr))],
) -> HashMap<String, (HumanAddr, HumanAddr)> {
    let mut pairs_map: HashMap<String, (HumanAddr, HumanAddr)> = HashMap::new();
    for (key, pair) in pairs.iter() {
        pairs_map.insert(
            key.to_string(),
            (HumanAddr::from(pair.0), HumanAddr::from(pair.1)),
        );
    }
    pairs_map
}

#[derive(Clone, Default)]
pub struct TerraswapPairQuerier {
    staking_tokens: HashMap<HumanAddr, HumanAddr>,
    configs: HashMap<HumanAddr, (Decimal, Decimal)>,
}

impl TerraswapPairQuerier {
    pub fn with_staking_tokens(
        mut self: Self,
        staking_tokens: &[(&HumanAddr, &HumanAddr)],
    ) -> Self {
        self.staking_tokens = address_pair_to_map(staking_tokens);
        self
    }

    pub fn with_configs(mut self: Self, configs: &[(&HumanAddr, &(Decimal, Decimal))]) -> Self {
        self.configs = configs_to_map(configs);
        self
    }
}

#[derive(Clone, Default)]
pub struct OracleQuerier {
    feeders: HashMap<HumanAddr, HumanAddr>,
}

impl OracleQuerier {
    pub fn new(feeders: &[(&HumanAddr, &HumanAddr)]) -> Self {
        OracleQuerier {
            feeders: address_pair_to_map(feeders),
        }
    }
}

pub(crate) fn address_pair_to_map(
    address_pair: &[(&HumanAddr, &HumanAddr)],
) -> HashMap<HumanAddr, HumanAddr> {
    let mut address_pair_map: HashMap<HumanAddr, HumanAddr> = HashMap::new();
    for (contract_addr, staking_token) in address_pair.iter() {
        address_pair_map.insert(
            HumanAddr::from(contract_addr),
            HumanAddr::from(staking_token),
        );
    }
    address_pair_map
}

#[derive(Clone, Default)]
pub struct MintQuerier {
    configs: HashMap<HumanAddr, (Decimal, Decimal)>,
}

impl MintQuerier {
    pub fn new(configs: &[(&HumanAddr, &(Decimal, Decimal))]) -> Self {
        MintQuerier {
            configs: configs_to_map(configs),
        }
    }
}

pub(crate) fn configs_to_map(
    configs: &[(&HumanAddr, &(Decimal, Decimal))],
) -> HashMap<HumanAddr, (Decimal, Decimal)> {
    let mut configs_map: HashMap<HumanAddr, (Decimal, Decimal)> = HashMap::new();
    for (contract_addr, pair) in configs.iter() {
        configs_map.insert(HumanAddr::from(contract_addr), (pair.0, pair.1));
    }
    configs_map
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
                let prefix_asset = to_length_prefixed(b"asset").to_vec();

                if key.len() > prefix_config.len()
                    && key[..prefix_config.len()].to_vec() == prefix_config
                {
                    if key[prefix_config.len()..].to_vec() == b"swap" {
                        let item = match self.terraswap_pair_querier.configs.get(contract_addr) {
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

                        Ok(to_binary(
                            &to_binary(&PairConfigSwap {
                                lp_commission: item.0,
                                owner_commission: item.1,
                            })
                            .unwrap(),
                        ))
                    } else {
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
                    }
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

                    let pair_info = match self.terraswap_factory_querier.pairs.get(&key_str) {
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
                            owner: api.canonical_address(&pair_info.0).unwrap(),
                            contract_addr: api.canonical_address(&pair_info.1).unwrap(),
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
                } else if key.len() > prefix_asset.len()
                    && key[..prefix_asset.len()].to_vec() == prefix_asset
                {
                    let api: MockApi = MockApi::new(self.canonical_length);
                    let rest_key: &[u8] = &key[prefix_asset.len()..];
                    let asset_token_raw: CanonicalAddr = CanonicalAddr::from(rest_key.to_vec());
                    let asset_token = api.human_address(&asset_token_raw).unwrap();

                    if contract_addr == &HumanAddr::from("oracle0000") {
                        let feeder = match self.oracle_querier.feeders.get(&asset_token) {
                            Some(v) => v,
                            None => {
                                return Err(SystemError::InvalidRequest {
                                    error: format!(
                                        "Oracle Feeder is not found for {}",
                                        asset_token
                                    ),
                                    request: key.into(),
                                })
                            }
                        };

                        Ok(to_binary(
                            &to_binary(&OracleAssetConfig {
                                asset_token: asset_token_raw,
                                feeder: api.canonical_address(&feeder).unwrap(),
                            })
                            .unwrap(),
                        ))
                    } else {
                        let config = match self.mint_querier.configs.get(&asset_token) {
                            Some(v) => v,
                            None => {
                                return Err(SystemError::InvalidRequest {
                                    error: format!("Mint Config is not found for {}", asset_token),
                                    request: key.into(),
                                })
                            }
                        };

                        Ok(to_binary(
                            &to_binary(&MintAssetConfig {
                                token: asset_token_raw,
                                auction_discount: config.0,
                                min_collateral_ratio: config.1,
                            })
                            .unwrap(),
                        ))
                    }
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
            mint_querier: MintQuerier::default(),
            oracle_querier: OracleQuerier::default(),
            canonical_length,
        }
    }

    // configure the terraswap pair
    pub fn with_terraswap_pairs(&mut self, pairs: &[(&String, (&HumanAddr, &HumanAddr))]) {
        self.terraswap_factory_querier = TerraswapFactoryQuerier::new(pairs);
    }

    // configure the staking token mock querier
    pub fn with_terraswap_pair_staking_token(
        &mut self,
        staking_tokens: &[(&HumanAddr, &HumanAddr)],
    ) {
        self.terraswap_pair_querier = self
            .terraswap_pair_querier
            .clone()
            .with_staking_tokens(staking_tokens);
    }

    // configure the pair contract config mock querier
    pub fn with_terraswap_pair_configs(&mut self, configs: &[(&HumanAddr, &(Decimal, Decimal))]) {
        self.terraswap_pair_querier = self.terraswap_pair_querier.clone().with_configs(configs);
    }

    pub fn with_oracle_feeders(&mut self, feeders: &[(&HumanAddr, &HumanAddr)]) {
        self.oracle_querier = OracleQuerier::new(feeders);
    }

    pub fn with_mint_configs(&mut self, configs: &[(&HumanAddr, &(Decimal, Decimal))]) {
        self.mint_querier = MintQuerier::new(configs);
    }
}
