use crate::contract::{handle, init, query};
use crate::mock_querier::mock_dependencies;
use crate::msg::{
    ConfigResponse, DistributionInfoResponse, HandleMsg, InitMsg, QueryMsg, StakingCw20HookMsg,
};
use crate::register_msgs::*;
use crate::state::{read_params, Params};
use cosmwasm_std::testing::{mock_env, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_binary, log, to_binary, CosmosMsg, Decimal, Env, HumanAddr, StdError, Uint128, WasmMsg,
};
use cw20::{Cw20HandleMsg, MinterResponse};
use terraswap::{AssetInfo, InitHook, TokenInitMsg};

fn mock_env_height(signer: &HumanAddr, height: u64) -> Env {
    let mut env = mock_env(signer, &[]);
    env.block.height = height;
    env
}

static TOKEN_CODE_ID: u64 = 10u64;
static BASE_DENOM: &str = "uusd";

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        mint_per_block: Uint128(100u128),
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
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
            mint_per_block: Uint128(100u128),
            base_denom: BASE_DENOM.to_string(),
            token_code_id: TOKEN_CODE_ID,
        }
    );
}

#[test]
fn test_update_config() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        mint_per_block: Uint128(100u128),
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
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
        mint_per_block: None,
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
            mint_per_block: Uint128(100u128),
            base_denom: BASE_DENOM.to_string(),
            token_code_id: TOKEN_CODE_ID,
        }
    );

    // update rest part
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        mint_per_block: Some(Uint128(200u128)),
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
            mint_per_block: Uint128(200u128),
            base_denom: BASE_DENOM.to_string(),
            token_code_id: TOKEN_CODE_ID + 1,
        }
    );

    // failed unauthoirzed
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        mint_per_block: Some(Uint128(200u128)),
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
fn test_whitelist() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        mint_per_block: Uint128(100u128),
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
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
            weight: Decimal::from_ratio(15u64, 10u64),
            lp_commission: Decimal::percent(1),
            owner_commission: Decimal::percent(1),
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
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
            weight: Decimal::from_ratio(15u64, 10u64),
            lp_commission: Decimal::percent(1),
            owner_commission: Decimal::percent(1),
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
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
        mint_per_block: Uint128(100u128),
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
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
            assert_eq!(msg, "There is no whitelist process in progress")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = HandleMsg::Whitelist {
        name: "apple derivative".to_string(),
        symbol: "mAPPL".to_string(),
        oracle_feeder: HumanAddr::from("feeder0000"),
        params: Params {
            weight: Decimal::from_ratio(15u64, 10u64),
            lp_commission: Decimal::percent(1),
            owner_commission: Decimal::percent(1),
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
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
                msg: to_binary(&TerraswapHandleMsg::CreatePair {
                    pair_owner: env.contract.address,
                    commission_collector: HumanAddr::from("collector0000"),
                    lp_commission: Decimal::percent(1),
                    owner_commission: Decimal::percent(1),
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

    let res = query(
        &deps,
        QueryMsg::DistributionInfo {
            asset_token: HumanAddr::from("asset0000"),
        },
    )
    .unwrap();
    let distribution_info: DistributionInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        distribution_info,
        DistributionInfoResponse {
            weight: Decimal::from_ratio(15u64, 10u64),
            last_height: 12345,
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
            assert_eq!(msg, "There is no whitelist process in progress")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }
}

#[test]
fn test_terraswap_creation_hook() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_terraswap_pairs(&[(
        &"asset0000\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}uusd".to_string(),
        &HumanAddr::from("pair0000"),
    )]);
    deps.querier.with_terraswap_pair_staking_token(&[(
        &HumanAddr::from("pair0000"),
        &HumanAddr::from("LP0000"),
    )]);

    let msg = InitMsg {
        mint_per_block: Uint128(100u128),
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
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
            weight: Decimal::from_ratio(15u64, 10u64),
            lp_commission: Decimal::percent(1),
            owner_commission: Decimal::percent(1),
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
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
fn test_update_weight() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_terraswap_pairs(&[(
        &"asset0000\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}uusd".to_string(),
        &HumanAddr::from("pair0000"),
    )]);
    deps.querier.with_terraswap_pair_staking_token(&[(
        &HumanAddr::from("pair0000"),
        &HumanAddr::from("LP0000"),
    )]);

    let msg = InitMsg {
        mint_per_block: Uint128(100u128),
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
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
            weight: Decimal::from_ratio(15u64, 10u64),
            lp_commission: Decimal::percent(1),
            owner_commission: Decimal::percent(1),
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
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

    let msg = HandleMsg::UpdateWeight {
        asset_token: HumanAddr::from("asset0000"),
        weight: Decimal::from_ratio(2u64, 1u64),
    };
    let env = mock_env("owner0001", &[]);
    let res = handle(&mut deps, env, msg.clone());
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("SIBONG"),
    }

    let env = mock_env("owner0000", &[]);
    let res = handle(&mut deps, env.clone(), msg).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "update_weight"),
            log("asset_token", "asset0000"),
            log("weight", "2")
        ]
    );

    let res = query(
        &deps,
        QueryMsg::DistributionInfo {
            asset_token: HumanAddr::from("asset0000"),
        },
    )
    .unwrap();
    let distribution_info: DistributionInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        distribution_info,
        DistributionInfoResponse {
            weight: Decimal::from_ratio(2u64, 1u64),
            last_height: 12345,
        }
    );
}

#[test]
fn test_mint() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_terraswap_pairs(&[(
        &"asset0000\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}uusd".to_string(),
        &HumanAddr::from("pair0000"),
    )]);
    deps.querier.with_terraswap_pair_staking_token(&[(
        &HumanAddr::from("pair0000"),
        &HumanAddr::from("LP0000"),
    )]);

    let msg = InitMsg {
        mint_per_block: Uint128(100u128),
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
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
            weight: Decimal::from_ratio(15u64, 10u64),
            lp_commission: Decimal::percent(1),
            owner_commission: Decimal::percent(1),
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
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

    // height is not increased so zero amount will be minted
    let msg = HandleMsg::Mint {
        asset_token: HumanAddr::from("asset0000"),
    };
    let env = mock_env("anyone", &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "mint"),
            log("asset_token", "asset0000"),
            log("mint_amount", "0"),
        ]
    );

    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("mirror0000"),
                msg: to_binary(&Cw20HandleMsg::Mint {
                    recipient: HumanAddr::from(MOCK_CONTRACT_ADDR),
                    amount: Uint128::zero(),
                })
                .unwrap(),
                send: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("mirror0000"),
                msg: to_binary(&Cw20HandleMsg::Send {
                    contract: HumanAddr::from("staking0000"),
                    amount: Uint128::zero(),
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
        ],
    );

    // one height increase
    let msg = HandleMsg::Mint {
        asset_token: HumanAddr::from("asset0000"),
    };
    let env = mock_env_height(&HumanAddr::from("addr0000"), 12346u64);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "mint"),
            log("asset_token", "asset0000"),
            log("mint_amount", "150"),
        ]
    );

    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("mirror0000"),
                msg: to_binary(&Cw20HandleMsg::Mint {
                    recipient: HumanAddr::from(MOCK_CONTRACT_ADDR),
                    amount: Uint128(150u128),
                })
                .unwrap(),
                send: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("mirror0000"),
                msg: to_binary(&Cw20HandleMsg::Send {
                    contract: HumanAddr::from("staking0000"),
                    amount: Uint128(150u128),
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
        ],
    );
}
