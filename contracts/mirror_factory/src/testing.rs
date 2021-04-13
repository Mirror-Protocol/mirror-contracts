use crate::contract::{handle, init, query};
use crate::mock_querier::mock_dependencies;

use crate::state::{read_params, read_total_weight, read_weight, store_total_weight, store_weight};
use cosmwasm_std::testing::{mock_env, MOCK_CONTRACT_ADDR};
use cosmwasm_std::Api;
use cosmwasm_std::{
    from_binary, log, to_binary, CosmosMsg, Decimal, Env, HumanAddr, StdError, Uint128, WasmMsg,
};
use cw20::{Cw20HandleMsg, MinterResponse};

use mirror_protocol::factory::{
    ConfigResponse, DistributionInfoResponse, HandleMsg, InitMsg, Params, QueryMsg,
};
use mirror_protocol::mint::HandleMsg as MintHandleMsg;
use mirror_protocol::oracle::HandleMsg as OracleHandleMsg;
use mirror_protocol::staking::Cw20HookMsg as StakingCw20HookMsg;
use mirror_protocol::staking::HandleMsg as StakingHandleMsg;
use terraswap::asset::AssetInfo;
use terraswap::factory::HandleMsg as TerraswapFactoryHandleMsg;
use terraswap::hook::InitHook;
use terraswap::token::InitMsg as TokenInitMsg;

fn mock_env_time(signer: &HumanAddr, time: u64) -> Env {
    let mut env = mock_env(signer, &[]);
    env.block.time = time;
    env
}

static TOKEN_CODE_ID: u64 = 10u64;
static BASE_DENOM: &str = "uusd";

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitialize {
        owner: HumanAddr::from("owner0000"),
        mirror_token: HumanAddr::from("mirror0000"),
        mint_contract: HumanAddr::from("mint0000"),
        staking_contract: HumanAddr::from("staking0000"),
        commission_collector: HumanAddr::from("collector0000"),
        oracle_contract: HumanAddr::from("oracle0000"),
        terraswap_factory: HumanAddr::from("terraswapfactory"),
    };
    let _res = handle(&mut deps, env.clone(), msg).unwrap();

    // cannot update mirror token after initialization
    let msg = HandleMsg::PostInitialize {
        owner: HumanAddr::from("owner0000"),
        mirror_token: HumanAddr::from("mirror0000"),
        mint_contract: HumanAddr::from("mint0000"),
        staking_contract: HumanAddr::from("staking0000"),
        commission_collector: HumanAddr::from("collector0000"),
        oracle_contract: HumanAddr::from("oracle0000"),
        terraswap_factory: HumanAddr::from("terraswapfactory"),
    };
    let _res = handle(&mut deps, env, msg).unwrap_err();

    // it worked, let's query the state
    let res = query(&deps, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            owner: HumanAddr::from("owner0000"),
            mirror_token: HumanAddr::from("mirror0000"),
            mint_contract: HumanAddr::from("mint0000"),
            staking_contract: HumanAddr::from("staking0000"),
            commission_collector: HumanAddr::from("collector0000"),
            oracle_contract: HumanAddr::from("oracle0000"),
            terraswap_factory: HumanAddr::from("terraswapfactory"),
            base_denom: BASE_DENOM.to_string(),
            token_code_id: TOKEN_CODE_ID,
            genesis_time: 1_571_797_419,
            distribution_schedule: vec![],
        }
    );
}

#[test]
fn test_update_config() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitialize {
        owner: HumanAddr::from("owner0000"),
        mirror_token: HumanAddr::from("mirror0000"),
        mint_contract: HumanAddr::from("mint0000"),
        staking_contract: HumanAddr::from("staking0000"),
        commission_collector: HumanAddr::from("collector0000"),
        oracle_contract: HumanAddr::from("oracle0000"),
        terraswap_factory: HumanAddr::from("terraswapfactory"),
    };
    let _res = handle(&mut deps, env.clone(), msg).unwrap();

    // upate owner
    let msg = HandleMsg::UpdateConfig {
        owner: Some(HumanAddr::from("owner0001")),
        distribution_schedule: None,
        token_code_id: None,
    };

    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();
    let res = query(&deps, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            owner: HumanAddr::from("owner0001"),
            mirror_token: HumanAddr::from("mirror0000"),
            mint_contract: HumanAddr::from("mint0000"),
            staking_contract: HumanAddr::from("staking0000"),
            commission_collector: HumanAddr::from("collector0000"),
            oracle_contract: HumanAddr::from("oracle0000"),
            terraswap_factory: HumanAddr::from("terraswapfactory"),
            base_denom: BASE_DENOM.to_string(),
            token_code_id: TOKEN_CODE_ID,
            genesis_time: 1_571_797_419,
            distribution_schedule: vec![],
        }
    );

    // update rest part
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        distribution_schedule: Some(vec![(1, 2, Uint128::from(123u128))]),
        token_code_id: Some(TOKEN_CODE_ID + 1),
    };

    let env = mock_env("owner0001", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();
    let res = query(&deps, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            owner: HumanAddr::from("owner0001"),
            mirror_token: HumanAddr::from("mirror0000"),
            mint_contract: HumanAddr::from("mint0000"),
            staking_contract: HumanAddr::from("staking0000"),
            commission_collector: HumanAddr::from("collector0000"),
            oracle_contract: HumanAddr::from("oracle0000"),
            terraswap_factory: HumanAddr::from("terraswapfactory"),
            base_denom: BASE_DENOM.to_string(),
            token_code_id: TOKEN_CODE_ID + 1,
            genesis_time: 1_571_797_419,
            distribution_schedule: vec![(1, 2, Uint128::from(123u128))],
        }
    );

    // failed unauthoirzed
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        distribution_schedule: None,
        token_code_id: Some(TOKEN_CODE_ID + 1),
    };

    let env = mock_env("owner0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::Unauthorized { .. } => {}
        _ => panic!("DO NOT ENTER HERE"),
    }
}

#[test]
fn test_update_weight() {
    let mut deps = mock_dependencies(20, &[]);
    let msg = InitMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitialize {
        owner: HumanAddr::from("owner0000"),
        mirror_token: HumanAddr::from("mirror0000"),
        mint_contract: HumanAddr::from("mint0000"),
        staking_contract: HumanAddr::from("staking0000"),
        commission_collector: HumanAddr::from("collector0000"),
        oracle_contract: HumanAddr::from("oracle0000"),
        terraswap_factory: HumanAddr::from("terraswapfactory"),
    };
    let _res = handle(&mut deps, env.clone(), msg).unwrap();

    store_total_weight(&mut deps.storage, 100).unwrap();
    store_weight(
        &mut deps.storage,
        &deps
            .api
            .canonical_address(&HumanAddr::from("asset0000"))
            .unwrap(),
        10,
    )
    .unwrap();

    // incrase weight
    let msg = HandleMsg::UpdateWeight {
        asset_token: HumanAddr::from("asset0000"),
        weight: 20,
    };
    let env = mock_env("owner0001", &[]);
    let res = handle(&mut deps, env, msg.clone()).unwrap_err();
    match res {
        StdError::Unauthorized { .. } => {}
        _ => panic!("DO NOT ENTER HERE"),
    }

    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();
    let res = query(&deps, QueryMsg::DistributionInfo {}).unwrap();
    let distribution_info: DistributionInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        distribution_info,
        DistributionInfoResponse {
            weights: vec![(HumanAddr::from("asset0000"), 20)],
            last_distributed: 1_571_797_419,
        }
    );

    assert_eq!(
        read_weight(
            &deps.storage,
            &deps
                .api
                .canonical_address(&HumanAddr::from("asset0000"))
                .unwrap()
        )
        .unwrap(),
        20u32
    );
    assert_eq!(read_total_weight(&deps.storage).unwrap(), 110u32);
}

#[test]
fn test_whitelist() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitialize {
        owner: HumanAddr::from("owner0000"),
        mirror_token: HumanAddr::from("mirror0000"),
        mint_contract: HumanAddr::from("mint0000"),
        staking_contract: HumanAddr::from("staking0000"),
        commission_collector: HumanAddr::from("collector0000"),
        oracle_contract: HumanAddr::from("oracle0000"),
        terraswap_factory: HumanAddr::from("terraswapfactory"),
    };
    let _res = handle(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::Whitelist {
        name: "apple derivative".to_string(),
        symbol: "mAPPL".to_string(),
        oracle_feeder: HumanAddr::from("feeder0000"),
        params: Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
            weight: Some(100u32),
            mint_period: None,
            min_collateral_ratio_after_migration: None,
        },
    };
    let env = mock_env("owner0000", &[]);
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap();

    assert_eq!(
        res.log,
        vec![
            log("action", "whitelist"),
            log("symbol", "mAPPL"),
            log("name", "apple derivative")
        ]
    );

    // token creation msg should be returned
    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Instantiate {
            code_id: TOKEN_CODE_ID,
            send: vec![],
            label: None,
            msg: to_binary(&TokenInitMsg {
                name: "apple derivative".to_string(),
                symbol: "mAPPL".to_string(),
                decimals: 6u8,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: HumanAddr::from("mint0000"),
                    cap: None,
                }),
                init_hook: Some(InitHook {
                    contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                    msg: to_binary(&HandleMsg::TokenCreationHook {
                        oracle_feeder: HumanAddr::from("feeder0000")
                    })
                    .unwrap(),
                }),
            })
            .unwrap(),
        })]
    );

    let params: Params = read_params(&deps.storage).unwrap();
    assert_eq!(
        params,
        Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
            weight: Some(100u32),
            mint_period: None,
            min_collateral_ratio_after_migration: None,
        }
    );

    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "A whitelist process is in progress"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let env = mock_env("addr0001", &[]);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::Unauthorized { .. } => {}
        _ => panic!("DO NOT ENTER HERE"),
    }
}

#[test]
fn test_token_creation_hook() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitialize {
        owner: HumanAddr::from("owner0000"),
        mirror_token: HumanAddr::from("mirror0000"),
        mint_contract: HumanAddr::from("mint0000"),
        staking_contract: HumanAddr::from("staking0000"),
        commission_collector: HumanAddr::from("collector0000"),
        oracle_contract: HumanAddr::from("oracle0000"),
        terraswap_factory: HumanAddr::from("terraswapfactory"),
    };
    let _res = handle(&mut deps, env.clone(), msg).unwrap();

    // There is no whitelist process; failed
    let msg = HandleMsg::TokenCreationHook {
        oracle_feeder: HumanAddr::from("feeder0000"),
    };
    let env = mock_env("asset0000", &[]);
    let res = handle(&mut deps, env.clone(), msg.clone());
    match res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "No whitelist process in progress")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = HandleMsg::Whitelist {
        name: "apple derivative".to_string(),
        symbol: "mAPPL".to_string(),
        oracle_feeder: HumanAddr::from("feeder0000"),
        params: Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
            weight: Some(100u32),
            mint_period: None,
            min_collateral_ratio_after_migration: None,
        },
    };
    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env.clone(), msg.clone()).unwrap();

    let msg = HandleMsg::TokenCreationHook {
        oracle_feeder: HumanAddr::from("feeder0000"),
    };
    let env = mock_env("asset0000", &[]);
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("mint0000"),
                send: vec![],
                msg: to_binary(&MintHandleMsg::RegisterAsset {
                    asset_token: HumanAddr::from("asset0000"),
                    auction_discount: Decimal::percent(5),
                    min_collateral_ratio: Decimal::percent(150),
                    mint_end: None,
                    min_collateral_ratio_after_migration: None,
                })
                .unwrap(),
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("oracle0000"),
                send: vec![],
                msg: to_binary(&OracleHandleMsg::RegisterAsset {
                    asset_token: HumanAddr::from("asset0000"),
                    feeder: HumanAddr::from("feeder0000"),
                })
                .unwrap(),
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("terraswapfactory"),
                send: vec![],
                msg: to_binary(&TerraswapFactoryHandleMsg::CreatePair {
                    asset_infos: [
                        AssetInfo::NativeToken {
                            denom: BASE_DENOM.to_string(),
                        },
                        AssetInfo::Token {
                            contract_addr: HumanAddr::from("asset0000"),
                        },
                    ],
                    init_hook: Some(InitHook {
                        msg: to_binary(&HandleMsg::TerraswapCreationHook {
                            asset_token: HumanAddr::from("asset0000"),
                        })
                        .unwrap(),
                        contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                    }),
                })
                .unwrap(),
            })
        ]
    );

    let res = query(&deps, QueryMsg::DistributionInfo {}).unwrap();
    let distribution_info: DistributionInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        distribution_info,
        DistributionInfoResponse {
            weights: vec![(HumanAddr::from("asset0000"), 100)],
            last_distributed: 1_571_797_419,
        }
    );

    // There is no whitelist process; failed
    let msg = HandleMsg::TokenCreationHook {
        oracle_feeder: HumanAddr::from("feeder0000"),
    };
    let env = mock_env("asset0000", &[]);
    let res = handle(&mut deps, env.clone(), msg.clone());
    match res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "No whitelist process in progress")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }
}

#[test]
fn test_token_creation_hook_without_weight() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitialize {
        owner: HumanAddr::from("owner0000"),
        mirror_token: HumanAddr::from("mirror0000"),
        mint_contract: HumanAddr::from("mint0000"),
        staking_contract: HumanAddr::from("staking0000"),
        commission_collector: HumanAddr::from("collector0000"),
        oracle_contract: HumanAddr::from("oracle0000"),
        terraswap_factory: HumanAddr::from("terraswapfactory"),
    };
    let _res = handle(&mut deps, env.clone(), msg).unwrap();

    // There is no whitelist process; failed
    let msg = HandleMsg::TokenCreationHook {
        oracle_feeder: HumanAddr::from("feeder0000"),
    };
    let env = mock_env("asset0000", &[]);
    let res = handle(&mut deps, env.clone(), msg.clone());
    match res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "No whitelist process in progress")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = HandleMsg::Whitelist {
        name: "apple derivative".to_string(),
        symbol: "mAPPL".to_string(),
        oracle_feeder: HumanAddr::from("feeder0000"),
        params: Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
            weight: None,
            mint_period: None,
            min_collateral_ratio_after_migration: None,
        },
    };
    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env.clone(), msg.clone()).unwrap();

    let msg = HandleMsg::TokenCreationHook {
        oracle_feeder: HumanAddr::from("feeder0000"),
    };
    let env = mock_env("asset0000", &[]);
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("mint0000"),
                send: vec![],
                msg: to_binary(&MintHandleMsg::RegisterAsset {
                    asset_token: HumanAddr::from("asset0000"),
                    auction_discount: Decimal::percent(5),
                    min_collateral_ratio: Decimal::percent(150),
                    mint_end: None,
                    min_collateral_ratio_after_migration: None,
                })
                .unwrap(),
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("oracle0000"),
                send: vec![],
                msg: to_binary(&OracleHandleMsg::RegisterAsset {
                    asset_token: HumanAddr::from("asset0000"),
                    feeder: HumanAddr::from("feeder0000"),
                })
                .unwrap(),
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("terraswapfactory"),
                send: vec![],
                msg: to_binary(&TerraswapFactoryHandleMsg::CreatePair {
                    asset_infos: [
                        AssetInfo::NativeToken {
                            denom: BASE_DENOM.to_string(),
                        },
                        AssetInfo::Token {
                            contract_addr: HumanAddr::from("asset0000"),
                        },
                    ],
                    init_hook: Some(InitHook {
                        msg: to_binary(&HandleMsg::TerraswapCreationHook {
                            asset_token: HumanAddr::from("asset0000"),
                        })
                        .unwrap(),
                        contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                    }),
                })
                .unwrap(),
            })
        ]
    );

    let res = query(&deps, QueryMsg::DistributionInfo {}).unwrap();
    let distribution_info: DistributionInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        distribution_info,
        DistributionInfoResponse {
            weights: vec![(HumanAddr::from("asset0000"), 30)],
            last_distributed: 1_571_797_419,
        }
    );

    // There is no whitelist process; failed
    let msg = HandleMsg::TokenCreationHook {
        oracle_feeder: HumanAddr::from("feeder0000"),
    };
    let env = mock_env("asset0000", &[]);
    let res = handle(&mut deps, env.clone(), msg.clone());
    match res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "No whitelist process in progress")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }
}

#[test]
fn test_terraswap_creation_hook() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier
        .with_terraswap_pairs(&[(&"uusdasset0000".to_string(), &HumanAddr::from("LP0000"))]);

    let msg = InitMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitialize {
        owner: HumanAddr::from("owner0000"),
        mirror_token: HumanAddr::from("mirror0000"),
        mint_contract: HumanAddr::from("mint0000"),
        staking_contract: HumanAddr::from("staking0000"),
        commission_collector: HumanAddr::from("collector0000"),
        oracle_contract: HumanAddr::from("oracle0000"),
        terraswap_factory: HumanAddr::from("terraswapfactory"),
    };
    let _res = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::Whitelist {
        name: "apple derivative".to_string(),
        symbol: "mAPPL".to_string(),
        oracle_feeder: HumanAddr::from("feeder0000"),
        params: Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
            weight: Some(100u32),
            mint_period: None,
            min_collateral_ratio_after_migration: None,
        },
    };
    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::TokenCreationHook {
        oracle_feeder: HumanAddr::from("feeder0000"),
    };
    let env = mock_env("asset0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::TerraswapCreationHook {
        asset_token: HumanAddr::from("asset0000"),
    };
    let env = mock_env("terraswapfactory", &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("staking0000"),
            send: vec![],
            msg: to_binary(&StakingHandleMsg::RegisterAsset {
                asset_token: HumanAddr::from("asset0000"),
                staking_token: HumanAddr::from("LP0000"),
            })
            .unwrap(),
        })]
    );
}

#[test]
fn test_distribute() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_terraswap_pairs(&[
        (&"uusdasset0000".to_string(), &HumanAddr::from("LP0000")),
        (&"uusdasset0001".to_string(), &HumanAddr::from("LP0001")),
    ]);

    let msg = InitMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![
            (1800, 3600, Uint128::from(3600u128)),
            (3600, 3600 + 3600, Uint128::from(7200u128)),
        ],
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitialize {
        owner: HumanAddr::from("owner0000"),
        mirror_token: HumanAddr::from("mirror0000"),
        mint_contract: HumanAddr::from("mint0000"),
        staking_contract: HumanAddr::from("staking0000"),
        commission_collector: HumanAddr::from("collector0000"),
        oracle_contract: HumanAddr::from("oracle0000"),
        terraswap_factory: HumanAddr::from("terraswapfactory"),
    };
    let _res = handle(&mut deps, env, msg).unwrap();

    // whitelist first item with weight 1.5
    let msg = HandleMsg::Whitelist {
        name: "apple derivative".to_string(),
        symbol: "mAPPL".to_string(),
        oracle_feeder: HumanAddr::from("feeder0000"),
        params: Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
            weight: Some(100u32),
            mint_period: None,
            min_collateral_ratio_after_migration: None,
        },
    };
    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::TokenCreationHook {
        oracle_feeder: HumanAddr::from("feeder0000"),
    };
    let env = mock_env("asset0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::TerraswapCreationHook {
        asset_token: HumanAddr::from("asset0000"),
    };
    let env = mock_env("terraswapfactory", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // Whitelist second item with weight 1
    let msg = HandleMsg::Whitelist {
        name: "google derivative".to_string(),
        symbol: "mGOGL".to_string(),
        oracle_feeder: HumanAddr::from("feeder0000"),
        params: Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
            weight: Some(100u32),
            mint_period: None,
            min_collateral_ratio_after_migration: None,
        },
    };
    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::TokenCreationHook {
        oracle_feeder: HumanAddr::from("feeder0000"),
    };
    let env = mock_env("asset0001", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::TerraswapCreationHook {
        asset_token: HumanAddr::from("asset0001"),
    };
    let env = mock_env("terraswapfactory", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // height is not increased so zero amount will be minted
    let msg = HandleMsg::Distribute {};
    let env = mock_env("anyone", &[]);
    let res = handle(&mut deps, env, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "Cannot distribute mirror token before interval")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    // one height increase
    let msg = HandleMsg::Distribute {};
    let env = mock_env_time(&HumanAddr::from("addr0000"), 1_571_797_419u64 + 5400u64);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "distribute"),
            log("distributed_amount", "7200"),
        ]
    );

    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("mirror0000"),
                msg: to_binary(&Cw20HandleMsg::Send {
                    contract: HumanAddr::from("staking0000"),
                    amount: Uint128(3600u128),
                    msg: Some(
                        to_binary(&StakingCw20HookMsg::DepositReward {
                            asset_token: HumanAddr::from("asset0000"),
                        })
                        .unwrap()
                    ),
                })
                .unwrap(),
                send: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("mirror0000"),
                msg: to_binary(&Cw20HandleMsg::Send {
                    contract: HumanAddr::from("staking0000"),
                    amount: Uint128(3600u128),
                    msg: Some(
                        to_binary(&StakingCw20HookMsg::DepositReward {
                            asset_token: HumanAddr::from("asset0001"),
                        })
                        .unwrap()
                    ),
                })
                .unwrap(),
                send: vec![],
            }),
        ],
    );

    let res = query(&deps, QueryMsg::DistributionInfo {}).unwrap();
    let distribution_info: DistributionInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        distribution_info,
        DistributionInfoResponse {
            weights: vec![
                (HumanAddr::from("asset0000"), 100),
                (HumanAddr::from("asset0001"), 100)
            ],
            last_distributed: 1_571_802_819,
        }
    );
}

#[test]
fn test_revocation() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier
        .with_terraswap_pairs(&[(&"uusdasset0000".to_string(), &HumanAddr::from("LP0000"))]);

    let msg = InitMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitialize {
        owner: HumanAddr::from("owner0000"),
        mirror_token: HumanAddr::from("mirror0000"),
        mint_contract: HumanAddr::from("mint0000"),
        staking_contract: HumanAddr::from("staking0000"),
        commission_collector: HumanAddr::from("collector0000"),
        oracle_contract: HumanAddr::from("oracle0000"),
        terraswap_factory: HumanAddr::from("terraswapfactory"),
    };
    let _res = handle(&mut deps, env, msg).unwrap();

    // whitelist first item with weight 1.5
    let msg = HandleMsg::Whitelist {
        name: "apple derivative".to_string(),
        symbol: "mAPPL".to_string(),
        oracle_feeder: HumanAddr::from("feeder0000"),
        params: Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
            weight: Some(100u32),
            mint_period: None,
            min_collateral_ratio_after_migration: None,
        },
    };
    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::TokenCreationHook {
        oracle_feeder: HumanAddr::from("feeder0000"),
    };
    let env = mock_env("asset0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::TerraswapCreationHook {
        asset_token: HumanAddr::from("asset0000"),
    };
    let env = mock_env("terraswapfactory", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();
    // register queriers
    deps.querier.with_oracle_feeders(&[(
        &HumanAddr::from("asset0000"),
        &HumanAddr::from("feeder0000"),
    )]);

    // unauthorized revoke attempt
    let msg = HandleMsg::RevokeAsset {
        asset_token: HumanAddr::from("asset0000"),
        end_price: Decimal::from_ratio(2u128, 1u128),
    };
    let env = mock_env("owner0000", &[]);
    let res = handle(&mut deps, env, msg.clone()).unwrap_err();

    match res {
        StdError::Unauthorized { .. } => {}
        _ => panic!("DO NOT ENTER HERE"),
    }

    let env = mock_env("feeder0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("mint0000"),
            send: vec![],
            msg: to_binary(&MintHandleMsg::RegisterMigration {
                asset_token: HumanAddr::from("asset0000"),
                end_price: Decimal::from_ratio(2u128, 1u128),
            })
            .unwrap(),
        }),]
    );
}

#[test]
fn test_migration() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier
        .with_terraswap_pairs(&[(&"uusdasset0000".to_string(), &HumanAddr::from("LP0000"))]);

    let msg = InitMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitialize {
        owner: HumanAddr::from("owner0000"),
        mirror_token: HumanAddr::from("mirror0000"),
        mint_contract: HumanAddr::from("mint0000"),
        staking_contract: HumanAddr::from("staking0000"),
        commission_collector: HumanAddr::from("collector0000"),
        oracle_contract: HumanAddr::from("oracle0000"),
        terraswap_factory: HumanAddr::from("terraswapfactory"),
    };
    let _res = handle(&mut deps, env, msg).unwrap();

    // whitelist first item with weight 1.5
    let msg = HandleMsg::Whitelist {
        name: "apple derivative".to_string(),
        symbol: "mAPPL".to_string(),
        oracle_feeder: HumanAddr::from("feeder0000"),
        params: Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
            weight: Some(100u32),
            mint_period: None,
            min_collateral_ratio_after_migration: None,
        },
    };
    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::TokenCreationHook {
        oracle_feeder: HumanAddr::from("feeder0000"),
    };
    let env = mock_env("asset0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::TerraswapCreationHook {
        asset_token: HumanAddr::from("asset0000"),
    };
    let env = mock_env("terraswapfactory", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // register queriers
    deps.querier.with_mint_configs(&[(
        &HumanAddr::from("asset0000"),
        &(Decimal::percent(1), Decimal::percent(1), None),
    )]);
    deps.querier.with_oracle_feeders(&[(
        &HumanAddr::from("asset0000"),
        &HumanAddr::from("feeder0000"),
    )]);

    // unauthorized migrate attempt
    let msg = HandleMsg::MigrateAsset {
        name: "apple migration".to_string(),
        symbol: "mAPPL2".to_string(),
        from_token: HumanAddr::from("asset0000"),
        end_price: Decimal::from_ratio(2u128, 1u128),
    };
    let env = mock_env("owner0000", &[]);
    let res = handle(&mut deps, env, msg.clone()).unwrap_err();

    match res {
        StdError::Unauthorized { .. } => {}
        _ => panic!("DO NOT ENTER HERE"),
    }

    let env = mock_env("feeder0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("mint0000"),
                send: vec![],
                msg: to_binary(&MintHandleMsg::RegisterMigration {
                    asset_token: HumanAddr::from("asset0000"),
                    end_price: Decimal::from_ratio(2u128, 1u128),
                })
                .unwrap(),
            }),
            CosmosMsg::Wasm(WasmMsg::Instantiate {
                code_id: TOKEN_CODE_ID,
                send: vec![],
                label: None,
                msg: to_binary(&TokenInitMsg {
                    name: "apple migration".to_string(),
                    symbol: "mAPPL2".to_string(),
                    decimals: 6u8,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: HumanAddr::from("mint0000"),
                        cap: None,
                    }),
                    init_hook: Some(InitHook {
                        contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                        msg: to_binary(&HandleMsg::TokenCreationHook {
                            oracle_feeder: HumanAddr::from("feeder0000")
                        })
                        .unwrap(),
                    }),
                })
                .unwrap(),
            })
        ]
    );
}

#[test]
fn test_whitelist_pre_ipo_asset() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitialize {
        owner: HumanAddr::from("owner0000"),
        mirror_token: HumanAddr::from("mirror0000"),
        mint_contract: HumanAddr::from("mint0000"),
        staking_contract: HumanAddr::from("staking0000"),
        commission_collector: HumanAddr::from("collector0000"),
        oracle_contract: HumanAddr::from("oracle0000"),
        terraswap_factory: HumanAddr::from("terraswapfactory"),
    };
    let _res = handle(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::Whitelist {
        name: "pre-IPO asset".to_string(),
        symbol: "mPreIPO".to_string(),
        oracle_feeder: HumanAddr::from("feeder0000"),
        params: Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(1000),
            weight: Some(100u32),
            mint_period: Some(10000u64),
            min_collateral_ratio_after_migration: Some(Decimal::percent(150)),
        },
    };
    let env = mock_env("owner0000", &[]);
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap();

    // token creation msg should be returned
    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Instantiate {
            code_id: TOKEN_CODE_ID,
            send: vec![],
            label: None,
            msg: to_binary(&TokenInitMsg {
                name: "pre-IPO asset".to_string(),
                symbol: "mPreIPO".to_string(),
                decimals: 6u8,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: HumanAddr::from("mint0000"),
                    cap: None,
                }),
                init_hook: Some(InitHook {
                    contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                    msg: to_binary(&HandleMsg::TokenCreationHook {
                        oracle_feeder: HumanAddr::from("feeder0000")
                    })
                    .unwrap(),
                }),
            })
            .unwrap(),
        })]
    );

    let params: Params = read_params(&deps.storage).unwrap();
    assert_eq!(
        params,
        Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(1000),
            weight: Some(100u32),
            mint_period: Some(10000u64),
            min_collateral_ratio_after_migration: Some(Decimal::percent(150)),
        }
    );

    // execute token creation hook
    let msg = HandleMsg::TokenCreationHook {
        oracle_feeder: HumanAddr::from("feeder0000"),
    };

    let env = mock_env("asset0000", &[]);
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("mint0000"),
                send: vec![],
                msg: to_binary(&MintHandleMsg::RegisterAsset {
                    asset_token: HumanAddr::from("asset0000"),
                    auction_discount: Decimal::percent(5),
                    min_collateral_ratio: Decimal::percent(1000),
                    mint_end: Some(env.block.height + 10000u64),
                    min_collateral_ratio_after_migration: Some(Decimal::percent(150)),
                })
                .unwrap(),
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("oracle0000"),
                send: vec![],
                msg: to_binary(&OracleHandleMsg::RegisterAsset {
                    asset_token: HumanAddr::from("asset0000"),
                    feeder: HumanAddr::from("feeder0000"),
                })
                .unwrap(),
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("terraswapfactory"),
                send: vec![],
                msg: to_binary(&TerraswapFactoryHandleMsg::CreatePair {
                    asset_infos: [
                        AssetInfo::NativeToken {
                            denom: BASE_DENOM.to_string(),
                        },
                        AssetInfo::Token {
                            contract_addr: HumanAddr::from("asset0000"),
                        },
                    ],
                    init_hook: Some(InitHook {
                        msg: to_binary(&HandleMsg::TerraswapCreationHook {
                            asset_token: HumanAddr::from("asset0000"),
                        })
                        .unwrap(),
                        contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                    }),
                })
                .unwrap(),
            })
        ]
    );
}

#[test]
fn test_migrate_pre_ipo_asset() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_terraswap_pairs(&[(
        &"uusdpreIPOasset0000".to_string(),
        &HumanAddr::from("LP0000"),
    )]);

    let msg = InitMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitialize {
        owner: HumanAddr::from("owner0000"),
        mirror_token: HumanAddr::from("mirror0000"),
        mint_contract: HumanAddr::from("mint0000"),
        staking_contract: HumanAddr::from("staking0000"),
        commission_collector: HumanAddr::from("collector0000"),
        oracle_contract: HumanAddr::from("oracle0000"),
        terraswap_factory: HumanAddr::from("terraswapfactory"),
    };
    let _res = handle(&mut deps, env, msg).unwrap();

    // whitelist pre-IPO asset
    let msg = HandleMsg::Whitelist {
        name: "Pre-IPO asset".to_string(),
        symbol: "mPreIPO".to_string(),
        oracle_feeder: HumanAddr::from("feeder0000"),
        params: Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(1000),
            weight: Some(100u32),
            mint_period: Some(1000u64),
            min_collateral_ratio_after_migration: Some(Decimal::percent(150)),
        },
    };
    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::TokenCreationHook {
        oracle_feeder: HumanAddr::from("feeder0000"),
    };
    let env = mock_env("preIPOasset0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::TerraswapCreationHook {
        asset_token: HumanAddr::from("preIPOasset0000"),
    };
    let env = mock_env("terraswapfactory", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // register queriers
    deps.querier.with_mint_configs(&[(
        &HumanAddr::from("preIPOasset0000"),
        &(
            Decimal::percent(5),
            Decimal::percent(1000),
            Some(Decimal::percent(150)),
        ),
    )]);
    deps.querier.with_oracle_feeders(&[(
        &HumanAddr::from("preIPOasset0000"),
        &HumanAddr::from("feeder0000"),
    )]);

    // migration triggered by feeder
    let msg = HandleMsg::MigrateAsset {
        name: "Post-IPO asset".to_string(),
        symbol: "mPostIPO".to_string(),
        from_token: HumanAddr::from("preIPOasset0000"),
        end_price: Decimal::from_ratio(2u128, 1u128), // give first IPO price
    };

    let env = mock_env("feeder0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("mint0000"),
                send: vec![],
                msg: to_binary(&MintHandleMsg::RegisterMigration {
                    asset_token: HumanAddr::from("preIPOasset0000"),
                    end_price: Decimal::from_ratio(2u128, 1u128),
                })
                .unwrap(),
            }),
            CosmosMsg::Wasm(WasmMsg::Instantiate {
                code_id: TOKEN_CODE_ID,
                send: vec![],
                label: None,
                msg: to_binary(&TokenInitMsg {
                    name: "Post-IPO asset".to_string(),
                    symbol: "mPostIPO".to_string(),
                    decimals: 6u8,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: HumanAddr::from("mint0000"),
                        cap: None,
                    }),
                    init_hook: Some(InitHook {
                        contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                        msg: to_binary(&HandleMsg::TokenCreationHook {
                            oracle_feeder: HumanAddr::from("feeder0000") // same feeder
                        })
                        .unwrap(),
                    }),
                })
                .unwrap(),
            })
        ]
    );

    let msg = HandleMsg::TokenCreationHook {
        oracle_feeder: HumanAddr::from("feeder0000"),
    };
    let env = mock_env("postIPOasset", &[]);
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("mint0000"),
                send: vec![],
                msg: to_binary(&MintHandleMsg::RegisterAsset {
                    asset_token: HumanAddr::from("postIPOasset"),
                    auction_discount: Decimal::percent(5),
                    min_collateral_ratio: Decimal::percent(150), // new collateral ratio
                    mint_end: None,                              // reset to None
                    min_collateral_ratio_after_migration: None,  // reset to None
                })
                .unwrap(),
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("oracle0000"),
                send: vec![],
                msg: to_binary(&OracleHandleMsg::RegisterAsset {
                    asset_token: HumanAddr::from("postIPOasset"),
                    feeder: HumanAddr::from("feeder0000"),
                })
                .unwrap(),
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("terraswapfactory"),
                send: vec![],
                msg: to_binary(&TerraswapFactoryHandleMsg::CreatePair {
                    asset_infos: [
                        AssetInfo::NativeToken {
                            denom: BASE_DENOM.to_string(),
                        },
                        AssetInfo::Token {
                            contract_addr: HumanAddr::from("postIPOasset"),
                        },
                    ],
                    init_hook: Some(InitHook {
                        msg: to_binary(&HandleMsg::TerraswapCreationHook {
                            asset_token: HumanAddr::from("postIPOasset"),
                        })
                        .unwrap(),
                        contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                    }),
                })
                .unwrap(),
            })
        ]
    );
}
