use cosmwasm_std::{
    log, to_binary, BankMsg, BlockInfo, Coin, CosmosMsg, Decimal, Env, HandleResponse, HumanAddr,
    StdError, Uint128, WasmMsg,
};

use crate::contract::{
    deduct_tax, handle, init, query_config_asset, query_config_general, query_config_swap,
    query_pool, query_provider, query_reverse_simulation, query_simulation,
};

use crate::math::{decimal_multiplication, reverse_decimal};

use crate::msg::{
    ConfigAssetResponse, ConfigGeneralResponse, ConfigSwapResponse, HandleMsg, InitMsg,
    PoolResponse, ProviderResponse, ReverseSimulationResponse, SimulationResponse, SwapOperation,
};

use crate::mock_querier::mock_dependencies;
use cosmwasm_std::testing::{mock_env, MOCK_CONTRACT_ADDR};
use cw20::Cw20HandleMsg;

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        collateral_denom: "uusd".to_string(),
        liquidity_token: HumanAddr("liquidity0000".to_string()),
        commission_collector: HumanAddr("collector0000".to_string()),
        asset_symbol: "mAPPL".to_string(),
        asset_token: HumanAddr("asset0000".to_string()),
        asset_oracle: HumanAddr("oracle0000".to_string()),
        active_commission: Decimal::permille(3),
        inactive_commission: Decimal::permille(1),
        max_spread: Decimal::percent(20),
        max_minus_spread: Decimal::percent(2),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();
    // // it worked, let's query the state
    let config_general: ConfigGeneralResponse = query_config_general(&deps).unwrap();
    assert_eq!("addr0000", config_general.owner.as_str());
    assert_eq!(
        "collector0000",
        config_general.commission_collector.as_str()
    );
    assert_eq!("uusd", config_general.collateral_denom.as_str());
    assert_eq!("liquidity0000", config_general.liquidity_token.as_str());

    let config_asset: ConfigAssetResponse = query_config_asset(&deps).unwrap();
    assert_eq!("oracle0000", config_asset.oracle.as_str());
    assert_eq!("mAPPL", config_asset.symbol.as_str());
    assert_eq!("asset0000", config_asset.token.as_str());

    let config_swap: ConfigSwapResponse = query_config_swap(&deps).unwrap();
    assert_eq!(Decimal::permille(3), config_swap.active_commission);
    assert_eq!(Decimal::permille(1), config_swap.inactive_commission);
    assert_eq!(Decimal::percent(20), config_swap.max_spread);
    assert_eq!(Decimal::percent(2), config_swap.max_minus_spread);
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(20, &[]);
    let msg = InitMsg {
        collateral_denom: "uusd".to_string(),
        liquidity_token: HumanAddr("liquidity0000".to_string()),
        commission_collector: HumanAddr("collector0000".to_string()),
        asset_symbol: "mAPPL".to_string(),
        asset_token: HumanAddr("asset0000".to_string()),
        asset_oracle: HumanAddr("oracle0000".to_string()),
        active_commission: Decimal::permille(3),
        inactive_commission: Decimal::permille(1),
        max_spread: Decimal::percent(20),
        max_minus_spread: Decimal::percent(2),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    // update owner
    let env = mock_env("addr0000", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: Some(HumanAddr("addr0001".to_string())),
        active_commission: None,
        inactive_commission: None,
        max_minus_spread: None,
        max_spread: None,
    };

    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let config_general: ConfigGeneralResponse = query_config_general(&deps).unwrap();
    assert_eq!("addr0001", config_general.owner.as_str());

    // update left items
    let env = mock_env("addr0001", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        active_commission: Some(Decimal::percent(1)),
        inactive_commission: Some(Decimal::percent(2)),
        max_minus_spread: Some(Decimal::percent(5)),
        max_spread: Some(Decimal::percent(6)),
    };

    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let config_swap: ConfigSwapResponse = query_config_swap(&deps).unwrap();
    assert_eq!(Decimal::percent(1), config_swap.active_commission);
    assert_eq!(Decimal::percent(2), config_swap.inactive_commission);
    assert_eq!(Decimal::percent(5), config_swap.max_minus_spread);
    assert_eq!(Decimal::percent(6), config_swap.max_spread);

    // Unauthorzied err
    let env = mock_env("addr0000", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        active_commission: None,
        inactive_commission: None,
        max_minus_spread: None,
        max_spread: None,
    };

    let res = handle(&mut deps, env, msg);
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn provide_liquidity() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier
        .with_oracle_price(&[(&HumanAddr("oracle0000".to_string()), &Decimal::one())]);

    let msg = InitMsg {
        collateral_denom: "uusd".to_string(),
        liquidity_token: HumanAddr("liquidity0000".to_string()),
        commission_collector: HumanAddr("collector0000".to_string()),
        asset_symbol: "mAPPL".to_string(),
        asset_token: HumanAddr("asset0000".to_string()),
        asset_oracle: HumanAddr("oracle0000".to_string()),
        active_commission: Decimal::permille(3),
        inactive_commission: Decimal::permille(1),
        max_spread: Decimal::percent(20),
        max_minus_spread: Decimal::percent(2),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    // successfully provide liquidity for the exist pool
    let msg = HandleMsg::ProvideLiquidity {
        coins: vec![
            Coin {
                denom: "mAPPL".to_string(),
                amount: Uint128::from(100u128),
            },
            Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(100u128),
            },
        ],
    };

    let env = mock_env_with_block_time(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(100u128),
        }],
        1000,
    );
    let res = handle(&mut deps, env, msg).unwrap();
    let transfer_from_msg = res.messages.get(0).expect("no message");
    let mint_msg = res.messages.get(1).expect("no message");
    assert_eq!(
        transfer_from_msg,
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::TransferFrom {
                owner: HumanAddr::from("addr0000"),
                recipient: HumanAddr::from(MOCK_CONTRACT_ADDR),
                amount: Uint128::from(100u128),
            })
            .unwrap(),
            send: vec![],
        })
    );
    assert_eq!(
        mint_msg,
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("liquidity0000"),
            msg: to_binary(&Cw20HandleMsg::Mint {
                recipient: HumanAddr::from("addr0000"),
                amount: Uint128::from(100u128),
            })
            .unwrap(),
            send: vec![],
        })
    );

    let provider_response: ProviderResponse =
        query_provider(&deps, HumanAddr::from("addr0000")).unwrap();
    assert_eq!(100u128, provider_response.share.u128());

    // provide more liquidity not propotionally 1:1; 1:2,
    // then it must accept 1:1 and treat left amount as donation
    let msg = HandleMsg::ProvideLiquidity {
        coins: vec![
            Coin {
                denom: "mAPPL".to_string(),
                amount: Uint128::from(100u128),
            },
            Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(200u128),
            },
        ],
    };

    let env = mock_env_with_block_time(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(200u128),
        }],
        1000,
    );
    let res: HandleResponse = handle(&mut deps, env, msg).unwrap();
    let transfer_from_msg = res.messages.get(0).expect("no message");
    let mint_msg = res.messages.get(1).expect("no message");
    assert_eq!(
        transfer_from_msg,
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::TransferFrom {
                owner: HumanAddr::from("addr0000"),
                recipient: HumanAddr::from(MOCK_CONTRACT_ADDR),
                amount: Uint128::from(100u128),
            })
            .unwrap(),
            send: vec![],
        })
    );
    assert_eq!(
        mint_msg,
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("liquidity0000"),
            msg: to_binary(&Cw20HandleMsg::Mint {
                recipient: HumanAddr::from("addr0000"),
                amount: Uint128::from(100u128),
            })
            .unwrap(),
            send: vec![],
        })
    );

    let provider_response: ProviderResponse =
        query_provider(&deps, HumanAddr::from("addr0000")).unwrap();

    // only 100 share will be added due to inconsistent liquidity deposit
    assert_eq!(200u128, provider_response.share.u128());

    // current liquidity is 2:3; lets put more to make it 1:1
    // then no liquidity tokens will be issued
    let msg = HandleMsg::ProvideLiquidity {
        coins: vec![Coin {
            denom: "mAPPL".to_string(),
            amount: Uint128::from(100u128),
        }],
    };

    let env = mock_env_with_block_time("addr0001", &[], 1000);
    let _res = handle(&mut deps, env, msg).unwrap();

    let provider_response: ProviderResponse =
        query_provider(&deps, HumanAddr::from("addr0001")).unwrap();
    assert_eq!(0u128, provider_response.share.u128());

    // check wrong argument
    let msg = HandleMsg::ProvideLiquidity {
        coins: vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(50u128),
        }],
    };

    let env = mock_env(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(100u128),
        }],
    );
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(
            msg,
            "Collateral amount missmatch between the argument and the transferred".to_string()
        ),
        _ => panic!("Must return generic error"),
    }
}

#[test]
fn withdraw_liquidity() {
    let mut deps = mock_dependencies(
        20,
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128(100u128),
        }],
    );

    deps.querier.with_tax(
        Decimal::zero(),
        &[(&"uusd".to_string(), &Uint128::from(1000000u128))],
    );
    deps.querier.with_token_balances(&[
        (
            &HumanAddr::from("liquidity0000"),
            &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &Uint128(100u128))],
        ),
        (
            &HumanAddr::from("asset0000"),
            &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &Uint128(100u128))],
        ),
    ]);

    deps.querier
        .with_oracle_price(&[(&HumanAddr::from("oracle0000"), &Decimal::one())]);

    let msg = InitMsg {
        collateral_denom: "uusd".to_string(),
        liquidity_token: HumanAddr("liquidity0000".to_string()),
        commission_collector: HumanAddr("collector0000".to_string()),
        asset_symbol: "mAPPL".to_string(),
        asset_token: HumanAddr("asset0000".to_string()),
        asset_oracle: HumanAddr("oracle0000".to_string()),
        active_commission: Decimal::permille(3),
        inactive_commission: Decimal::permille(1),
        max_spread: Decimal::percent(20),
        max_minus_spread: Decimal::percent(2),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    // successfully provide liquidity for the exist pools (mAPPL:uusd = 2:1)
    let msg = HandleMsg::ProvideLiquidity {
        coins: vec![
            Coin {
                denom: "mAPPL".to_string(),
                amount: Uint128::from(100u128),
            },
            Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(100u128),
            },
        ],
    };

    let env = mock_env_with_block_time(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(100u128),
        }],
        1000,
    );
    let _res = handle(&mut deps, env, msg).unwrap();
    // received 100 shares

    // withdraw liquidity
    let msg = HandleMsg::WithdrawLiquidity {
        amount: Uint128(100u128),
    };

    let env = mock_env_with_block_time("addr0000", &[], 1000);
    let res = handle(&mut deps, env, msg).unwrap();
    let log_withdrawn_share = res.log.get(1).expect("no log");
    let log_refund_asset_amount = res.log.get(2).expect("no log");
    let log_refund_collateral_amount = res.log.get(3).expect("no log");
    let msg_asset_refund = res.messages.get(0).expect("no message");
    let msg_collateral_refund = res.messages.get(1).expect("no message");
    let msg_burn_liquidity = res.messages.get(2).expect("no message");
    assert_eq!(
        msg_asset_refund,
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: HumanAddr::from("addr0000"),
                amount: Uint128::from(100u128),
            })
            .unwrap(),
            send: vec![],
        })
    );
    assert_eq!(
        msg_collateral_refund,
        &CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
            to_address: HumanAddr::from("addr0000"),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(100u128),
            }],
        })
    );
    assert_eq!(
        msg_burn_liquidity,
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("liquidity0000"),
            msg: to_binary(&Cw20HandleMsg::BurnFrom {
                owner: HumanAddr::from("addr0000"),
                amount: Uint128::from(100u128),
            })
            .unwrap(),
            send: vec![],
        })
    );

    assert_eq!(
        log_withdrawn_share,
        &log("withdrawn_share", 100u128.to_string())
    );
    assert_eq!(
        log_refund_asset_amount,
        &log("refund_asset_amount", 100u128.to_string())
    );
    assert_eq!(
        log_refund_collateral_amount,
        &log("refund_collateral_amount", 100u128.to_string())
    );

    let provider_response: ProviderResponse =
        query_provider(&deps, HumanAddr::from("addr0000")).unwrap();
    assert_eq!(0u128, provider_response.share.u128());

    // can not withdraw liquidity over than provide amount
    let msg = HandleMsg::WithdrawLiquidity {
        amount: Uint128(50u128),
    };

    let env = mock_env_with_block_time("addr0000", &[], 1000);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "Can't withdraw more than you provided".to_string())
        }
        _ => panic!("Must return generic error"),
    }
}

#[test]
fn try_buy() {
    let total_share = Uint128(30000000000u128);
    let asset_pool_amount = Uint128(20000000000u128);
    let collateral_pool_amount = Uint128(30000000000u128);
    let price = Decimal::from_ratio(collateral_pool_amount, asset_pool_amount);
    let exchange_rate = reverse_decimal(price);

    let mut deps = mock_dependencies(
        20,
        &[Coin {
            denom: "uusd".to_string(),
            amount: collateral_pool_amount,
        }],
    );
    deps.querier.with_tax(
        Decimal::zero(),
        &[(&"uusd".to_string(), &Uint128::from(1000000u128))],
    );
    deps.querier.with_token_balances(&[
        (
            &HumanAddr::from("liquidity0000"),
            &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &total_share)],
        ),
        (
            &HumanAddr::from("asset0000"),
            &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &asset_pool_amount)],
        ),
    ]);

    deps.querier
        .with_oracle_price(&[(&HumanAddr::from("oracle0000"), &price)]);

    let msg = InitMsg {
        collateral_denom: "uusd".to_string(),
        liquidity_token: HumanAddr("liquidity0000".to_string()),
        commission_collector: HumanAddr("collector0000".to_string()),
        asset_symbol: "mAPPL".to_string(),
        asset_token: HumanAddr("asset0000".to_string()),
        asset_oracle: HumanAddr("oracle0000".to_string()),
        active_commission: Decimal::permille(2),
        inactive_commission: Decimal::permille(1),
        max_spread: Decimal::percent(20),
        max_minus_spread: Decimal::percent(2),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    // successfully provide liquidity for the exist pools (mAPPL:uusd = 2:1)
    let msg = HandleMsg::ProvideLiquidity {
        coins: vec![
            Coin {
                denom: "mAPPL".to_string(),
                amount: asset_pool_amount,
            },
            Coin {
                denom: "uusd".to_string(),
                amount: collateral_pool_amount,
            },
        ],
    };

    let env = mock_env_with_block_time(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: collateral_pool_amount,
        }],
        1000,
    );
    let _res = handle(&mut deps, env, msg).unwrap();

    // normal buy
    let msg = HandleMsg::Buy { max_spread: None };
    let offer_amount = Uint128(1500000000u128);
    let env = mock_env_with_block_time(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: offer_amount,
        }],
        1000,
    );

    let res = handle(&mut deps, env, msg).unwrap();
    let msg_transfer = res.messages.get(0).expect("no message");
    let msg_commission_transfer = res.messages.get(1).expect("no message");
    let log_return_amount = res.log.get(2).expect("no data");
    let log_spread_amount = res.log.get(3).expect("no data");
    let log_minus_spread_amount = res.log.get(4).expect("no data");
    let log_commission_amount = res.log.get(5).expect("no data");

    // current price is 1.5, so expected return without spread is 1000
    // 952.380953 = 20000 - 20000 * 30000 / (30000 + 1500)
    let expected_ret_amount = Uint128(952_380_953u128);
    let expected_spread_amount = (offer_amount * exchange_rate - expected_ret_amount).unwrap();
    let expected_minus_spread_amount = Uint128::zero();
    let expected_active_commission = expected_ret_amount.multiply_ratio(2u128, 1000u128); // 0.2%
    let expected_inactive_commission = expected_ret_amount.multiply_ratio(1u128, 1000u128); // 0.1%
    let expected_commission_amount = expected_active_commission + expected_inactive_commission;
    let expected_return_amount = (expected_ret_amount - expected_commission_amount).unwrap();

    // check simulation res
    let simulation_res: SimulationResponse =
        query_simulation(&deps, offer_amount, SwapOperation::Buy).unwrap();
    assert_eq!(expected_return_amount, simulation_res.return_amount.amount);
    assert_eq!(
        expected_commission_amount,
        simulation_res.commission_amount.amount
    );
    assert_eq!(expected_spread_amount, simulation_res.spread_amount.amount);
    assert_eq!(
        expected_minus_spread_amount,
        simulation_res.minus_spread_amount.amount
    );

    // check reverse simulation res
    let reverse_simulation_res: ReverseSimulationResponse =
        query_reverse_simulation(&deps, expected_return_amount, SwapOperation::Buy).unwrap();
    assert_eq!(
        (offer_amount.u128() as i128 - reverse_simulation_res.offer_amount.amount.u128() as i128)
            .abs()
            < 5i128,
        true
    );
    assert_eq!(
        (expected_commission_amount.u128() as i128
            - reverse_simulation_res.commission_amount.amount.u128() as i128)
            .abs()
            < 5i128,
        true
    );
    assert_eq!(
        (expected_spread_amount.u128() as i128
            - reverse_simulation_res.spread_amount.amount.u128() as i128)
            .abs()
            < 5i128,
        true
    );
    assert_eq!(
        Uint128::zero(),
        reverse_simulation_res.minus_spread_amount.amount
    );

    assert_eq!(
        &log(
            "return_amount",
            expected_return_amount.to_string() + "mAPPL"
        ),
        log_return_amount
    );
    assert_eq!(
        &log(
            "spread_amount",
            expected_spread_amount.to_string() + "mAPPL"
        ),
        log_spread_amount
    );
    assert_eq!(
        &log(
            "minus_spread_amount",
            expected_minus_spread_amount.to_string() + "mAPPL"
        ),
        log_minus_spread_amount
    );
    assert_eq!(
        &log(
            "commission_amount",
            expected_commission_amount.to_string() + "mAPPL"
        ),
        log_commission_amount
    );

    assert_eq!(
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: HumanAddr::from("addr0000"),
                amount: Uint128::from(expected_return_amount),
            })
            .unwrap(),
            send: vec![],
        }),
        msg_transfer,
    );

    assert_eq!(
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: HumanAddr::from("collector0000"),
                amount: Uint128::from(expected_inactive_commission),
            })
            .unwrap(),
            send: vec![],
        }),
        msg_commission_transfer,
    );

    // over max spread
    let msg = HandleMsg::Buy {
        max_spread: Some(Decimal::zero()),
    };
    let offer_amount = Uint128(10000000000u128);
    let env = mock_env_with_block_time(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: offer_amount,
        }],
        1000,
    );

    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "Operation exceeds max spread limit");
        }
        _ => panic!("Must return generic error"),
    };

    // hit max spread 20%
    let msg = HandleMsg::Buy { max_spread: None };
    let offer_amount = Uint128(10000000000u128);
    let env = mock_env_with_block_time(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: offer_amount,
        }],
        1000,
    );

    let res = handle(&mut deps, env, msg).unwrap();
    let msg_transfer = res.messages.get(0).expect("no message");
    let msg_commission_transfer = res.messages.get(1).expect("no message");
    let log_return_amount = res.log.get(2).expect("no data");
    let log_spread_amount = res.log.get(3).expect("no data");
    let log_minus_spread_amount = res.log.get(4).expect("no data");
    let log_commission_amount = res.log.get(5).expect("no data");

    let expected_ret_amount = Uint128(5_333_333_332u128);
    let expected_spread_amount = (offer_amount * exchange_rate - expected_ret_amount).unwrap();
    let expected_minus_spread_amount = Uint128::zero();
    let expected_active_commission = expected_ret_amount.multiply_ratio(2u128, 1000u128); // 0.2%
    let expected_inactive_commission = expected_ret_amount.multiply_ratio(1u128, 1000u128); // 0.1%
    let expected_commission_amount = expected_active_commission + expected_inactive_commission;
    let expected_return_amount = (expected_ret_amount - expected_commission_amount).unwrap();

    // check simulation res
    let simulation_res: SimulationResponse =
        query_simulation(&deps, offer_amount, SwapOperation::Buy).unwrap();
    assert_eq!(expected_return_amount, simulation_res.return_amount.amount);
    assert_eq!(
        expected_commission_amount,
        simulation_res.commission_amount.amount
    );
    assert_eq!(expected_spread_amount, simulation_res.spread_amount.amount);
    assert_eq!(
        expected_minus_spread_amount,
        simulation_res.minus_spread_amount.amount
    );

    // check reverse simulation res
    let reverse_simulation_res: ReverseSimulationResponse =
        query_reverse_simulation(&deps, expected_return_amount, SwapOperation::Buy).unwrap();
    assert_eq!(
        (offer_amount.u128() as i128 - reverse_simulation_res.offer_amount.amount.u128() as i128)
            .abs()
            < 50i128,
        true
    );
    assert_eq!(
        (expected_commission_amount.u128() as i128
            - reverse_simulation_res.commission_amount.amount.u128() as i128)
            .abs()
            < 50i128,
        true
    );
    assert_eq!(
        (expected_spread_amount.u128() as i128
            - reverse_simulation_res.spread_amount.amount.u128() as i128)
            .abs()
            < 50i128,
        true
    );
    assert_eq!(
        Uint128::zero(),
        reverse_simulation_res.minus_spread_amount.amount
    );

    assert_eq!(
        &log(
            "return_amount",
            expected_return_amount.to_string() + "mAPPL"
        ),
        log_return_amount
    );
    assert_eq!(
        &log(
            "spread_amount",
            expected_spread_amount.to_string() + "mAPPL"
        ),
        log_spread_amount
    );
    assert_eq!(
        &log(
            "minus_spread_amount",
            expected_minus_spread_amount.to_string() + "mAPPL"
        ),
        log_minus_spread_amount
    );
    assert_eq!(
        &log(
            "commission_amount",
            expected_commission_amount.to_string() + "mAPPL"
        ),
        log_commission_amount
    );

    assert_eq!(
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: HumanAddr::from("addr0000"),
                amount: Uint128::from(expected_return_amount),
            })
            .unwrap(),
            send: vec![],
        }),
        msg_transfer,
    );

    assert_eq!(
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: HumanAddr::from("collector0000"),
                amount: Uint128::from(expected_inactive_commission),
            })
            .unwrap(),
            send: vec![],
        }),
        msg_commission_transfer,
    );

    // hit max minus spread +2%
    let price = Decimal::from_ratio(2u128, 1u128);
    let exchange_rate = reverse_decimal(price);
    deps.querier
        .with_oracle_price(&[(&HumanAddr::from("oracle0000"), &price)]);

    let msg = HandleMsg::Buy { max_spread: None };
    let offer_amount = Uint128(100000000u128);
    let env = mock_env_with_block_time(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: offer_amount,
        }],
        1000,
    );

    let res = handle(&mut deps, env, msg).unwrap();
    let msg_transfer = res.messages.get(0).expect("no message");
    let msg_commission_transfer = res.messages.get(1).expect("no message");
    let log_return_amount = res.log.get(2).expect("no data");
    let log_spread_amount = res.log.get(3).expect("no data");
    let log_minus_spread_amount = res.log.get(4).expect("no data");
    let log_commission_amount = res.log.get(5).expect("no data");

    let expected_ret_amount = offer_amount * exchange_rate;
    let expected_minus_spread_amount = expected_ret_amount * Decimal::percent(2);
    let expected_ret_amount = expected_ret_amount + expected_minus_spread_amount;
    let expected_active_commission = expected_ret_amount.multiply_ratio(2u128, 1000u128); // 0.2%
    let expected_inactive_commission = expected_ret_amount.multiply_ratio(1u128, 1000u128); // 0.1%
    let expected_commission_amount = expected_active_commission + expected_inactive_commission;
    let expected_return_amount = (expected_ret_amount - expected_commission_amount).unwrap();

    // check simulation res
    let simulation_res: SimulationResponse =
        query_simulation(&deps, offer_amount, SwapOperation::Buy).unwrap();
    assert_eq!(expected_return_amount, simulation_res.return_amount.amount);
    assert_eq!(
        expected_commission_amount,
        simulation_res.commission_amount.amount
    );
    assert_eq!(Uint128::zero(), simulation_res.spread_amount.amount);
    assert_eq!(
        expected_minus_spread_amount,
        simulation_res.minus_spread_amount.amount
    );

    // check reverse simulation res
    let reverse_simulation_res: ReverseSimulationResponse =
        query_reverse_simulation(&deps, expected_return_amount, SwapOperation::Buy).unwrap();
    assert_eq!(
        (offer_amount.u128() as i128 - reverse_simulation_res.offer_amount.amount.u128() as i128)
            .abs()
            < 5i128,
        true
    );
    assert_eq!(
        (expected_commission_amount.u128() as i128
            - reverse_simulation_res.commission_amount.amount.u128() as i128)
            .abs()
            < 5i128,
        true
    );
    assert_eq!(Uint128::zero(), reverse_simulation_res.spread_amount.amount);
    assert_eq!(
        (expected_minus_spread_amount.u128() as i128
            - reverse_simulation_res.minus_spread_amount.amount.u128() as i128)
            .abs()
            < 5i128,
        true
    );

    // check logs
    assert_eq!(
        &log(
            "return_amount",
            expected_return_amount.to_string() + "mAPPL"
        ),
        log_return_amount
    );
    assert_eq!(&log("spread_amount", "0mAPPL"), log_spread_amount);
    assert_eq!(
        &log(
            "minus_spread_amount",
            expected_minus_spread_amount.to_string() + "mAPPL"
        ),
        log_minus_spread_amount
    );
    assert_eq!(
        &log(
            "commission_amount",
            expected_commission_amount.to_string() + "mAPPL"
        ),
        log_commission_amount
    );

    assert_eq!(
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: HumanAddr::from("addr0000"),
                amount: Uint128::from(expected_return_amount),
            })
            .unwrap(),
            send: vec![],
        }),
        msg_transfer,
    );

    assert_eq!(
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: HumanAddr::from("collector0000"),
                amount: Uint128::from(expected_inactive_commission),
            })
            .unwrap(),
            send: vec![],
        }),
        msg_commission_transfer,
    );
}

#[test]
fn try_sell() {
    let total_share = Uint128(20000000000u128);
    let asset_pool_amount = Uint128(30000000000u128);
    let collateral_pool_amount = Uint128(20000000000u128);
    let price = Decimal::from_ratio(collateral_pool_amount, asset_pool_amount);
    let exchange_rate = decimal_multiplication(price, Decimal::one());

    let mut deps = mock_dependencies(
        20,
        &[Coin {
            denom: "uusd".to_string(),
            amount: collateral_pool_amount,
        }],
    );
    deps.querier.with_tax(
        Decimal::zero(),
        &[(&"uusd".to_string(), &Uint128::from(1000000u128))],
    );
    deps.querier.with_token_balances(&[
        (
            &HumanAddr::from("liquidity0000"),
            &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &total_share)],
        ),
        (
            &HumanAddr::from("asset0000"),
            &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &asset_pool_amount)],
        ),
    ]);

    deps.querier
        .with_oracle_price(&[(&HumanAddr::from("oracle0000"), &price)]);

    let msg = InitMsg {
        collateral_denom: "uusd".to_string(),
        liquidity_token: HumanAddr("liquidity0000".to_string()),
        commission_collector: HumanAddr("collector0000".to_string()),
        asset_symbol: "mAPPL".to_string(),
        asset_token: HumanAddr("asset0000".to_string()),
        asset_oracle: HumanAddr("oracle0000".to_string()),
        active_commission: Decimal::permille(2),
        inactive_commission: Decimal::permille(1),
        max_spread: Decimal::percent(20),
        max_minus_spread: Decimal::percent(2),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    // successfully provide liquidity for the exist pools (mAPPL:uusd = 2:1)
    let msg = HandleMsg::ProvideLiquidity {
        coins: vec![
            Coin {
                denom: "mAPPL".to_string(),
                amount: asset_pool_amount,
            },
            Coin {
                denom: "uusd".to_string(),
                amount: collateral_pool_amount,
            },
        ],
    };

    let env = mock_env_with_block_time(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: collateral_pool_amount,
        }],
        1000,
    );
    let _res = handle(&mut deps, env, msg).unwrap();

    // normal sell
    let offer_amount = Uint128(1500000000u128);
    let msg = HandleMsg::Sell {
        amount: offer_amount,
        max_spread: None,
    };
    let env = mock_env_with_block_time("addr0000", &[], 1000);

    let res = handle(&mut deps, env, msg).unwrap();
    let msg_transfer_from = res.messages.get(0).expect("no message");
    let msg_transfer = res.messages.get(1).expect("no message");
    let msg_commission_transfer = res.messages.get(2).expect("no message");
    let log_return_amount = res.log.get(2).expect("no data");
    let log_spread_amount = res.log.get(3).expect("no data");
    let log_minus_spread_amount = res.log.get(4).expect("no data");
    let log_commission_amount = res.log.get(5).expect("no data");

    // current price is 1.5, so expected return without spread is 1000
    // 952.380953 = 20000 - 20000 * 30000 / (30000 + 1500)
    let expected_ret_amount = Uint128(952_380_953u128);
    let expected_spread_amount = (offer_amount * exchange_rate - expected_ret_amount).unwrap();
    let expected_minus_spread_amount = Uint128::zero();
    let expected_active_commission = expected_ret_amount.multiply_ratio(2u128, 1000u128); // 0.2%
    let expected_inactive_commission = expected_ret_amount.multiply_ratio(1u128, 1000u128); // 0.1%
    let expected_commission_amount = expected_active_commission + expected_inactive_commission;
    let expected_return_amount = (expected_ret_amount - expected_commission_amount).unwrap();

    // check simulation res
    let simulation_res: SimulationResponse =
        query_simulation(&deps, offer_amount, SwapOperation::Sell).unwrap();
    assert_eq!(expected_return_amount, simulation_res.return_amount.amount);
    assert_eq!(
        expected_commission_amount,
        simulation_res.commission_amount.amount
    );
    assert_eq!(expected_spread_amount, simulation_res.spread_amount.amount);
    assert_eq!(
        expected_minus_spread_amount,
        simulation_res.minus_spread_amount.amount
    );

    // check reverse simulation res
    let reverse_simulation_res: ReverseSimulationResponse =
        query_reverse_simulation(&deps, expected_return_amount, SwapOperation::Sell).unwrap();
    assert_eq!(
        (offer_amount.u128() as i128 - reverse_simulation_res.offer_amount.amount.u128() as i128)
            .abs()
            < 5i128,
        true
    );
    assert_eq!(
        (expected_commission_amount.u128() as i128
            - reverse_simulation_res.commission_amount.amount.u128() as i128)
            .abs()
            < 5i128,
        true
    );
    assert_eq!(
        (expected_spread_amount.u128() as i128
            - reverse_simulation_res.spread_amount.amount.u128() as i128)
            .abs()
            < 5i128,
        true
    );
    assert_eq!(
        Uint128::zero(),
        reverse_simulation_res.minus_spread_amount.amount
    );

    assert_eq!(
        &log("return_amount", expected_return_amount.to_string() + "uusd"),
        log_return_amount
    );
    assert_eq!(
        &log("spread_amount", expected_spread_amount.to_string() + "uusd"),
        log_spread_amount
    );
    assert_eq!(
        &log(
            "minus_spread_amount",
            expected_minus_spread_amount.to_string() + "uusd"
        ),
        log_minus_spread_amount
    );
    assert_eq!(
        &log(
            "commission_amount",
            expected_commission_amount.to_string() + "uusd"
        ),
        log_commission_amount
    );

    assert_eq!(
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::TransferFrom {
                owner: HumanAddr::from("addr0000"),
                recipient: HumanAddr::from(MOCK_CONTRACT_ADDR),
                amount: offer_amount,
            })
            .unwrap(),
            send: vec![],
        }),
        msg_transfer_from,
    );

    assert_eq!(
        &CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
            to_address: HumanAddr::from("addr0000"),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: expected_return_amount,
            }],
        }),
        msg_transfer,
    );

    assert_eq!(
        &CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
            to_address: HumanAddr::from("collector0000"),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: expected_inactive_commission,
            }],
        }),
        msg_commission_transfer,
    );

    // over max spread
    let offer_amount = Uint128(10000000000u128);
    let msg = HandleMsg::Sell {
        amount: offer_amount,
        max_spread: Some(Decimal::zero()),
    };
    let env = mock_env_with_block_time("addr0000", &[], 1000);

    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "Operation exceeds max spread limit");
        }
        _ => panic!("Must return generic error"),
    };

    // hit max spread 20%
    let offer_amount = Uint128(10000000000u128);
    let msg = HandleMsg::Sell {
        amount: offer_amount,
        max_spread: None,
    };
    let env = mock_env_with_block_time("addr0000", &[], 1000);

    let res = handle(&mut deps, env, msg).unwrap();
    let msg_transfer_from = res.messages.get(0).expect("no message");
    let msg_transfer = res.messages.get(1).expect("no message");
    let msg_commission_transfer = res.messages.get(2).expect("no message");
    let log_return_amount = res.log.get(2).expect("no data");
    let log_spread_amount = res.log.get(3).expect("no data");
    let log_minus_spread_amount = res.log.get(4).expect("no data");
    let log_commission_amount = res.log.get(5).expect("no data");

    let expected_ret_amount = Uint128(5_333_333_328u128);
    let expected_spread_amount = (offer_amount * exchange_rate - expected_ret_amount).unwrap();
    let expected_minus_spread_amount = Uint128::zero();
    let expected_active_commission = expected_ret_amount.multiply_ratio(2u128, 1000u128); // 0.2%
    let expected_inactive_commission = expected_ret_amount.multiply_ratio(1u128, 1000u128); // 0.1%
    let expected_commission_amount = expected_active_commission + expected_inactive_commission;
    let expected_return_amount = (expected_ret_amount - expected_commission_amount).unwrap();

    // check simulation res
    let simulation_res: SimulationResponse =
        query_simulation(&deps, offer_amount, SwapOperation::Sell).unwrap();
    assert_eq!(expected_return_amount, simulation_res.return_amount.amount);
    assert_eq!(
        expected_commission_amount,
        simulation_res.commission_amount.amount
    );
    assert_eq!(expected_spread_amount, simulation_res.spread_amount.amount);
    assert_eq!(
        expected_minus_spread_amount,
        simulation_res.minus_spread_amount.amount
    );

    // check reverse simulation res
    let reverse_simulation_res: ReverseSimulationResponse =
        query_reverse_simulation(&deps, expected_return_amount, SwapOperation::Sell).unwrap();
    assert_eq!(
        (offer_amount.u128() as i128 - reverse_simulation_res.offer_amount.amount.u128() as i128)
            .abs()
            < 50i128,
        true
    );
    assert_eq!(
        (expected_commission_amount.u128() as i128
            - reverse_simulation_res.commission_amount.amount.u128() as i128)
            .abs()
            < 50i128,
        true
    );
    assert_eq!(
        (expected_spread_amount.u128() as i128
            - reverse_simulation_res.spread_amount.amount.u128() as i128)
            .abs()
            < 50i128,
        true
    );
    assert_eq!(
        Uint128::zero(),
        reverse_simulation_res.minus_spread_amount.amount
    );

    assert_eq!(
        &log("return_amount", expected_return_amount.to_string() + "uusd"),
        log_return_amount
    );
    assert_eq!(
        &log("spread_amount", expected_spread_amount.to_string() + "uusd"),
        log_spread_amount
    );
    assert_eq!(
        &log(
            "minus_spread_amount",
            expected_minus_spread_amount.to_string() + "uusd"
        ),
        log_minus_spread_amount
    );
    assert_eq!(
        &log(
            "commission_amount",
            expected_commission_amount.to_string() + "uusd"
        ),
        log_commission_amount
    );

    assert_eq!(
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::TransferFrom {
                owner: HumanAddr::from("addr0000"),
                recipient: HumanAddr::from(MOCK_CONTRACT_ADDR),
                amount: offer_amount,
            })
            .unwrap(),
            send: vec![],
        }),
        msg_transfer_from,
    );

    assert_eq!(
        &CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
            to_address: HumanAddr::from("addr0000"),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: expected_return_amount,
            }],
        }),
        msg_transfer,
    );

    assert_eq!(
        &CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
            to_address: HumanAddr::from("collector0000"),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: expected_inactive_commission,
            }],
        }),
        msg_commission_transfer,
    );

    // hit max minus spread +2%
    let price = Decimal::from_ratio(1u128, 2u128);
    let exchange_rate = decimal_multiplication(price, Decimal::one());
    deps.querier
        .with_oracle_price(&[(&HumanAddr::from("oracle0000"), &price)]);

    let offer_amount = Uint128(100000000u128);
    let msg = HandleMsg::Sell {
        amount: offer_amount,
        max_spread: None,
    };
    let env = mock_env_with_block_time("addr0000", &[], 1000);

    let res = handle(&mut deps, env, msg).unwrap();
    let msg_transfer_from = res.messages.get(0).expect("no message");
    let msg_transfer = res.messages.get(1).expect("no message");
    let msg_commission_transfer = res.messages.get(2).expect("no message");
    let log_return_amount = res.log.get(2).expect("no data");
    let log_spread_amount = res.log.get(3).expect("no data");
    let log_minus_spread_amount = res.log.get(4).expect("no data");
    let log_commission_amount = res.log.get(5).expect("no data");

    let expected_ret_amount = offer_amount * exchange_rate;
    let expected_minus_spread_amount = expected_ret_amount * Decimal::percent(2);
    let expected_ret_amount = expected_ret_amount + expected_minus_spread_amount;
    let expected_active_commission = expected_ret_amount.multiply_ratio(2u128, 1000u128); // 0.2%
    let expected_inactive_commission = expected_ret_amount.multiply_ratio(1u128, 1000u128); // 0.1%
    let expected_commission_amount = expected_active_commission + expected_inactive_commission;
    let expected_return_amount = (expected_ret_amount - expected_commission_amount).unwrap();

    // check simulation res
    let simulation_res: SimulationResponse =
        query_simulation(&deps, offer_amount, SwapOperation::Sell).unwrap();
    assert_eq!(expected_return_amount, simulation_res.return_amount.amount);
    assert_eq!(
        expected_commission_amount,
        simulation_res.commission_amount.amount
    );
    assert_eq!(Uint128::zero(), simulation_res.spread_amount.amount);
    assert_eq!(
        expected_minus_spread_amount,
        simulation_res.minus_spread_amount.amount
    );

    // check reverse simulation res
    let reverse_simulation_res: ReverseSimulationResponse =
        query_reverse_simulation(&deps, expected_return_amount, SwapOperation::Sell).unwrap();
    assert_eq!(
        (offer_amount.u128() as i128 - reverse_simulation_res.offer_amount.amount.u128() as i128)
            .abs()
            < 5i128,
        true
    );
    assert_eq!(
        (expected_commission_amount.u128() as i128
            - reverse_simulation_res.commission_amount.amount.u128() as i128)
            .abs()
            < 5i128,
        true
    );
    assert_eq!(Uint128::zero(), reverse_simulation_res.spread_amount.amount);
    assert_eq!(
        (expected_minus_spread_amount.u128() as i128
            - reverse_simulation_res.minus_spread_amount.amount.u128() as i128)
            .abs()
            < 5i128,
        true
    );

    // check logs
    assert_eq!(
        &log("return_amount", expected_return_amount.to_string() + "uusd"),
        log_return_amount
    );
    assert_eq!(&log("spread_amount", "0uusd"), log_spread_amount);
    assert_eq!(
        &log(
            "minus_spread_amount",
            expected_minus_spread_amount.to_string() + "uusd"
        ),
        log_minus_spread_amount
    );
    assert_eq!(
        &log(
            "commission_amount",
            expected_commission_amount.to_string() + "uusd"
        ),
        log_commission_amount
    );

    assert_eq!(
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::TransferFrom {
                owner: HumanAddr::from("addr0000"),
                recipient: HumanAddr::from(MOCK_CONTRACT_ADDR),
                amount: offer_amount,
            })
            .unwrap(),
            send: vec![],
        }),
        msg_transfer_from,
    );

    assert_eq!(
        &CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
            to_address: HumanAddr::from("addr0000"),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: expected_return_amount,
            }],
        }),
        msg_transfer,
    );

    assert_eq!(
        &CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
            to_address: HumanAddr::from("collector0000"),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: expected_inactive_commission,
            }],
        }),
        msg_commission_transfer,
    );
}

#[test]
fn test_deduct() {
    let mut deps = mock_dependencies(20, &[]);

    let tax_rate = Decimal::percent(2);
    let tax_cap = Uint128::from(1_000_000u128);
    deps.querier.with_tax(
        Decimal::percent(2),
        &[(&"uusd".to_string(), &Uint128::from(1000000u128))],
    );
    let amount = Uint128(1000_000_000u128);
    let expected_after_amount = std::cmp::max(
        (amount - amount * tax_rate).unwrap(),
        (amount - tax_cap).unwrap(),
    );

    let after_amount = deduct_tax(
        &deps,
        Coin {
            denom: "uusd".to_string(),
            amount: amount,
        },
    )
    .unwrap();

    assert_eq!(expected_after_amount, after_amount.amount);
}

#[test]
fn test_query_pool() {
    let total_share_amount = Uint128::from(111u128);
    let collateral_pool_amount = Uint128::from(222u128);
    let asset_pool_amount = Uint128::from(333u128);
    let mut deps = mock_dependencies(
        20,
        &[Coin {
            denom: "uusd".to_string(),
            amount: collateral_pool_amount,
        }],
    );

    deps.querier.with_token_balances(&[
        (
            &HumanAddr::from("asset0000"),
            &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &asset_pool_amount)],
        ),
        (
            &HumanAddr::from("liquidity0000"),
            &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &total_share_amount)],
        ),
    ]);

    let msg = InitMsg {
        collateral_denom: "uusd".to_string(),
        liquidity_token: HumanAddr("liquidity0000".to_string()),
        commission_collector: HumanAddr("collector0000".to_string()),
        asset_symbol: "mAPPL".to_string(),
        asset_token: HumanAddr("asset0000".to_string()),
        asset_oracle: HumanAddr("oracle0000".to_string()),
        active_commission: Decimal::permille(2),
        inactive_commission: Decimal::permille(1),
        max_spread: Decimal::percent(20),
        max_minus_spread: Decimal::percent(2),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    let res: PoolResponse = query_pool(&deps).unwrap();

    assert_eq!(res.asset_pool, asset_pool_amount);
    assert_eq!(res.collateral_pool, collateral_pool_amount);
    assert_eq!(res.total_share, total_share_amount);
}

fn mock_env_with_block_time<U: Into<HumanAddr>>(sender: U, sent: &[Coin], time: u64) -> Env {
    let env = mock_env(sender, sent);
    // register time
    return Env {
        block: BlockInfo {
            height: 1,
            time,
            chain_id: "columbus".to_string(),
        },
        ..env
    };
}
