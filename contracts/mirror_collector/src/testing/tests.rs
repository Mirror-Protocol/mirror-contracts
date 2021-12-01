use crate::contract::{execute, instantiate, query_config};
use crate::swap::MoneyMarketCw20HookMsg;
use crate::testing::mock_querier::mock_dependencies;
use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{to_binary, Coin, CosmosMsg, Decimal, SubMsg, Uint128, WasmMsg};
use cw20::Cw20ExecuteMsg;
use mirror_protocol::collector::{ConfigResponse, ExecuteMsg, InstantiateMsg};
use mirror_protocol::gov::Cw20HookMsg::DepositReward;
use terra_cosmwasm::{TerraMsg, TerraMsgWrapper, TerraRoute};
use terraswap::asset::{Asset, AssetInfo};
use terraswap::pair::{Cw20HookMsg as TerraswapCw20HookMsg, ExecuteMsg as TerraswapExecuteMsg};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        terraswap_factory: "terraswapfactory".to_string(),
        distribution_contract: "gov0000".to_string(),
        mirror_token: "mirror0000".to_string(),
        base_denom: "uusd".to_string(),
        aust_token: "aust0000".to_string(),
        anchor_market: "anchormarket0000".to_string(),
        bluna_token: "bluna0000".to_string(),
        bluna_swap_denom: "uluna".to_string(),
        mir_ust_pair: None,
    };

    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // it worked, let's query the state
    let config: ConfigResponse = query_config(deps.as_ref()).unwrap();
    assert_eq!("terraswapfactory", config.terraswap_factory.as_str());
    assert_eq!("uusd", config.base_denom.as_str());
}

#[test]
fn test_convert() {
    let mut deps = mock_dependencies(&[Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(100u128),
    }]);
    deps.querier.with_token_balances(&[(
        &"tokenAPPL".to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(100u128))],
    )]);

    deps.querier.with_tax(
        Decimal::percent(1),
        &[(&"uusd".to_string(), &Uint128::from(1000000u128))],
    );

    deps.querier.with_terraswap_pairs(&[
        (&"uusdtokenAPPL".to_string(), &"pairAPPL".to_string()),
        (&"uusdtokenMIRROR".to_string(), &"pairMIRROR".to_string()),
    ]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        terraswap_factory: "terraswapfactory".to_string(),
        distribution_contract: "gov0000".to_string(),
        mirror_token: "tokenMIRROR".to_string(),
        base_denom: "uusd".to_string(),
        aust_token: "aust0000".to_string(),
        anchor_market: "anchormarket0000".to_string(),
        bluna_token: "bluna0000".to_string(),
        bluna_swap_denom: "uluna".to_string(),
        mir_ust_pair: None,
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Convert {
        asset_token: "tokenAPPL".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "tokenAPPL".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "pairAPPL".to_string(),
                amount: Uint128::from(100u128),
                msg: to_binary(&TerraswapCw20HookMsg::Swap {
                    max_spread: None,
                    belief_price: None,
                    to: None,
                })
                .unwrap(),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    let msg = ExecuteMsg::Convert {
        asset_token: "tokenMIRROR".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // tax deduct 100 => 99
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "pairMIRROR".to_string(),
            msg: to_binary(&TerraswapExecuteMsg::Swap {
                offer_asset: Asset {
                    info: AssetInfo::NativeToken {
                        denom: "uusd".to_string()
                    },
                    amount: Uint128::from(99u128),
                },
                max_spread: None,
                belief_price: None,
                to: None,
            })
            .unwrap(),
            funds: vec![Coin {
                amount: Uint128::from(99u128),
                denom: "uusd".to_string(),
            }],
        }))]
    );
}

#[test]
fn test_convert_aust() {
    let mut deps = mock_dependencies(&[Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(100u128),
    }]);
    deps.querier.with_token_balances(&[(
        &"aust0000".to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(100u128))],
    )]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        terraswap_factory: "terraswapfactory".to_string(),
        distribution_contract: "gov0000".to_string(),
        mirror_token: "mirror0000".to_string(),
        base_denom: "uusd".to_string(),
        aust_token: "aust0000".to_string(),
        anchor_market: "anchormarket0000".to_string(),
        bluna_token: "bluna0000".to_string(),
        bluna_swap_denom: "uluna".to_string(),
        mir_ust_pair: None,
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Convert {
        asset_token: "aust0000".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "aust0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "anchormarket0000".to_string(),
                amount: Uint128::from(100u128),
                msg: to_binary(&MoneyMarketCw20HookMsg::RedeemStable {}).unwrap(),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );
}

#[test]
fn test_convert_bluna() {
    let mut deps = mock_dependencies(&[Coin {
        denom: "uluna".to_string(),
        amount: Uint128::from(100u128),
    }]);
    deps.querier.with_token_balances(&[(
        &"bluna0000".to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(100u128))],
    )]);

    deps.querier
        .with_terraswap_pairs(&[(&"ulunabluna0000".to_string(), &"pairbLuna".to_string())]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        terraswap_factory: "terraswapfactory".to_string(),
        distribution_contract: "gov0000".to_string(),
        mirror_token: "mirror0000".to_string(),
        base_denom: "uusd".to_string(),
        aust_token: "aust0000".to_string(),
        anchor_market: "anchormarket0000".to_string(),
        bluna_token: "bluna0000".to_string(),
        bluna_swap_denom: "uluna".to_string(),
        mir_ust_pair: None,
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Convert {
        asset_token: "bluna0000".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "bluna0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: "pairbLuna".to_string(),
                    amount: Uint128::from(100u128),
                    msg: to_binary(&TerraswapCw20HookMsg::Swap {
                        max_spread: None,
                        belief_price: None,
                        to: None,
                    })
                    .unwrap(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::LunaSwapHook {}).unwrap(),
                funds: vec![],
            })),
        ]
    );

    // suppose we sell the bluna for 100uluna
    let msg = ExecuteMsg::LunaSwapHook {};
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Custom(TerraMsgWrapper {
            route: TerraRoute::Market,
            msg_data: TerraMsg::Swap {
                offer_coin: Coin {
                    amount: Uint128::from(100u128),
                    denom: "uluna".to_string()
                },
                ask_denom: "uusd".to_string(),
            },
        }))],
    )
}

#[test]
fn test_send() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_token_balances(&[(
        &"mirror0000".to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(100u128))],
    )]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        terraswap_factory: "terraswapfactory".to_string(),
        distribution_contract: "gov0000".to_string(),
        mirror_token: "mirror0000".to_string(),
        base_denom: "uusd".to_string(),
        aust_token: "aust0000".to_string(),
        anchor_market: "anchormarket0000".to_string(),
        bluna_token: "bluna0000".to_string(),
        bluna_swap_denom: "uluna".to_string(),
        mir_ust_pair: None,
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    let msg = ExecuteMsg::Distribute {};

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "mirror0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "gov0000".to_string(),
                amount: Uint128::from(100u128),
                msg: to_binary(&DepositReward {}).unwrap(),
            })
            .unwrap(),
            funds: vec![],
        }))]
    )
}

#[test]
fn test_set_astroport_mir_pair() {
    let mut deps = mock_dependencies(&[Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(100u128),
    }]);

    deps.querier.with_tax(
        Decimal::percent(1),
        &[(&"uusd".to_string(), &Uint128::from(1000000u128))],
    );

    deps.querier
        .with_terraswap_pairs(&[(&"uusdtokenMIRROR".to_string(), &"pairMIRROR".to_string())]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        terraswap_factory: "terraswapfactory".to_string(),
        distribution_contract: "gov0000".to_string(),
        mirror_token: "tokenMIRROR".to_string(),
        base_denom: "uusd".to_string(),
        aust_token: "aust0000".to_string(),
        anchor_market: "anchormarket0000".to_string(),
        bluna_token: "bluna0000".to_string(),
        bluna_swap_denom: "uluna".to_string(),
        mir_ust_pair: None,
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // first, try to swap when the pair is not set yet
    let msg = ExecuteMsg::Convert {
        asset_token: "tokenMIRROR".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // tax deduct 100 => 99
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "pairMIRROR".to_string(), // terraswap pair
            msg: to_binary(&TerraswapExecuteMsg::Swap {
                offer_asset: Asset {
                    info: AssetInfo::NativeToken {
                        denom: "uusd".to_string()
                    },
                    amount: Uint128::from(99u128),
                },
                max_spread: None,
                belief_price: None,
                to: None,
            })
            .unwrap(),
            funds: vec![Coin {
                amount: Uint128::from(99u128),
                denom: "uusd".to_string(),
            }],
        }))]
    );

    // trigger the change by updating the configuration
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        terraswap_factory: None,
        distribution_contract: None,
        mirror_token: None,
        base_denom: None,
        aust_token: None,
        anchor_market: None,
        bluna_token: None,
        bluna_swap_denom: None,
        mir_ust_pair: Some("astroportPAIR".to_string()),
    };

    let info = mock_info("owner0000", &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // try again
    let msg = ExecuteMsg::Convert {
        asset_token: "tokenMIRROR".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // tax deduct 100 => 99
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "astroportPAIR".to_string(), // astroport pair
            msg: to_binary(&TerraswapExecuteMsg::Swap {
                // swap message format is same on astroport, will parse ok
                offer_asset: Asset {
                    info: AssetInfo::NativeToken {
                        denom: "uusd".to_string()
                    },
                    amount: Uint128::from(99u128),
                },
                max_spread: None,
                belief_price: None,
                to: None,
            })
            .unwrap(),
            funds: vec![Coin {
                amount: Uint128::from(99u128),
                denom: "uusd".to_string(),
            }],
        }))]
    );
}
