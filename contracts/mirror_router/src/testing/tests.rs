use cosmwasm_std::testing::{mock_env, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_binary, to_binary, BankMsg, Coin, CosmosMsg, Decimal, HumanAddr, StdError, Uint128,
    WasmMsg,
};

use crate::contract::{handle, init, query};
use crate::testing::mock_querier::mock_dependencies;

use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};
use mirror_protocol::mint::HandleMsg as MintHandleMsg;
use mirror_protocol::router::{
    BuyWithRoutesResponse, ConfigResponse, Cw20HookMsg, HandleMsg, InitMsg, QueryMsg,
};
use mirror_protocol::staking::Cw20HookMsg as StakingCw20HookMsg;
use terraswap::asset::{Asset, AssetInfo};
use terraswap::pair::HandleMsg as PairHandleMsg;

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        mint_contract: HumanAddr("mint".to_string()),
        oracle_contract: HumanAddr("oracle".to_string()),
        staking_contract: HumanAddr("staking".to_string()),
        terraswap_factory: HumanAddr("terraswapfactory".to_string()),
        base_denom: "uusd".to_string(),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    // it worked, let's query the state
    let config: ConfigResponse = from_binary(&query(&deps, QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!("mint", config.mint_contract.as_str());
    assert_eq!("oracle", config.oracle_contract.as_str());
    assert_eq!("staking", config.staking_contract.as_str());
    assert_eq!("terraswapfactory", config.terraswap_factory.as_str());
    assert_eq!("uusd", config.base_denom.as_str());
}

#[test]
fn execute_buy_operations() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        mint_contract: HumanAddr("mint".to_string()),
        oracle_contract: HumanAddr("oracle".to_string()),
        staking_contract: HumanAddr("staking".to_string()),
        terraswap_factory: HumanAddr("terraswapfactory".to_string()),
        base_denom: "uusd".to_string(),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    deps.querier.with_tax(
        Decimal::percent(5),
        &[(&"uusd".to_string(), &Uint128(1000000u128))],
    );
    deps.querier
        .with_terraswap_pairs(&[(&"uusdasset".to_string(), &HumanAddr::from("pair"))]);
    deps.querier.with_balance(&[(
        &HumanAddr::from("pair"),
        &[Coin {
            amount: Uint128(10000u128 * 1000000u128),
            denom: "uusd".to_string(),
        }],
    )]);

    let msg = HandleMsg::BuyAndStake {
        asset_token: HumanAddr::from("asset"),
        belief_price: None,
        max_spread: None,
    };

    // zero balance error
    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg.clone());
    match res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "Cannot execute operations with zero balance",)
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    let env = mock_env(
        "addr0000",
        &[Coin {
            amount: Uint128(1000000u128),
            denom: "uusd".to_string(),
        }],
    );

    // pre-deducted  tax 500000 - 500000 / (1 + 0.05) = 23,810 * 2 = 47,620
    // after deducted 1000000 - 47,620 = 952,380
    // sqrt(10000*1000000 * (10000*1000000 + 952,380)) - 10000*1000000 = 476,178
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("pair"),
                msg: to_binary(&PairHandleMsg::Swap {
                    offer_asset: Asset {
                        amount: Uint128(476178u128),
                        info: AssetInfo::NativeToken {
                            denom: "uusd".to_string(),
                        },
                    },
                    belief_price: None,
                    max_spread: None,
                    to: None,
                })
                .unwrap(),
                send: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128(476178u128),
                }],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                msg: to_binary(&HandleMsg::ProvideOperation {
                    asset_token: HumanAddr::from("asset"),
                    pair_contract: HumanAddr::from("pair"),
                })
                .unwrap(),
                send: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                msg: to_binary(&HandleMsg::StakeOperation {
                    asset_token: HumanAddr::from("asset"),
                    liquidity_token: HumanAddr::from("liquidity"),
                    staker: HumanAddr::from("addr0000"),
                })
                .unwrap(),
                send: vec![],
            })
        ]
    );
}

#[test]
fn execute_mint_operations() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        mint_contract: HumanAddr("mint".to_string()),
        oracle_contract: HumanAddr("oracle".to_string()),
        staking_contract: HumanAddr("staking".to_string()),
        terraswap_factory: HumanAddr("terraswapfactory".to_string()),
        base_denom: "uusd".to_string(),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    deps.querier.with_tax(
        Decimal::percent(5),
        &[(&"uusd".to_string(), &Uint128(1000000u128))],
    );
    deps.querier
        .with_terraswap_pairs(&[(&"uusdasset".to_string(), &HumanAddr::from("pair"))]);
    deps.querier.with_token_balances(&[(
        &HumanAddr::from("asset"),
        &[(&HumanAddr::from("pair"), &Uint128(10u128 * 1000000u128))],
    )]);
    deps.querier.with_balance(&[(
        &HumanAddr::from("pair"),
        &[Coin {
            amount: Uint128(10000u128 * 1000000u128),
            denom: "uusd".to_string(),
        }],
    )]);
    deps.querier.with_oracle_price(&[
        (&"asset".to_string(), &Decimal::from_ratio(990u128, 1u128)),
        (&"uusd".to_string(), &Decimal::one()),
    ]);

    let msg = HandleMsg::MintAndStake {
        asset_token: HumanAddr::from("asset"),
        collateral_ratio: Decimal::percent(150),
    };

    // zero balance error
    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg.clone());
    match res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "Cannot execute operations with zero balance",)
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    let mut env = mock_env(
        "addr0000",
        &[Coin {
            amount: Uint128(1000000u128),
            denom: "uusd".to_string(),
        }],
    );

    env.block.time = 1000u64;

    // pre-deducted  tax 500000 - 500000 / (1 + 0.05) = 23,810 * 2 = 47,620
    // after deducted 1000000 - 47,620 = 952,380
    // oracle price = 990
    // pair price = 1000
    // collateral_ratio = 150%
    // collateral_amount = 1.5 * 990 * 952,380 / (1.5 * 990 + 1000) = 569,128
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("mint"),
                msg: to_binary(&MintHandleMsg::OpenPosition {
                    owner: Some(HumanAddr::from("addr0000")),
                    collateral: Asset {
                        amount: Uint128(569128u128),
                        info: AssetInfo::NativeToken {
                            denom: "uusd".to_string(),
                        },
                    },
                    asset_info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset"),
                    },
                    collateral_ratio: Decimal::percent(150),
                })
                .unwrap(),
                send: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128(569128u128),
                }],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                msg: to_binary(&HandleMsg::ProvideOperation {
                    asset_token: HumanAddr::from("asset"),
                    pair_contract: HumanAddr::from("pair"),
                })
                .unwrap(),
                send: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                msg: to_binary(&HandleMsg::StakeOperation {
                    asset_token: HumanAddr::from("asset"),
                    liquidity_token: HumanAddr::from("liquidity"),
                    staker: HumanAddr::from("addr0000"),
                })
                .unwrap(),
                send: vec![],
            })
        ]
    );
}

#[test]
fn buy_with_routes() {
    let mut deps = mock_dependencies(20, &[]);
    let msg = InitMsg {
        mint_contract: HumanAddr("mint".to_string()),
        oracle_contract: HumanAddr("oracle".to_string()),
        staking_contract: HumanAddr("staking".to_string()),
        terraswap_factory: HumanAddr("terraswapfactory".to_string()),
        base_denom: "uusd".to_string(),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::BuyWithRoutes {
        offer_asset_info: AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
        routes: vec![],
        max_spread: None,
    };

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "must provide routes"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = HandleMsg::BuyWithRoutes {
        offer_asset_info: AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
        routes: vec![
            AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
            AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0001"),
            },
            AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0002"),
            },
            AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            },
        ],
        max_spread: Some(Decimal::one()),
    };

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                send: vec![],
                msg: to_binary(&HandleMsg::BuyOperation {
                    offer_asset_info: AssetInfo::NativeToken {
                        denom: "uusd".to_string(),
                    },
                    ask_asset_info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0000")
                    },
                    max_spread: Some(Decimal::one()),
                    to: None,
                })
                .unwrap(),
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                send: vec![],
                msg: to_binary(&HandleMsg::BuyOperation {
                    offer_asset_info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0000")
                    },
                    ask_asset_info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0001")
                    },
                    max_spread: Some(Decimal::one()),
                    to: None,
                })
                .unwrap(),
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                send: vec![],
                msg: to_binary(&HandleMsg::BuyOperation {
                    offer_asset_info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0001")
                    },
                    ask_asset_info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0002")
                    },
                    max_spread: Some(Decimal::one()),
                    to: None,
                })
                .unwrap(),
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                send: vec![],
                msg: to_binary(&HandleMsg::BuyOperation {
                    offer_asset_info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0002")
                    },
                    ask_asset_info: AssetInfo::NativeToken {
                        denom: "ukrw".to_string()
                    },
                    max_spread: Some(Decimal::one()),
                    to: Some(HumanAddr::from("addr0000")),
                })
                .unwrap(),
            })
        ]
    );

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128::from(1000000u128),
        msg: Some(
            to_binary(&Cw20HookMsg::BuyWithRoutes {
                routes: vec![
                    AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0001"),
                    },
                    AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0002"),
                    },
                    AssetInfo::NativeToken {
                        denom: "ukrw".to_string(),
                    },
                ],
                max_spread: Some(Decimal::one()),
            })
            .unwrap(),
        ),
    });

    let env = mock_env("asset0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                send: vec![],
                msg: to_binary(&HandleMsg::BuyOperation {
                    offer_asset_info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0000")
                    },
                    ask_asset_info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0001")
                    },
                    max_spread: Some(Decimal::one()),
                    to: None,
                })
                .unwrap(),
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                send: vec![],
                msg: to_binary(&HandleMsg::BuyOperation {
                    offer_asset_info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0001")
                    },
                    ask_asset_info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0002")
                    },
                    max_spread: Some(Decimal::one()),
                    to: None,
                })
                .unwrap(),
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                send: vec![],
                msg: to_binary(&HandleMsg::BuyOperation {
                    offer_asset_info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0002")
                    },
                    ask_asset_info: AssetInfo::NativeToken {
                        denom: "ukrw".to_string()
                    },
                    max_spread: Some(Decimal::one()),
                    to: Some(HumanAddr::from("addr0000")),
                })
                .unwrap(),
            })
        ]
    );
}

#[test]
fn buy_operation() {
    let mut deps = mock_dependencies(20, &[]);
    let msg = InitMsg {
        mint_contract: HumanAddr("mint".to_string()),
        oracle_contract: HumanAddr("oracle".to_string()),
        staking_contract: HumanAddr("staking".to_string()),
        terraswap_factory: HumanAddr("terraswapfactory".to_string()),
        base_denom: "uusd".to_string(),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    deps.querier
        .with_terraswap_pairs(&[(&"uusdasset".to_string(), &HumanAddr::from("pair"))]);
    deps.querier.with_tax(
        Decimal::percent(5),
        &[(&"uusd".to_string(), &Uint128(1000000u128))],
    );
    deps.querier.with_balance(&[(
        &HumanAddr::from(MOCK_CONTRACT_ADDR),
        &[Coin {
            amount: Uint128(1000000u128),
            denom: "uusd".to_string(),
        }],
    )]);

    let msg = HandleMsg::BuyOperation {
        offer_asset_info: AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
        ask_asset_info: AssetInfo::Token {
            contract_addr: HumanAddr::from("asset"),
        },
        max_spread: Some(Decimal::one()),
        to: Some(HumanAddr::from("addr0000")),
    };
    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg.clone());
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("DO NOT ENTER HERE"),
    }

    let env = mock_env(MOCK_CONTRACT_ADDR, &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("pair"),
            send: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128(952380u128), // 1000000 / (1 + 0.05)
            }],
            msg: to_binary(&PairHandleMsg::Swap {
                offer_asset: Asset {
                    info: AssetInfo::NativeToken {
                        denom: "uusd".to_string(),
                    },
                    amount: Uint128(952380u128),
                },
                belief_price: None,
                max_spread: Some(Decimal::one()),
                to: Some(HumanAddr::from("addr0000")),
            })
            .unwrap()
        })]
    );

    // buy uusd with asset
    deps.querier
        .with_terraswap_pairs(&[(&"assetuusd".to_string(), &HumanAddr::from("pair"))]);
    deps.querier.with_token_balances(&[(
        &HumanAddr::from("asset"),
        &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &Uint128(1000000u128))],
    )]);

    let msg = HandleMsg::BuyOperation {
        offer_asset_info: AssetInfo::Token {
            contract_addr: HumanAddr::from("asset"),
        },
        ask_asset_info: AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
        max_spread: Some(Decimal::one()),
        to: Some(HumanAddr::from("addr0000")),
    };

    let env = mock_env(MOCK_CONTRACT_ADDR, &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset"),
            send: vec![],
            msg: to_binary(&Cw20HandleMsg::Send {
                contract: HumanAddr::from("pair"),
                amount: Uint128(1000000u128),
                msg: Some(
                    to_binary(&PairHandleMsg::Swap {
                        offer_asset: Asset {
                            info: AssetInfo::Token {
                                contract_addr: HumanAddr::from("asset"),
                            },
                            amount: Uint128(1000000u128),
                        },
                        belief_price: None,
                        max_spread: Some(Decimal::one()),
                        to: Some(HumanAddr::from("addr0000")),
                    })
                    .unwrap()
                )
            })
            .unwrap()
        })]
    );
}

#[test]
fn provide_operation() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        mint_contract: HumanAddr("mint".to_string()),
        oracle_contract: HumanAddr("oracle".to_string()),
        staking_contract: HumanAddr("staking".to_string()),
        terraswap_factory: HumanAddr("terraswapfactory".to_string()),
        base_denom: "uusd".to_string(),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    // Case 1. after execute_buy_operations
    // swap_amount = 476,178
    // native_balance = 523,822
    // asset_balance = 476
    // native_balance_pair = 10000 * 1000000 + 476,178 = 10,000,476,178
    // asset_balance_pair = 10000000 - 476 = 9,999,524

    // pre-deducted  tax 523,822 - 523,822 / (1 + 0.05) = 24,944
    // after deducted 523,822 - 24,944 = 498,878
    // provide_native_amount = min(498,878, 476,045) = 476,045

    deps.querier.with_tax(
        Decimal::percent(5),
        &[(&"uusd".to_string(), &Uint128(1000000u128))],
    );
    deps.querier.with_token_balances(&[(
        &HumanAddr::from("asset"),
        &[
            (&HumanAddr::from(MOCK_CONTRACT_ADDR), &Uint128(476u128)),
            (
                &HumanAddr::from("pair"),
                &Uint128(10u128 * 1000000u128 - 476u128),
            ),
        ],
    )]);
    deps.querier.with_balance(&[
        (
            &HumanAddr::from(MOCK_CONTRACT_ADDR),
            &[Coin {
                amount: Uint128(1000000u128 - 476178u128),
                denom: "uusd".to_string(),
            }],
        ),
        (
            &HumanAddr::from("pair"),
            &[Coin {
                amount: Uint128(10000u128 * 1000000u128 + 476178u128),
                denom: "uusd".to_string(),
            }],
        ),
    ]);

    let msg = HandleMsg::ProvideOperation {
        asset_token: HumanAddr::from("asset"),
        pair_contract: HumanAddr::from("pair"),
    };

    let env = mock_env(MOCK_CONTRACT_ADDR, &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("asset"),
                msg: to_binary(&Cw20HandleMsg::IncreaseAllowance {
                    spender: HumanAddr::from("pair"),
                    amount: Uint128(476u128),
                    expires: None,
                })
                .unwrap(),
                send: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("pair"),
                msg: to_binary(&PairHandleMsg::ProvideLiquidity {
                    assets: [
                        Asset {
                            info: AssetInfo::NativeToken {
                                denom: "uusd".to_string(),
                            },
                            amount: Uint128(476045u128),
                        },
                        Asset {
                            info: AssetInfo::Token {
                                contract_addr: HumanAddr::from("asset"),
                            },
                            amount: Uint128(476u128),
                        },
                    ],
                    slippage_tolerance: Some(Decimal::percent(1)),
                })
                .unwrap(),
                send: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128(476045u128),
                }],
            }),
        ]
    );

    // Case 2. after execute_mint_operations
    // collateral_amount = 569,128
    // native_balance = 430,872
    // asset_balance = 383
    // native_balance_pair = 10000 * 1000000
    // asset_balance_pair = 10000000

    // pre-deducted  tax 430,872 - 430,872 / (1 + 0.05) = 20,517
    // after deducted 430,872 - 20,517 = 410,355
    // provide_native_amount = min(410,355, 383,000) = 383,000

    deps.querier.with_tax(
        Decimal::percent(5),
        &[(&"uusd".to_string(), &Uint128(1000000u128))],
    );
    deps.querier.with_token_balances(&[(
        &HumanAddr::from("asset"),
        &[
            (&HumanAddr::from(MOCK_CONTRACT_ADDR), &Uint128(383u128)),
            (&HumanAddr::from("pair"), &Uint128(10u128 * 1000000u128)),
        ],
    )]);
    deps.querier.with_balance(&[
        (
            &HumanAddr::from(MOCK_CONTRACT_ADDR),
            &[Coin {
                amount: Uint128(1000000u128 - 569128u128),
                denom: "uusd".to_string(),
            }],
        ),
        (
            &HumanAddr::from("pair"),
            &[Coin {
                amount: Uint128(10000u128 * 1000000u128),
                denom: "uusd".to_string(),
            }],
        ),
    ]);

    let msg = HandleMsg::ProvideOperation {
        asset_token: HumanAddr::from("asset"),
        pair_contract: HumanAddr::from("pair"),
    };

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg.clone());
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("DO NOT ENTER HERE"),
    }

    let env = mock_env(MOCK_CONTRACT_ADDR, &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("asset"),
                msg: to_binary(&Cw20HandleMsg::IncreaseAllowance {
                    spender: HumanAddr::from("pair"),
                    amount: Uint128(383u128),
                    expires: None,
                })
                .unwrap(),
                send: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("pair"),
                msg: to_binary(&PairHandleMsg::ProvideLiquidity {
                    assets: [
                        Asset {
                            info: AssetInfo::NativeToken {
                                denom: "uusd".to_string(),
                            },
                            amount: Uint128(383000u128),
                        },
                        Asset {
                            info: AssetInfo::Token {
                                contract_addr: HumanAddr::from("asset"),
                            },
                            amount: Uint128(383u128),
                        },
                    ],
                    slippage_tolerance: Some(Decimal::percent(1)),
                })
                .unwrap(),
                send: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128(383000u128),
                }],
            }),
        ]
    );
}

#[test]
fn stake_operation() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        mint_contract: HumanAddr("mint".to_string()),
        oracle_contract: HumanAddr("oracle".to_string()),
        staking_contract: HumanAddr("staking".to_string()),
        terraswap_factory: HumanAddr("terraswapfactory".to_string()),
        base_denom: "uusd".to_string(),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    deps.querier.with_tax(
        Decimal::percent(5),
        &[(&"uusd".to_string(), &Uint128(1000000u128))],
    );
    deps.querier.with_token_balances(&[(
        &HumanAddr::from("liquidity"),
        &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &Uint128(100u128))],
    )]);
    deps.querier.with_balance(&[(
        &HumanAddr::from(MOCK_CONTRACT_ADDR),
        &[Coin {
            amount: Uint128(200u128),
            denom: "uusd".to_string(),
        }],
    )]);

    let msg = HandleMsg::StakeOperation {
        asset_token: HumanAddr::from("asset"),
        liquidity_token: HumanAddr::from("liquidity"),
        staker: HumanAddr::from("addr0000"),
    };

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg.clone());
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("DO NOT ENTER HERE"),
    }

    let env = mock_env(MOCK_CONTRACT_ADDR, &[]);
    let res = handle(&mut deps, env, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Bank(BankMsg::Send {
                from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
                to_address: HumanAddr::from("addr0000"),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128(190u128),
                }],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("liquidity"),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Send {
                    contract: HumanAddr::from("staking"),
                    amount: Uint128(100u128),
                    msg: Some(
                        to_binary(&StakingCw20HookMsg::Bond {
                            asset_token: HumanAddr::from("asset"),
                            staker: Some(HumanAddr::from("addr0000")),
                        })
                        .unwrap()
                    ),
                })
                .unwrap(),
            })
        ]
    );
}

#[test]
fn query_buy_with_routes() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        mint_contract: HumanAddr("mint".to_string()),
        oracle_contract: HumanAddr("oracle".to_string()),
        staking_contract: HumanAddr("staking".to_string()),
        terraswap_factory: HumanAddr("terraswapfactory".to_string()),
        base_denom: "uusd".to_string(),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    // set tax rate as 5%
    deps.querier.with_tax(
        Decimal::percent(5),
        &[
            (&"uusd".to_string(), &Uint128(1000000u128)),
            (&"ukrw".to_string(), &Uint128(1000000u128)),
        ],
    );

    let msg = QueryMsg::BuyWithRoutes {
        offer_asset: Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128::from(1000000u128),
        },
        routes: vec![
            AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
            AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            },
            AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0001"),
            },
        ],
    };

    deps.querier.with_terraswap_pairs(&[
        (&"uusdasset0000".to_string(), &HumanAddr::from("pair0000")),
        (&"asset0000ukrw".to_string(), &HumanAddr::from("pair0001")),
        (&"ukrwasset0001".to_string(), &HumanAddr::from("pair0002")),
    ]);

    let res: BuyWithRoutesResponse = from_binary(&query(&deps, msg).unwrap()).unwrap();
    assert_eq!(
        res,
        BuyWithRoutesResponse {
            amount: Uint128::from(863836u128), // tax charged 3 times uusd => asset0000, asset0000 => ukrw, ukrw => asset0001
        }
    );
}
