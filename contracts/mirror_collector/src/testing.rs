use crate::contract::{handle, init, query_config};
use crate::mock_querier::mock_dependencies;
use cosmwasm_std::testing::{mock_env, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{to_binary, Coin, CosmosMsg, Decimal, HumanAddr, Uint128, WasmMsg};
use cw20::Cw20HandleMsg;
use mirror_protocol::collector::{ConfigResponse, HandleMsg, InitMsg};
use mirror_protocol::gov::Cw20HookMsg::DepositReward;
use terraswap::asset::{Asset, AssetInfo};
use terraswap::pair::{Cw20HookMsg as TerraswapCw20HookMsg, HandleMsg as TerraswapHandleMsg};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        terraswap_factory: HumanAddr("terraswapfactory".to_string()),
        distribution_contract: HumanAddr("gov0000".to_string()),
        mirror_token: HumanAddr("mirror0000".to_string()),
        base_denom: "uusd".to_string(),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    // it worked, let's query the state
    let config: ConfigResponse = query_config(&deps).unwrap();
    assert_eq!("terraswapfactory", config.terraswap_factory.as_str());
    assert_eq!("uusd", config.base_denom.as_str());
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

    deps.querier.with_terraswap_pairs(&[
        (&"uusdtokenAPPL".to_string(), &HumanAddr::from("pairAPPL")),
        (
            &"uusdtokenMIRROR".to_string(),
            &HumanAddr::from("pairMIRROR"),
        ),
    ]);

    let msg = InitMsg {
        terraswap_factory: HumanAddr("terraswapfactory".to_string()),
        distribution_contract: HumanAddr("gov0000".to_string()),
        mirror_token: HumanAddr("tokenMIRROR".to_string()),
        base_denom: "uusd".to_string(),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::Convert {
        asset_token: HumanAddr::from("tokenAPPL"),
    };

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("tokenAPPL"),
            msg: to_binary(&Cw20HandleMsg::Send {
                contract: HumanAddr::from("pairAPPL"),
                amount: Uint128(100u128),
                msg: Some(
                    to_binary(&TerraswapCw20HookMsg::Swap {
                        max_spread: None,
                        belief_price: None,
                        to: None,
                    })
                    .unwrap()
                ),
            })
            .unwrap(),
            send: vec![],
        })]
    );

    let msg = HandleMsg::Convert {
        asset_token: HumanAddr::from("tokenMIRROR"),
    };

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap();

    // tax deduct 100 => 99
    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("pairMIRROR"),
            msg: to_binary(&TerraswapHandleMsg::Swap {
                offer_asset: Asset {
                    info: AssetInfo::NativeToken {
                        denom: "uusd".to_string()
                    },
                    amount: Uint128(99u128),
                },
                max_spread: None,
                belief_price: None,
                to: None,
            })
            .unwrap(),
            send: vec![Coin {
                amount: Uint128(99u128),
                denom: "uusd".to_string(),
            }],
        })]
    );
}

#[test]
fn test_send() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_token_balances(&[(
        &HumanAddr::from("mirror0000"),
        &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &Uint128(100u128))],
    )]);

    let msg = InitMsg {
        terraswap_factory: HumanAddr("terraswapfactory".to_string()),
        distribution_contract: HumanAddr("gov0000".to_string()),
        mirror_token: HumanAddr("mirror0000".to_string()),
        base_denom: "uusd".to_string(),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();
    let msg = HandleMsg::Distribute {};

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("mirror0000"),
            msg: to_binary(&Cw20HandleMsg::Send {
                contract: HumanAddr::from("gov0000"),
                amount: Uint128(100u128),
                msg: Some(to_binary(&DepositReward {}).unwrap()),
            })
            .unwrap(),
            send: vec![],
        })]
    )
}
