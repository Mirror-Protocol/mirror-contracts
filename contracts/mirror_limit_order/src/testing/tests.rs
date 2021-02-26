use cosmwasm_std::testing::{mock_env, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_binary, log, to_binary, BankMsg, Coin, CosmosMsg, Decimal, HumanAddr, StdError, Uint128,
    WasmMsg,
};

use crate::contract::{handle, init, query};
use crate::testing::mock_querier::mock_dependencies;

use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};
use mirror_protocol::common::OrderBy;
use mirror_protocol::limit_order::{
    Cw20HookMsg, HandleMsg, InitMsg, LastOrderIDResponse, OrderResponse, OrdersResponse, QueryMsg,
};
use terraswap::asset::{Asset, AssetInfo};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {};
    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();
}

#[test]
fn submit_order() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {};
    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::SubmitOrder {
        offer_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::Token {
                contract_addr: HumanAddr::from("mAAPL"),
            },
        },
        ask_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::Token {
                contract_addr: HumanAddr::from("mAAPL"),
            },
        },
    };

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "must provide native token"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = HandleMsg::SubmitOrder {
        offer_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
        },
        ask_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::Token {
                contract_addr: HumanAddr::from("mAAPL"),
            },
        },
    };

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(
            msg,
            "Native token balance missmatch between the argument and the transferred"
        ),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = HandleMsg::SubmitOrder {
        offer_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
        },
        ask_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::Token {
                contract_addr: HumanAddr::from("mAAPL"),
            },
        },
    };

    let env = mock_env(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000u128),
        }],
    );
    let res = handle(&mut deps, env.clone(), msg).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "submit_order"),
            log("order_id", 1),
            log("bidder_addr", "addr0000"),
            log("offer_asset", "1000000uusd"),
            log("ask_asset", "1000000mAAPL"),
        ]
    );

    assert_eq!(
        from_binary::<LastOrderIDResponse>(&query(&deps, QueryMsg::LastOrderID {}).unwrap())
            .unwrap(),
        LastOrderIDResponse { last_order_id: 1 }
    );

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(1000000u128),
        msg: Some(
            to_binary(&Cw20HookMsg::SubmitOrder {
                ask_asset: Asset {
                    amount: Uint128::from(1000000u128),
                    info: AssetInfo::NativeToken {
                        denom: "uusd".to_string(),
                    },
                },
            })
            .unwrap(),
        ),
    });

    let env = mock_env("mAAPL", &[]);
    let res = handle(&mut deps, env.clone(), msg).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "submit_order"),
            log("order_id", 2),
            log("bidder_addr", "addr0000"),
            log("offer_asset", "1000000mAAPL"),
            log("ask_asset", "1000000uusd"),
        ]
    );
    assert_eq!(
        from_binary::<LastOrderIDResponse>(&query(&deps, QueryMsg::LastOrderID {}).unwrap())
            .unwrap(),
        LastOrderIDResponse { last_order_id: 2 }
    );
}

#[test]
fn cancel_order_native_token() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_tax(
        Decimal::percent(1),
        &[(&"uusd".to_string(), &Uint128(1000000u128))],
    );

    let msg = InitMsg {};
    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::SubmitOrder {
        offer_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
        },
        ask_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::Token {
                contract_addr: HumanAddr::from("mAAPL"),
            },
        },
    };

    let env = mock_env(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000u128),
        }],
    );
    let _res = handle(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::CancelOrder { order_id: 1 };

    // failed verfication failed
    let wrong_env = mock_env("addr0001", &[]);
    let res = handle(&mut deps, wrong_env, msg.clone());
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("DO NOT ENTER HERE"),
    }

    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "cancel_order"),
            log("order_id", 1),
            log("bidder_refund", "1000000uusd")
        ]
    );
    assert_eq!(
        res.messages,
        vec![CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
            to_address: HumanAddr::from("addr0000"),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128(990099u128),
            }]
        })]
    );

    // failed no order exists
    let res = handle(&mut deps, env.clone(), msg.clone());
    assert_eq!(true, res.is_err());
}

#[test]
fn cancel_order_token() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_tax(
        Decimal::percent(1),
        &[(&"uusd".to_string(), &Uint128(1000000u128))],
    );

    let msg = InitMsg {};
    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(1000000u128),
        msg: Some(
            to_binary(&Cw20HookMsg::SubmitOrder {
                ask_asset: Asset {
                    amount: Uint128::from(1000000u128),
                    info: AssetInfo::NativeToken {
                        denom: "uusd".to_string(),
                    },
                },
            })
            .unwrap(),
        ),
    });

    let env = mock_env("mAAPL", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::CancelOrder { order_id: 1 };

    // failed verfication failed
    let wrong_env = mock_env("addr0001", &[]);
    let res = handle(&mut deps, wrong_env, msg.clone());
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("DO NOT ENTER HERE"),
    }

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "cancel_order"),
            log("order_id", 1),
            log("bidder_refund", "1000000mAAPL")
        ]
    );
    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("mAAPL"),
            send: vec![],
            msg: to_binary(&Cw20HandleMsg::Transfer {
                amount: Uint128(1000000u128),
                recipient: HumanAddr::from("addr0000"),
            })
            .unwrap(),
        })]
    );

    // failed no order exists
    let res = handle(&mut deps, env.clone(), msg.clone());
    assert_eq!(true, res.is_err());
}

#[test]
fn execute_order_native_token() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_tax(
        Decimal::percent(1),
        &[
            (&"uusd".to_string(), &Uint128(1000000u128)),
            (&"ukrw".to_string(), &Uint128(1000000u128)),
        ],
    );

    let msg = InitMsg {};
    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::SubmitOrder {
        offer_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
        },
        ask_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            },
        },
    };

    let env = mock_env(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000u128),
        }],
    );
    let _res = handle(&mut deps, env.clone(), msg).unwrap();

    // assertion; native asset balance
    let msg = HandleMsg::ExecuteOrder {
        execute_asset: Asset {
            amount: Uint128(500000u128),
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
        },
        order_id: 1u64,
    };
    let res = handle(&mut deps, env, msg.clone());
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(
            msg,
            "Native token balance missmatch between the argument and the transferred"
        ),
        _ => panic!("DO NOT ENTER HERE"),
    }

    // cannot execute order with other asset
    let env = mock_env(
        "addr0001",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(500000u128),
        }],
    );
    let res = handle(&mut deps, env.clone(), msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "invalid asset given"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    // partial execute
    let env = mock_env(
        "addr0001",
        &[Coin {
            denom: "ukrw".to_string(),
            amount: Uint128::from(500000u128),
        }],
    );
    let msg = HandleMsg::ExecuteOrder {
        execute_asset: Asset {
            amount: Uint128(500000u128),
            info: AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            },
        },
        order_id: 1u64,
    };
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "execute_order"),
            log("order_id", 1),
            log("executor_receive", "500000uusd"),
            log("bidder_receive", "500000ukrw"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Bank(BankMsg::Send {
                from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
                to_address: HumanAddr::from("addr0001"),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::from(495049u128)
                }]
            }),
            CosmosMsg::Bank(BankMsg::Send {
                from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
                to_address: HumanAddr::from("addr0000"),
                amount: vec![Coin {
                    denom: "ukrw".to_string(),
                    amount: Uint128::from(495049u128)
                }]
            }),
        ]
    );

    let resp: OrderResponse =
        from_binary(&query(&deps, QueryMsg::Order { order_id: 1 }).unwrap()).unwrap();
    assert_eq!(resp.filled_ask_amount, Uint128(500000u128));
    assert_eq!(resp.filled_offer_amount, Uint128(500000u128));

    // fill left amount
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "execute_order"),
            log("order_id", 1),
            log("executor_receive", "500000uusd"),
            log("bidder_receive", "500000ukrw"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Bank(BankMsg::Send {
                from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
                to_address: HumanAddr::from("addr0001"),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::from(495049u128)
                }]
            }),
            CosmosMsg::Bank(BankMsg::Send {
                from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
                to_address: HumanAddr::from("addr0000"),
                amount: vec![Coin {
                    denom: "ukrw".to_string(),
                    amount: Uint128::from(495049u128)
                }]
            }),
        ]
    );

    assert_eq!(true, query(&deps, QueryMsg::Order { order_id: 1 }).is_err());
}

#[test]
fn execute_order_token() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_tax(
        Decimal::percent(1),
        &[
            (&"uusd".to_string(), &Uint128(1000000u128)),
            (&"ukrw".to_string(), &Uint128(1000000u128)),
        ],
    );

    let msg = InitMsg {};
    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128::from(1000000u128),
        msg: Some(
            to_binary(&Cw20HookMsg::SubmitOrder {
                ask_asset: Asset {
                    amount: Uint128::from(1000000u128),
                    info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("token0001"),
                    },
                },
            })
            .unwrap(),
        ),
    });

    let env = mock_env("token0000", &[]);
    let _res = handle(&mut deps, env.clone(), msg).unwrap();

    // cannot execute order with other asset
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0001"),
        amount: Uint128::from(500000u128),
        msg: Some(to_binary(&Cw20HookMsg::ExecuteOrder { order_id: 1u64 }).unwrap()),
    });

    let env = mock_env("token0000", &[]);
    let res = handle(&mut deps, env, msg.clone());
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "invalid asset given"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    // partial execute
    let env = mock_env("token0001", &[]);
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "execute_order"),
            log("order_id", 1),
            log("executor_receive", "500000token0000"),
            log("bidder_receive", "500000token0001"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("token0000"),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Transfer {
                    recipient: HumanAddr::from("addr0001"),
                    amount: Uint128::from(500000u128)
                })
                .unwrap(),
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("token0001"),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Transfer {
                    recipient: HumanAddr::from("addr0000"),
                    amount: Uint128::from(500000u128)
                })
                .unwrap(),
            }),
        ]
    );

    let resp: OrderResponse =
        from_binary(&query(&deps, QueryMsg::Order { order_id: 1 }).unwrap()).unwrap();
    assert_eq!(resp.filled_ask_amount, Uint128(500000u128));
    assert_eq!(resp.filled_offer_amount, Uint128(500000u128));

    // fill left amount
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "execute_order"),
            log("order_id", 1),
            log("executor_receive", "500000token0000"),
            log("bidder_receive", "500000token0001"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("token0000"),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Transfer {
                    recipient: HumanAddr::from("addr0001"),
                    amount: Uint128::from(500000u128)
                })
                .unwrap(),
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("token0001"),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Transfer {
                    recipient: HumanAddr::from("addr0000"),
                    amount: Uint128::from(500000u128)
                })
                .unwrap(),
            }),
        ]
    );

    assert_eq!(true, query(&deps, QueryMsg::Order { order_id: 1 }).is_err());
}

#[test]
fn orders_querier() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_tax(
        Decimal::percent(1),
        &[
            (&"uusd".to_string(), &Uint128(1000000u128)),
            (&"ukrw".to_string(), &Uint128(1000000u128)),
        ],
    );

    let msg = InitMsg {};
    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::SubmitOrder {
        offer_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
        },
        ask_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            },
        },
    };

    let env = mock_env(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000u128),
        }],
    );
    let _res = handle(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128::from(1000000u128),
        msg: Some(
            to_binary(&Cw20HookMsg::SubmitOrder {
                ask_asset: Asset {
                    amount: Uint128::from(1000000u128),
                    info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("token0001"),
                    },
                },
            })
            .unwrap(),
        ),
    });

    let env = mock_env("token0000", &[]);
    let _res = handle(&mut deps, env.clone(), msg).unwrap();

    let order_1 = OrderResponse {
        order_id: 1u64,
        bidder_addr: HumanAddr::from("addr0000"),
        offer_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
        },
        ask_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            },
        },
        filled_offer_amount: Uint128::zero(),
        filled_ask_amount: Uint128::zero(),
    };

    let order_2 = OrderResponse {
        order_id: 2u64,
        bidder_addr: HumanAddr::from("addr0000"),
        offer_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::Token {
                contract_addr: HumanAddr::from("token0000"),
            },
        },
        ask_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::Token {
                contract_addr: HumanAddr::from("token0001"),
            },
        },
        filled_offer_amount: Uint128::zero(),
        filled_ask_amount: Uint128::zero(),
    };

    assert_eq!(
        OrdersResponse {
            orders: vec![order_1.clone(), order_2.clone(),],
        },
        from_binary::<OrdersResponse>(
            &query(
                &deps,
                QueryMsg::Orders {
                    bidder_addr: Some(HumanAddr::from("addr0000")),
                    start_after: None,
                    limit: None,
                    order_by: Some(OrderBy::Asc),
                }
            )
            .unwrap()
        )
        .unwrap()
    );

    assert_eq!(
        OrdersResponse {
            orders: vec![order_1.clone(), order_2.clone(),],
        },
        from_binary::<OrdersResponse>(
            &query(
                &deps,
                QueryMsg::Orders {
                    bidder_addr: None,
                    start_after: None,
                    limit: None,
                    order_by: Some(OrderBy::Asc),
                }
            )
            .unwrap()
        )
        .unwrap()
    );

    // DESC test
    assert_eq!(
        OrdersResponse {
            orders: vec![order_2.clone(), order_1.clone(),],
        },
        from_binary::<OrdersResponse>(
            &query(
                &deps,
                QueryMsg::Orders {
                    bidder_addr: None,
                    start_after: None,
                    limit: None,
                    order_by: Some(OrderBy::Desc),
                }
            )
            .unwrap()
        )
        .unwrap()
    );

    // different bidder
    assert_eq!(
        OrdersResponse { orders: vec![] },
        from_binary::<OrdersResponse>(
            &query(
                &deps,
                QueryMsg::Orders {
                    bidder_addr: Some(HumanAddr::from("addr0001")),
                    start_after: None,
                    limit: None,
                    order_by: None,
                }
            )
            .unwrap()
        )
        .unwrap()
    );

    // start after DESC
    assert_eq!(
        OrdersResponse {
            orders: vec![order_1.clone()],
        },
        from_binary::<OrdersResponse>(
            &query(
                &deps,
                QueryMsg::Orders {
                    bidder_addr: None,
                    start_after: Some(2u64),
                    limit: None,
                    order_by: Some(OrderBy::Desc),
                }
            )
            .unwrap()
        )
        .unwrap()
    );

    // start after ASC
    assert_eq!(
        OrdersResponse {
            orders: vec![order_2.clone()],
        },
        from_binary::<OrdersResponse>(
            &query(
                &deps,
                QueryMsg::Orders {
                    bidder_addr: None,
                    start_after: Some(1u64),
                    limit: None,
                    order_by: Some(OrderBy::Asc),
                }
            )
            .unwrap()
        )
        .unwrap()
    );
}
