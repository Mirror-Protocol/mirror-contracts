use crate::contract::{handle, init, query_config};
use crate::mock_querier::{mock_dependencies, WhitelistItem};
use crate::msg::{ConfigResponse, HandleMsg, InitMsg, MarketHandleMsg};
use cosmwasm_std::testing::{mock_env, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{to_binary, Coin, CosmosMsg, Decimal, HumanAddr, Uint128, WasmMsg};
use cw20::Cw20HandleMsg;

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        factory_contract: HumanAddr("factory0000".to_string()),
        mirror_symbol: "mirror".to_string(),
        collateral_denom: "uusd".to_string(),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    // it worked, let's query the state
    let config: ConfigResponse = query_config(&deps).unwrap();
    assert_eq!("factory0000", config.factory_contract.as_str());
    assert_eq!("mirror", config.mirror_symbol.as_str());
    assert_eq!("uusd", config.collateral_denom.as_str());
}

#[test]
fn test_convert() {
    let mut deps = mock_dependencies(
        20,
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128(100u128),
        }],
    );
    deps.querier.with_token_balances(&[(
        &HumanAddr::from("tokenAPPL"),
        &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &Uint128(100u128))],
    )]);

    deps.querier.with_tax(
        Decimal::percent(1),
        &[(&"uusd".to_string(), &Uint128(1000000u128))],
    );

    deps.querier.with_whitelist(&[(
        &HumanAddr::from("factory0000"),
        vec![
            (
                &"mAPPL".to_string(),
                &WhitelistItem {
                    token_contract: HumanAddr::from("tokenAPPL"),
                    market_contract: HumanAddr::from("marketAPPL"),
                    staking_contract: HumanAddr::from("stakingAPPL"),
                },
            ),
            (
                &"MIRROR".to_string(),
                &WhitelistItem {
                    token_contract: HumanAddr::from("tokenMIRROR"),
                    market_contract: HumanAddr::from("marketMIRROR"),
                    staking_contract: HumanAddr::from("stakingMIRROR"),
                },
            ),
        ],
    )]);

    let msg = InitMsg {
        factory_contract: HumanAddr("factory0000".to_string()),
        mirror_symbol: "MIRROR".to_string(),
        collateral_denom: "uusd".to_string(),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::Convert {
        symbol: "mAPPL".to_string(),
    };

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("tokenAPPL"),
                msg: to_binary(&Cw20HandleMsg::IncreaseAllowance {
                    spender: HumanAddr::from("marketAPPL"),
                    amount: Uint128(100u128),
                    expires: None,
                })
                .unwrap(),
                send: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("marketAPPL"),
                msg: to_binary(&MarketHandleMsg::Sell {
                    amount: Uint128(100u128),
                    max_spread: None,
                })
                .unwrap(),
                send: vec![],
            })
        ]
    );

    let msg = HandleMsg::Convert {
        symbol: "MIRROR".to_string(),
    };

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap();

    // tax deduct 100 => 99
    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("marketMIRROR"),
            msg: to_binary(&MarketHandleMsg::Buy { max_spread: None }).unwrap(),
            send: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128(99u128),
            }],
        })]
    );
}

#[test]
fn test_send() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_token_balances(&[(
        &HumanAddr::from("tokenMIRROR"),
        &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &Uint128(100u128))],
    )]);
    deps.querier.with_whitelist(&[(
        &HumanAddr::from("factory0000"),
        vec![(
            &"MIRROR".to_string(),
            &WhitelistItem {
                token_contract: HumanAddr::from("tokenMIRROR"),
                market_contract: HumanAddr::from("marketMIRROR"),
                staking_contract: HumanAddr::from("stakingMIRROR"),
            },
        )],
    )]);

    let msg = InitMsg {
        factory_contract: HumanAddr("factory0000".to_string()),
        mirror_symbol: "MIRROR".to_string(),
        collateral_denom: "uusd".to_string(),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();
    let msg = HandleMsg::Send {};

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("tokenMIRROR"),
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: HumanAddr::from("stakingMIRROR"),
                amount: Uint128(100u128),
            })
            .unwrap(),
            send: vec![],
        })]
    )
}
