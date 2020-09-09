use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_slice, to_binary, Api, CanonicalAddr, Coin, Decimal, Empty, Extern, HumanAddr, Querier,
    QuerierResult, QueryRequest, StdError, SystemError, Uint128, WasmQuery,
};
use cosmwasm_storage::to_length_prefixed;

use std::collections::HashMap;

use crate::querier::WhitelistInfo;
use cw20::TokenInfoResponse;

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
    token_querier: TokenQuerier,
    whitelist_querier: WhitelistQuerier,
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

#[derive(Clone, Debug)]
pub struct WhitelistItem {
    pub token_contract: HumanAddr,
    pub market_contract: HumanAddr,
    pub staking_contract: HumanAddr,
}

#[derive(Clone, Default)]
pub struct WhitelistQuerier {
    // this lets us iterate over all pairs that match the first string
    whitelist: HashMap<String, WhitelistItem>,
}

impl WhitelistQuerier {
    pub fn new(whitelist: &[(&String, &WhitelistItem)]) -> Self {
        WhitelistQuerier {
            whitelist: whitelist_to_map(whitelist),
        }
    }
}

pub(crate) fn whitelist_to_map(
    whitelist: &[(&String, &WhitelistItem)],
) -> HashMap<String, WhitelistItem> {
    let mut whitelist_map: HashMap<String, WhitelistItem> = HashMap::new();
    for (symbol, item) in whitelist.iter() {
        whitelist_map.insert(symbol.to_string(), (*item).clone());
    }

    whitelist_map
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
                let prefix_config = to_length_prefixed(b"config").to_vec();
                let prefix_balance = to_length_prefixed(b"balances").to_vec();
                let prefix_whitelist = to_length_prefixed(b"whitelist").to_vec();

                if key.len() > prefix_whitelist.len()
                    && key[..prefix_whitelist.len()].to_vec() == prefix_whitelist
                {
                    let key_symbol: &[u8] = &key[prefix_whitelist.len()..];
                    let symbol: String = match String::from_utf8(key_symbol.to_vec()) {
                        Ok(v) => v,
                        Err(e) => {
                            return Err(SystemError::InvalidRequest {
                                error: format!("Parsing query request: {}", e),
                                request: key.into(),
                            })
                        }
                    };
                    let whitelist_item = match self.whitelist_querier.whitelist.get(&symbol) {
                        Some(whitelist_item) => whitelist_item,
                        None => {
                            return Ok(Err(StdError::generic_err(format!(
                                "No whitelist info registered for {}",
                                contract_addr,
                            ))))
                        }
                    };

                    let api: MockApi = MockApi::new(self.canonical_length);
                    Ok(to_binary(&WhitelistInfo {
                        mint_contract: CanonicalAddr::default(),
                        market_contract: api
                            .canonical_address(&whitelist_item.market_contract)
                            .unwrap(),
                        oracle_contract: CanonicalAddr::default(),
                        token_contract: api
                            .canonical_address(&whitelist_item.token_contract)
                            .unwrap(),
                        staking_contract: api
                            .canonical_address(&whitelist_item.staking_contract)
                            .unwrap(),
                        weight: Decimal::zero(),
                    }))
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

                    if key[..prefix_config.len()].to_vec() == prefix_config {
                        let key_total_supply: &[u8] = &key[prefix_config.len()..];
                        if key_total_supply == b"total_supply" {
                            let mut total_supply = Uint128::zero();

                            for balance in balances {
                                total_supply += *balance.1;
                            }

                            Ok(to_binary(&TokenInfoResponse {
                                name: "mAPPL".to_string(),
                                symbol: "mAPPL".to_string(),
                                decimals: 6,
                                total_supply: total_supply,
                            }))
                        } else {
                            panic!("DO NOT ENTER HERE")
                        }
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

                        Ok(to_binary(&balance))
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
    pub fn new<A: Api>(base: MockQuerier<Empty>, _api: A, canonical_length: usize) -> Self {
        WasmMockQuerier {
            base,
            token_querier: TokenQuerier::default(),
            whitelist_querier: WhitelistQuerier::default(),
            canonical_length,
        }
    }

    // configure the mint whitelist mock querier
    pub fn with_token_balances(&mut self, balances: &[(&HumanAddr, &[(&HumanAddr, &Uint128)])]) {
        self.token_querier = TokenQuerier::new(balances);
    }

    // configure the whitelist mock querier
    pub fn with_whitelist(&mut self, whitelist: &[(&String, &WhitelistItem)]) {
        self.whitelist_querier = WhitelistQuerier::new(whitelist);
    }
}
