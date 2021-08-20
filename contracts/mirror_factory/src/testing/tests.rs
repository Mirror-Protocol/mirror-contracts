use crate::contract::{execute, instantiate, query, reply};
use crate::response::MsgInstantiateContractResponse;
use crate::testing::mock_querier::{mock_dependencies, WasmMockQuerier};

use crate::state::{
    read_params, read_tmp_asset, read_tmp_oracle, read_total_weight, read_weight,
    store_total_weight, store_weight,
};
use cosmwasm_std::testing::{mock_env, mock_info, MockApi, MockStorage};
use cosmwasm_std::{
    attr, from_binary, to_binary, CanonicalAddr, ContractResult, CosmosMsg, Decimal, Env,
    OwnedDeps, Reply, ReplyOn, StdError, SubMsg, Timestamp, Uint128, WasmMsg,
};
use cosmwasm_std::{Api, SubMsgExecutionResponse};
use cw20::{Cw20ExecuteMsg, MinterResponse};

use mirror_protocol::factory::{
    ConfigResponse, DistributionInfoResponse, ExecuteMsg, InstantiateMsg, Params, QueryMsg,
};
use mirror_protocol::mint::{ExecuteMsg as MintExecuteMsg, IPOParams};
use mirror_protocol::oracle::ExecuteMsg as OracleExecuteMsg;
use mirror_protocol::staking::Cw20HookMsg as StakingCw20HookMsg;
use mirror_protocol::staking::ExecuteMsg as StakingExecuteMsg;

use protobuf::Message;

use terraswap::asset::AssetInfo;
use terraswap::factory::ExecuteMsg as TerraswapFactoryExecuteMsg;
use terraswap::token::InstantiateMsg as TokenInstantiateMsg;

fn mock_env_time(time: u64) -> Env {
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(time);
    env
}

static TOKEN_CODE_ID: u64 = 10u64;
static BASE_DENOM: &str = "uusd";

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);
    deps.querier
        .with_terraswap_pairs(&[(&"uusdmirror0000".to_string(), &"MIRLP0000".to_string())]);

    let msg = InstantiateMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::PostInitialize {
        owner: "owner0000".to_string(),
        mirror_token: "mirror0000".to_string(),
        mint_contract: "mint0000".to_string(),
        staking_contract: "staking0000".to_string(),
        commission_collector: "collector0000".to_string(),
        oracle_contract: "oracle0000".to_string(),
        terraswap_factory: "terraswapfactory".to_string(),
    };
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    // cannot update mirror token after initialization
    let msg = ExecuteMsg::PostInitialize {
        owner: "owner0001".to_string(),
        mirror_token: "mirror0001".to_string(),
        mint_contract: "mint0001".to_string(),
        staking_contract: "staking0001".to_string(),
        commission_collector: "collector0001".to_string(),
        oracle_contract: "oracle0001".to_string(),
        terraswap_factory: "terraswapfactory".to_string(),
    };
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();

    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            owner: "owner0000".to_string(),
            mirror_token: "mirror0000".to_string(),
            mint_contract: "mint0000".to_string(),
            staking_contract: "staking0000".to_string(),
            commission_collector: "collector0000".to_string(),
            oracle_contract: "oracle0000".to_string(),
            terraswap_factory: "terraswapfactory".to_string(),
            base_denom: BASE_DENOM.to_string(),
            token_code_id: TOKEN_CODE_ID,
            genesis_time: 1_571_797_419,
            distribution_schedule: vec![],
        }
    );
}

#[test]
fn test_update_config() {
    let mut deps = mock_dependencies(&[]);
    deps.querier
        .with_terraswap_pairs(&[(&"uusdmirror0000".to_string(), &"MIRLP0000".to_string())]);

    let msg = InstantiateMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::PostInitialize {
        owner: "owner0000".to_string(),
        mirror_token: "mirror0000".to_string(),
        mint_contract: "mint0000".to_string(),
        staking_contract: "staking0000".to_string(),
        commission_collector: "collector0000".to_string(),
        oracle_contract: "oracle0000".to_string(),
        terraswap_factory: "terraswapfactory".to_string(),
    };
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // upate owner
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("owner0001".to_string()),
        distribution_schedule: None,
        token_code_id: None,
    };

    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            owner: "owner0001".to_string(),
            mirror_token: "mirror0000".to_string(),
            mint_contract: "mint0000".to_string(),
            staking_contract: "staking0000".to_string(),
            commission_collector: "collector0000".to_string(),
            oracle_contract: "oracle0000".to_string(),
            terraswap_factory: "terraswapfactory".to_string(),
            base_denom: BASE_DENOM.to_string(),
            token_code_id: TOKEN_CODE_ID,
            genesis_time: 1_571_797_419,
            distribution_schedule: vec![],
        }
    );

    // update rest part
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        distribution_schedule: Some(vec![(1, 2, Uint128::from(123u128))]),
        token_code_id: Some(TOKEN_CODE_ID + 1),
    };

    let info = mock_info("owner0001", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            owner: "owner0001".to_string(),
            mirror_token: "mirror0000".to_string(),
            mint_contract: "mint0000".to_string(),
            staking_contract: "staking0000".to_string(),
            commission_collector: "collector0000".to_string(),
            oracle_contract: "oracle0000".to_string(),
            terraswap_factory: "terraswapfactory".to_string(),
            base_denom: BASE_DENOM.to_string(),
            token_code_id: TOKEN_CODE_ID + 1,
            genesis_time: 1_571_797_419,
            distribution_schedule: vec![(1, 2, Uint128::from(123u128))],
        }
    );

    // failed unauthoirzed
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        distribution_schedule: None,
        token_code_id: Some(TOKEN_CODE_ID + 1),
    };

    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }
}

#[test]
fn test_update_weight() {
    let mut deps = mock_dependencies(&[]);
    deps.querier
        .with_terraswap_pairs(&[(&"uusdmirror0000".to_string(), &"MIRLP0000".to_string())]);

    let msg = InstantiateMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::PostInitialize {
        owner: "owner0000".to_string(),
        mirror_token: "mirror0000".to_string(),
        mint_contract: "mint0000".to_string(),
        staking_contract: "staking0000".to_string(),
        commission_collector: "collector0000".to_string(),
        oracle_contract: "oracle0000".to_string(),
        terraswap_factory: "terraswapfactory".to_string(),
    };
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    store_total_weight(&mut deps.storage, 100).unwrap();
    store_weight(
        &mut deps.storage,
        &deps.api.addr_canonicalize("asset0000").unwrap(),
        10,
    )
    .unwrap();

    // increase weight
    let msg = ExecuteMsg::UpdateWeight {
        asset_token: "asset0000".to_string(),
        weight: 20,
    };
    let info = mock_info("owner0001", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let res = query(deps.as_ref(), mock_env(), QueryMsg::DistributionInfo {}).unwrap();
    let distribution_info: DistributionInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        distribution_info,
        DistributionInfoResponse {
            weights: vec![
                ("asset0000".to_string(), 20),
                ("mirror0000".to_string(), 300)
            ],
            last_distributed: 1_571_797_419,
        }
    );

    assert_eq!(
        read_weight(
            &deps.storage,
            &deps.api.addr_canonicalize("asset0000").unwrap()
        )
        .unwrap(),
        20u32
    );
    assert_eq!(read_total_weight(&deps.storage).unwrap(), 110u32)
}

#[test]
fn test_whitelist() {
    let mut deps = mock_dependencies(&[]);
    deps.querier
        .with_terraswap_pairs(&[(&"uusdmirror0000".to_string(), &"MIRLP0000".to_string())]);

    let msg = InstantiateMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::PostInitialize {
        owner: "owner0000".to_string(),
        mirror_token: "mirror0000".to_string(),
        mint_contract: "mint0000".to_string(),
        staking_contract: "staking0000".to_string(),
        commission_collector: "collector0000".to_string(),
        oracle_contract: "oracle0000".to_string(),
        terraswap_factory: "terraswapfactory".to_string(),
    };
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Whitelist {
        name: "apple derivative".to_string(),
        symbol: "mAPPL".to_string(),
        oracle_feeder: "feeder0000".to_string(),
        params: Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
            weight: Some(100u32),
            mint_period: None,
            min_collateral_ratio_after_ipo: None,
            pre_ipo_price: None,
        },
    };
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();

    assert_eq!(
        res.attributes,
        vec![
            attr("action", "whitelist"),
            attr("symbol", "mAPPL"),
            attr("name", "apple derivative")
        ]
    );

    // token creation msg should be returned
    assert_eq!(
        res.messages,
        vec![SubMsg {
            msg: WasmMsg::Instantiate {
                admin: None,
                code_id: TOKEN_CODE_ID,
                funds: vec![],
                label: "".to_string(),
                msg: to_binary(&TokenInstantiateMsg {
                    name: "apple derivative".to_string(),
                    symbol: "mAPPL".to_string(),
                    decimals: 6u8,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: "mint0000".to_string(),
                        cap: None,
                    }),
                })
                .unwrap(),
            }
            .into(),
            gas_limit: None,
            id: 1,
            reply_on: ReplyOn::Success,
        }]
    );

    let params: Params = read_params(&deps.storage).unwrap();
    assert_eq!(
        params,
        Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
            weight: Some(100u32),
            mint_period: None,
            min_collateral_ratio_after_ipo: None,
            pre_ipo_price: None,
        }
    );

    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "A whitelist process is in progress"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let info = mock_info("addr0001", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    //ensure temp oracle was stored
    let tmp_oracle = read_tmp_oracle(&deps.storage).unwrap();
    assert_eq!(tmp_oracle.to_string(), "feeder0000");
}

#[test]
fn test_token_creation_hook() {
    let mut deps = mock_dependencies(&[]);
    deps.querier
        .with_terraswap_pairs(&[(&"uusdmirror0000".to_string(), &"MIRLP0000".to_string())]);

    let msg = InstantiateMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::PostInitialize {
        owner: "owner0000".to_string(),
        mirror_token: "mirror0000".to_string(),
        mint_contract: "mint0000".to_string(),
        staking_contract: "staking0000".to_string(),
        commission_collector: "collector0000".to_string(),
        oracle_contract: "oracle0000".to_string(),
        terraswap_factory: "terraswapfactory".to_string(),
    };
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Whitelist {
        name: "apple derivative".to_string(),
        symbol: "mAPPL".to_string(),
        oracle_feeder: "feeder0000".to_string(),
        params: Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
            weight: Some(100u32),
            mint_period: None,
            min_collateral_ratio_after_ipo: None,
            pre_ipo_price: None,
        },
    };
    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    //ensure temp oracle was stored
    let tmp_oracle = read_tmp_oracle(&deps.storage).unwrap();
    assert_eq!(tmp_oracle.to_string(), "feeder0000");

    let mut token_inst_res = MsgInstantiateContractResponse::new();
    token_inst_res.set_contract_address("asset0000".to_string());

    let reply_msg = Reply {
        id: 1,
        result: ContractResult::Ok(SubMsgExecutionResponse {
            events: vec![],
            data: Some(token_inst_res.write_to_bytes().unwrap().into()),
        }),
    };

    let res = reply(deps.as_mut(), mock_env(), reply_msg).unwrap();

    //ensure asset token was stored
    let tmp_asset = read_tmp_asset(&deps.storage).unwrap();
    assert_eq!(tmp_asset.to_string(), "asset0000");

    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "mint0000".to_string(),
                funds: vec![],
                msg: to_binary(&MintExecuteMsg::RegisterAsset {
                    asset_token: "asset0000".to_string(),
                    auction_discount: Decimal::percent(5),
                    min_collateral_ratio: Decimal::percent(150),
                    ipo_params: None,
                })
                .unwrap(),
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "oracle0000".to_string(),
                funds: vec![],
                msg: to_binary(&OracleExecuteMsg::RegisterAsset {
                    asset_token: "asset0000".to_string(),
                    feeder: "feeder0000".to_string(),
                })
                .unwrap(),
            })),
            SubMsg {
                msg: WasmMsg::Execute {
                    contract_addr: "terraswapfactory".to_string(),
                    funds: vec![],
                    msg: to_binary(&TerraswapFactoryExecuteMsg::CreatePair {
                        asset_infos: [
                            AssetInfo::NativeToken {
                                denom: BASE_DENOM.to_string(),
                            },
                            AssetInfo::Token {
                                contract_addr: "asset0000".to_string(),
                            },
                        ],
                    })
                    .unwrap(),
                }
                .into(),
                gas_limit: None,
                id: 2,
                reply_on: ReplyOn::Success,
            }
        ]
    );

    assert_eq!(
        res.attributes,
        vec![
            attr("asset_token_addr", "asset0000"),
            attr("is_pre_ipo", "false"),
        ]
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::DistributionInfo {}).unwrap();
    let distribution_info: DistributionInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        distribution_info,
        DistributionInfoResponse {
            weights: vec![
                ("asset0000".to_string(), 100),
                ("mirror0000".to_string(), 300)
            ],
            last_distributed: 1_571_797_419,
        }
    );
}

#[test]
fn test_token_creation_hook_without_weight() {
    let mut deps = mock_dependencies(&[]);
    deps.querier
        .with_terraswap_pairs(&[(&"uusdmirror0000".to_string(), &"MIRLP0000".to_string())]);

    let msg = InstantiateMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::PostInitialize {
        owner: "owner0000".to_string(),
        mirror_token: "mirror0000".to_string(),
        mint_contract: "mint0000".to_string(),
        staking_contract: "staking0000".to_string(),
        commission_collector: "collector0000".to_string(),
        oracle_contract: "oracle0000".to_string(),
        terraswap_factory: "terraswapfactory".to_string(),
    };
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Whitelist {
        name: "apple derivative".to_string(),
        symbol: "mAPPL".to_string(),
        oracle_feeder: "feeder0000".to_string(),
        params: Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
            weight: None,
            mint_period: None,
            min_collateral_ratio_after_ipo: None,
            pre_ipo_price: None,
        },
    };
    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    //ensure temp oracle was stored
    let tmp_oracle = read_tmp_oracle(&deps.storage).unwrap();
    assert_eq!(tmp_oracle.to_string(), "feeder0000");

    let mut token_inst_res = MsgInstantiateContractResponse::new();
    token_inst_res.set_contract_address("asset0000".to_string());

    let reply_msg = Reply {
        id: 1,
        result: ContractResult::Ok(SubMsgExecutionResponse {
            events: vec![],
            data: Some(token_inst_res.write_to_bytes().unwrap().into()),
        }),
    };

    let res = reply(deps.as_mut(), mock_env(), reply_msg).unwrap();

    //ensure asset token was stored
    let tmp_asset = read_tmp_asset(&deps.storage).unwrap();
    assert_eq!(tmp_asset.to_string(), "asset0000");

    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "mint0000".to_string(),
                funds: vec![],
                msg: to_binary(&MintExecuteMsg::RegisterAsset {
                    asset_token: "asset0000".to_string(),
                    auction_discount: Decimal::percent(5),
                    min_collateral_ratio: Decimal::percent(150),
                    ipo_params: None,
                })
                .unwrap(),
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "oracle0000".to_string(),
                funds: vec![],
                msg: to_binary(&OracleExecuteMsg::RegisterAsset {
                    asset_token: "asset0000".to_string(),
                    feeder: "feeder0000".to_string(),
                })
                .unwrap(),
            })),
            SubMsg {
                msg: WasmMsg::Execute {
                    contract_addr: "terraswapfactory".to_string(),
                    funds: vec![],
                    msg: to_binary(&TerraswapFactoryExecuteMsg::CreatePair {
                        asset_infos: [
                            AssetInfo::NativeToken {
                                denom: BASE_DENOM.to_string(),
                            },
                            AssetInfo::Token {
                                contract_addr: "asset0000".to_string(),
                            },
                        ],
                    })
                    .unwrap(),
                }
                .into(),
                gas_limit: None,
                id: 2,
                reply_on: ReplyOn::Success,
            }
        ]
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::DistributionInfo {}).unwrap();
    let distribution_info: DistributionInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        distribution_info,
        DistributionInfoResponse {
            weights: vec![
                ("asset0000".to_string(), 30),
                ("mirror0000".to_string(), 300)
            ],
            last_distributed: 1_571_797_419,
        }
    );
}

#[test]
fn test_terraswap_creation_hook() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_terraswap_pairs(&[
        (&"uusdmirror0000".to_string(), &"MIRLP000".to_string()),
        (&"uusdasset0000".to_string(), &"LP0000".to_string()),
    ]);

    let msg = InstantiateMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::PostInitialize {
        owner: "owner0000".to_string(),
        mirror_token: "mirror0000".to_string(),
        mint_contract: "mint0000".to_string(),
        staking_contract: "staking0000".to_string(),
        commission_collector: "collector0000".to_string(),
        oracle_contract: "oracle0000".to_string(),
        terraswap_factory: "terraswapfactory".to_string(),
    };
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Whitelist {
        name: "apple derivative".to_string(),
        symbol: "mAPPL".to_string(),
        oracle_feeder: "feeder0000".to_string(),
        params: Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
            weight: Some(100u32),
            mint_period: None,
            min_collateral_ratio_after_ipo: None,
            pre_ipo_price: None,
        },
    };
    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    //ensure temp oracle was stored
    let tmp_oracle = read_tmp_oracle(&deps.storage).unwrap();
    assert_eq!(tmp_oracle.to_string(), "feeder0000");

    let mut token_inst_res = MsgInstantiateContractResponse::new();
    token_inst_res.set_contract_address("asset0000".to_string());

    let reply_msg = Reply {
        id: 1,
        result: ContractResult::Ok(SubMsgExecutionResponse {
            events: vec![],
            data: Some(token_inst_res.write_to_bytes().unwrap().into()),
        }),
    };

    let _res = reply(deps.as_mut(), mock_env(), reply_msg).unwrap();

    //ensure asset token was stored
    let tmp_asset = read_tmp_asset(&deps.storage).unwrap();
    assert_eq!(tmp_asset.to_string(), "asset0000");

    let reply_msg2 = Reply {
        id: 2,
        result: ContractResult::Ok(SubMsgExecutionResponse {
            events: vec![],
            data: None,
        }),
    };

    let res = reply(deps.as_mut(), mock_env(), reply_msg2).unwrap();

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "staking0000".to_string(),
            funds: vec![],
            msg: to_binary(&StakingExecuteMsg::RegisterAsset {
                asset_token: "asset0000".to_string(),
                staking_token: "LP0000".to_string(),
            })
            .unwrap(),
        }))]
    );
}

#[test]
fn test_distribute() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_terraswap_pairs(&[
        (&"uusdmirror0000".to_string(), &"MIRLP000".to_string()),
        (&"uusdasset0000".to_string(), &"LP0000".to_string()),
        (&"uusdasset0001".to_string(), &"LP0001".to_string()),
    ]);

    let msg = InstantiateMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![
            (1800, 3600, Uint128::from(3600u128)),
            (3600, 3600 + 3600, Uint128::from(7200u128)),
        ],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::PostInitialize {
        owner: "owner0000".to_string(),
        mirror_token: "mirror0000".to_string(),
        mint_contract: "mint0000".to_string(),
        staking_contract: "staking0000".to_string(),
        commission_collector: "collector0000".to_string(),
        oracle_contract: "oracle0000".to_string(),
        terraswap_factory: "terraswapfactory".to_string(),
    };
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // whitelist first item with weight 1.5
    let msg = ExecuteMsg::Whitelist {
        name: "apple derivative".to_string(),
        symbol: "mAPPL".to_string(),
        oracle_feeder: "feeder0000".to_string(),
        params: Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
            weight: Some(100u32),
            mint_period: None,
            min_collateral_ratio_after_ipo: None,
            pre_ipo_price: None,
        },
    };
    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    //ensure temp oracle was stored
    let tmp_oracle = read_tmp_oracle(&deps.storage).unwrap();
    assert_eq!(tmp_oracle.to_string(), "feeder0000");

    let mut token_inst_res = MsgInstantiateContractResponse::new();
    token_inst_res.set_contract_address("asset0000".to_string());

    let reply_msg = Reply {
        id: 1,
        result: ContractResult::Ok(SubMsgExecutionResponse {
            events: vec![],
            data: Some(token_inst_res.write_to_bytes().unwrap().into()),
        }),
    };

    let _res = reply(deps.as_mut(), mock_env(), reply_msg).unwrap();

    //ensure asset token was stored
    let tmp_asset = read_tmp_asset(&deps.storage).unwrap();
    assert_eq!(tmp_asset.to_string(), "asset0000");

    let reply_msg2 = Reply {
        id: 2,
        result: ContractResult::Ok(SubMsgExecutionResponse {
            events: vec![],
            data: None,
        }),
    };

    let _res = reply(deps.as_mut(), mock_env(), reply_msg2).unwrap();

    // Whitelist second item with weight 1
    let msg = ExecuteMsg::Whitelist {
        name: "google derivative".to_string(),
        symbol: "mGOGL".to_string(),
        oracle_feeder: "feeder0000".to_string(),
        params: Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
            weight: Some(100u32),
            mint_period: None,
            min_collateral_ratio_after_ipo: None,
            pre_ipo_price: None,
        },
    };
    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let mut token_inst_res = MsgInstantiateContractResponse::new();
    token_inst_res.set_contract_address("asset0001".to_string());

    let reply_msg = Reply {
        id: 1,
        result: ContractResult::Ok(SubMsgExecutionResponse {
            events: vec![],
            data: Some(token_inst_res.write_to_bytes().unwrap().into()),
        }),
    };

    let _res = reply(deps.as_mut(), mock_env(), reply_msg).unwrap();

    //ensure asset token was stored
    let tmp_asset = read_tmp_asset(&deps.storage).unwrap();
    assert_eq!(tmp_asset.to_string(), "asset0001");

    let reply_msg2 = Reply {
        id: 2,
        result: ContractResult::Ok(SubMsgExecutionResponse {
            events: vec![],
            data: None,
        }),
    };

    let _res = reply(deps.as_mut(), mock_env(), reply_msg2).unwrap();

    // height is not increased so zero amount will be minted
    let msg = ExecuteMsg::Distribute {};
    let info = mock_info("anyone", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "Cannot distribute mirror token before interval")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    // one height increase
    let msg = ExecuteMsg::Distribute {};
    let env = mock_env_time(1_571_797_419u64 + 5400u64);
    let info = mock_info("addr0000", &[]);

    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "distribute"),
            attr("distribution_amount", "7200"),
        ]
    );

    // MIR -> 7200 * 3/5
    // asset0000 -> 7200 * 1/5
    // asset0001 -> 7200 * 1/5
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "mirror0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "staking0000".to_string(),
                amount: Uint128::from(7200u128),
                msg: to_binary(&StakingCw20HookMsg::DepositReward {
                    rewards: vec![
                        ("asset0000".to_string(), Uint128::from(7200u128 / 5)),
                        ("asset0001".to_string(), Uint128::from(7200u128 / 5)),
                        ("mirror0000".to_string(), Uint128::from(7200u128 * 3 / 5)),
                    ],
                })
                .unwrap(),
            })
            .unwrap(),
            funds: vec![],
        }))],
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::DistributionInfo {}).unwrap();
    let distribution_info: DistributionInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        distribution_info,
        DistributionInfoResponse {
            weights: vec![
                ("asset0000".to_string(), 100),
                ("asset0001".to_string(), 100),
                ("mirror0000".to_string(), 300),
            ],
            last_distributed: 1_571_802_819,
        }
    );
}

fn whitelist_token(
    deps: &mut OwnedDeps<MockStorage, MockApi, WasmMockQuerier>,
    name: &str,
    symbol: &str,
    asset_token: &str,
    weight: u32,
) {
    // whitelist an asset with weight 1
    let msg = ExecuteMsg::Whitelist {
        name: name.to_string(),
        symbol: symbol.to_string(),
        oracle_feeder: "feeder0000".to_string(),
        params: Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(150),
            weight: Some(weight),
            mint_period: None,
            min_collateral_ratio_after_ipo: None,
            pre_ipo_price: None,
        },
    };
    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    //ensure temp oracle was stored
    let tmp_oracle = read_tmp_oracle(&deps.storage).unwrap();
    assert_eq!(tmp_oracle.to_string(), "feeder0000");

    // callback 1
    let mut token_inst_res = MsgInstantiateContractResponse::new();
    token_inst_res.set_contract_address(asset_token.to_string());

    let reply_msg = Reply {
        id: 1,
        result: ContractResult::Ok(SubMsgExecutionResponse {
            events: vec![],
            data: Some(token_inst_res.write_to_bytes().unwrap().into()),
        }),
    };

    let _res = reply(deps.as_mut(), mock_env(), reply_msg).unwrap();

    //ensure asset token was stored
    let tmp_asset = read_tmp_asset(&deps.storage).unwrap();
    assert_eq!(tmp_asset.to_string(), asset_token);

    // callback 2
    let reply_msg2 = Reply {
        id: 2,
        result: ContractResult::Ok(SubMsgExecutionResponse {
            events: vec![],
            data: None,
        }),
    };

    let _res = reply(deps.as_mut(), mock_env(), reply_msg2).unwrap();
}

#[test]
fn test_distribute_split() {
    let mut deps = mock_dependencies(&[]);

    let asset0 = deps
        .api
        .addr_humanize(&CanonicalAddr::from(vec![
            1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]))
        .unwrap()
        .to_string();
    let asset1 = deps
        .api
        .addr_humanize(&CanonicalAddr::from(vec![
            1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]))
        .unwrap()
        .to_string();
    let asset2 = deps
        .api
        .addr_humanize(&CanonicalAddr::from(vec![
            1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]))
        .unwrap()
        .to_string();
    let asset3 = deps
        .api
        .addr_humanize(&CanonicalAddr::from(vec![
            1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]))
        .unwrap()
        .to_string();
    let asset4 = deps
        .api
        .addr_humanize(&CanonicalAddr::from(vec![
            1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]))
        .unwrap()
        .to_string();
    let asset5 = deps
        .api
        .addr_humanize(&CanonicalAddr::from(vec![
            1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]))
        .unwrap()
        .to_string();
    let asset6 = deps
        .api
        .addr_humanize(&CanonicalAddr::from(vec![
            1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]))
        .unwrap()
        .to_string();
    let asset7 = deps
        .api
        .addr_humanize(&CanonicalAddr::from(vec![
            1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]))
        .unwrap()
        .to_string();
    let asset8 = deps
        .api
        .addr_humanize(&CanonicalAddr::from(vec![
            1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]))
        .unwrap()
        .to_string();
    let asset9 = deps
        .api
        .addr_humanize(&CanonicalAddr::from(vec![
            1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]))
        .unwrap()
        .to_string();
    let asset10 = deps
        .api
        .addr_humanize(&CanonicalAddr::from(vec![
            1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]))
        .unwrap()
        .to_string();
    let asset11 = deps
        .api
        .addr_humanize(&CanonicalAddr::from(vec![
            1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]))
        .unwrap()
        .to_string();
    let mirror_addr = deps
        .api
        .addr_humanize(&CanonicalAddr::from(vec![
            1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]))
        .unwrap()
        .to_string();

    deps.querier.with_terraswap_pairs(&[
        (&format!("uusd{}", asset0), &"LP0000".to_string()),
        (&format!("uusd{}", asset1), &"LP0001".to_string()),
        (&format!("uusd{}", asset2), &"LP0002".to_string()),
        (&format!("uusd{}", asset3), &"LP0003".to_string()),
        (&format!("uusd{}", asset4), &"LP0004".to_string()),
        (&format!("uusd{}", asset5), &"LP0005".to_string()),
        (&format!("uusd{}", asset6), &"LP0006".to_string()),
        (&format!("uusd{}", asset7), &"LP0007".to_string()),
        (&format!("uusd{}", asset8), &"LP0008".to_string()),
        (&format!("uusd{}", asset9), &"LP0009".to_string()),
        (&format!("uusd{}", asset10), &"LP0010".to_string()),
        (&format!("uusd{}", asset11), &"LP0011".to_string()),
        (&format!("uusd{}", mirror_addr), &"MIRLP000".to_string()),
    ]);

    let msg = InstantiateMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![
            (1800, 3600, Uint128::from(3600u128)),
            (3600, 3600 + 3600, Uint128::from(7200u128)),
        ],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::PostInitialize {
        owner: "owner0000".to_string(),
        mirror_token: mirror_addr.to_string(),
        mint_contract: "mint0000".to_string(),
        staking_contract: "staking0000".to_string(),
        commission_collector: "collector0000".to_string(),
        oracle_contract: "oracle0000".to_string(),
        terraswap_factory: "terraswapfactory".to_string(),
    };
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // whitelist first item with weight 1
    whitelist_token(&mut deps, "asset 0", "a0", &asset0, 100u32);
    whitelist_token(&mut deps, "asset 1", "a1", &asset1, 100u32);
    whitelist_token(&mut deps, "asset 2", "a2", &asset2, 100u32);
    whitelist_token(&mut deps, "asset 3", "a3", &asset3, 100u32);
    whitelist_token(&mut deps, "asset 4", "a4", &asset4, 100u32);
    whitelist_token(&mut deps, "asset 5", "a5", &asset5, 100u32);
    whitelist_token(&mut deps, "asset 6", "a6", &asset6, 100u32);
    whitelist_token(&mut deps, "asset 7", "a7", &asset7, 100u32);
    whitelist_token(&mut deps, "asset 8", "a8", &asset8, 100u32);
    whitelist_token(&mut deps, "asset 9", "a9", &asset9, 100u32);
    whitelist_token(&mut deps, "asset 10", "a10", &asset10, 100u32);
    whitelist_token(&mut deps, "asset 11", "a11", &asset11, 100u32);

    // one height increase
    let msg = ExecuteMsg::Distribute {};
    let env = mock_env_time(1_571_797_419u64 + 5400u64);
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "distribute"),
            attr("distribution_amount", "7200"),
        ]
    );

    // MIR = 7200 * 3/15
    // Other = 7200 * 1/15
    // Total first chunk = 7200 * 10/15
    // Total second chunk = 7200 * 5/15

    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: mirror_addr.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "staking0000".to_string(),
                amount: Uint128::from(7200u128 * 10 / 15),
                msg: to_binary(&StakingCw20HookMsg::DepositReward {
                    rewards: vec![
                        (asset0, Uint128::from(7200u128 / 15)),
                        (asset1, Uint128::from(7200u128 / 15)),
                        (asset2, Uint128::from(7200u128 / 15)),
                        (asset3, Uint128::from(7200u128 / 15)),
                        (asset4, Uint128::from(7200u128 / 15)),
                        (asset5, Uint128::from(7200u128 / 15)),
                        (asset6, Uint128::from(7200u128 / 15)),
                        (asset7, Uint128::from(7200u128 / 15)),
                        (asset8, Uint128::from(7200u128 / 15)),
                        (asset9, Uint128::from(7200u128 / 15)),
                    ],
                })
                .unwrap(),
            })
            .unwrap(),
            funds: vec![],
        })),
    );

    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: mirror_addr.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "staking0000".to_string(),
                amount: Uint128::from(7200u128 * 5 / 15),
                msg: to_binary(&StakingCw20HookMsg::DepositReward {
                    rewards: vec![
                        (asset10, Uint128::from(7200u128 / 15)),
                        (asset11, Uint128::from(7200u128 / 15)),
                        (mirror_addr, Uint128::from(7200u128 * 3 / 15)),
                    ],
                })
                .unwrap()
            })
            .unwrap(),
            funds: vec![],
        })),
    );
}

#[test]
fn test_revocation() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_terraswap_pairs(&[
        (&"uusdasset0000".to_string(), &"LP0000".to_string()),
        (&"uusdasset0001".to_string(), &"LP0001".to_string()),
        (&"uusdmirror0000".to_string(), &"MIRLP000".to_string()),
    ]);
    deps.querier.with_oracle_price(&[
        (&"uusd".to_string(), &Decimal::one()),
        (&"asset0001".to_string(), &Decimal::percent(200)),
    ]);
    deps.querier.with_mint_configs(&[(
        &"asset0001".to_string(),
        &(Decimal::percent(1), Decimal::percent(1)),
    )]);

    let msg = InstantiateMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::PostInitialize {
        owner: "owner0000".to_string(),
        mirror_token: "mirror0000".to_string(),
        mint_contract: "mint0000".to_string(),
        staking_contract: "staking0000".to_string(),
        commission_collector: "collector0000".to_string(),
        oracle_contract: "oracle0000".to_string(),
        terraswap_factory: "terraswapfactory".to_string(),
    };
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    whitelist_token(&mut deps, "tesla derivative", "mTSLA", "asset0000", 100u32);
    whitelist_token(&mut deps, "apple derivative", "mAPPL", "asset0001", 100u32);

    // register queriers
    deps.querier.with_oracle_feeders(&[
        (&"asset0000".to_string(), &"feeder0000".to_string()),
        (&"asset0001".to_string(), &"feeder0000".to_string()),
    ]);

    // unauthorized revoke attempt
    let msg = ExecuteMsg::RevokeAsset {
        asset_token: "asset0000".to_string(),
        end_price: Decimal::from_ratio(2u128, 1u128),
    };
    let info = mock_info("address0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    match err {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    // unatuthorized attemt 2, only owner can fix set price
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    match err {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    // SUCCESS - the feeder revokes item 1
    let info = mock_info("feeder0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "mint0000".to_string(),
            funds: vec![],
            msg: to_binary(&MintExecuteMsg::RegisterMigration {
                asset_token: "asset0000".to_string(),
                end_price: Decimal::from_ratio(2u128, 1u128),
            })
            .unwrap(),
        }))]
    );

    let msg = ExecuteMsg::RevokeAsset {
        asset_token: "asset0001".to_string(),
        end_price: Decimal::from_ratio(2u128, 1u128),
    };
    // SUCCESS - the owner revokes item 2
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "mint0000".to_string(),
            funds: vec![],
            msg: to_binary(&MintExecuteMsg::RegisterMigration {
                asset_token: "asset0001".to_string(),
                end_price: Decimal::from_ratio(2u128, 1u128),
            })
            .unwrap(),
        }))]
    );
}

#[test]
fn test_migration() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_terraswap_pairs(&[
        (&"uusdasset0000".to_string(), &"LP0000".to_string()),
        (&"uusdmirror0000".to_string(), &"MIRLP000".to_string()),
    ]);

    let msg = InstantiateMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::PostInitialize {
        owner: "owner0000".to_string(),
        mirror_token: "mirror0000".to_string(),
        mint_contract: "mint0000".to_string(),
        staking_contract: "staking0000".to_string(),
        commission_collector: "collector0000".to_string(),
        oracle_contract: "oracle0000".to_string(),
        terraswap_factory: "terraswapfactory".to_string(),
    };
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    whitelist_token(&mut deps, "apple derivative", "mAPPL", "asset0000", 100u32);

    // register queriers
    deps.querier.with_mint_configs(&[(
        &"asset0000".to_string(),
        &(Decimal::percent(1), Decimal::percent(1)),
    )]);
    deps.querier
        .with_oracle_feeders(&[(&"asset0000".to_string(), &"feeder0000".to_string())]);

    // unauthorized migrate attempt
    let msg = ExecuteMsg::MigrateAsset {
        name: "apple migration".to_string(),
        symbol: "mAPPL2".to_string(),
        from_token: "asset0000".to_string(),
        end_price: Decimal::from_ratio(2u128, 1u128),
    };
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();

    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let info = mock_info("feeder0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "mint0000".to_string(),
                funds: vec![],
                msg: to_binary(&MintExecuteMsg::RegisterMigration {
                    asset_token: "asset0000".to_string(),
                    end_price: Decimal::from_ratio(2u128, 1u128),
                })
                .unwrap(),
            })),
            SubMsg {
                msg: WasmMsg::Instantiate {
                    admin: None,
                    code_id: TOKEN_CODE_ID,
                    funds: vec![],
                    label: "".to_string(),
                    msg: to_binary(&TokenInstantiateMsg {
                        name: "apple migration".to_string(),
                        symbol: "mAPPL2".to_string(),
                        decimals: 6u8,
                        initial_balances: vec![],
                        mint: Some(MinterResponse {
                            minter: "mint0000".to_string(),
                            cap: None,
                        }),
                    })
                    .unwrap(),
                }
                .into(),
                gas_limit: None,
                id: 1,
                reply_on: ReplyOn::Success,
            }
        ]
    );
}

#[test]
fn test_whitelist_pre_ipo_asset() {
    let mut deps = mock_dependencies(&[]);
    deps.querier
        .with_terraswap_pairs(&[(&"uusdmirror0000".to_string(), &"MIRLP0000".to_string())]);

    let msg = InstantiateMsg {
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::PostInitialize {
        owner: "owner0000".to_string(),
        mirror_token: "mirror0000".to_string(),
        mint_contract: "mint0000".to_string(),
        staking_contract: "staking0000".to_string(),
        commission_collector: "collector0000".to_string(),
        oracle_contract: "oracle0000".to_string(),
        terraswap_factory: "terraswapfactory".to_string(),
    };
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Whitelist {
        name: "pre-IPO asset".to_string(),
        symbol: "mPreIPO".to_string(),
        oracle_feeder: "feeder0000".to_string(),
        params: Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(1000),
            weight: Some(100u32),
            mint_period: Some(10000u64),
            min_collateral_ratio_after_ipo: Some(Decimal::percent(150)),
            pre_ipo_price: Some(Decimal::percent(1)),
        },
    };
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // token creation submsg should be returned
    assert_eq!(
        res.messages,
        vec![SubMsg {
            msg: WasmMsg::Instantiate {
                admin: None,
                code_id: TOKEN_CODE_ID,
                funds: vec![],
                label: "".to_string(),
                msg: to_binary(&TokenInstantiateMsg {
                    name: "pre-IPO asset".to_string(),
                    symbol: "mPreIPO".to_string(),
                    decimals: 6u8,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: "mint0000".to_string(),
                        cap: None,
                    }),
                })
                .unwrap(),
            }
            .into(),
            gas_limit: None,
            id: 1,
            reply_on: ReplyOn::Success,
        }]
    );

    let params: Params = read_params(&deps.storage).unwrap();
    assert_eq!(
        params,
        Params {
            auction_discount: Decimal::percent(5),
            min_collateral_ratio: Decimal::percent(1000),
            weight: Some(100u32),
            mint_period: Some(10000u64),
            min_collateral_ratio_after_ipo: Some(Decimal::percent(150)),
            pre_ipo_price: Some(Decimal::percent(1)),
        }
    );

    //ensure temp oracle was stored
    let tmp_oracle = read_tmp_oracle(&deps.storage).unwrap();
    assert_eq!(tmp_oracle.to_string(), "feeder0000");

    // callback 1
    let mut token_inst_res = MsgInstantiateContractResponse::new();
    token_inst_res.set_contract_address("asset0000".to_string());

    let reply_msg = Reply {
        id: 1,
        result: ContractResult::Ok(SubMsgExecutionResponse {
            events: vec![],
            data: Some(token_inst_res.write_to_bytes().unwrap().into()),
        }),
    };

    let res = reply(deps.as_mut(), mock_env(), reply_msg).unwrap();

    //ensure asset token was stored
    let tmp_asset = read_tmp_asset(&deps.storage).unwrap();
    assert_eq!(tmp_asset.to_string(), "asset0000");

    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "mint0000".to_string(),
                funds: vec![],
                msg: to_binary(&MintExecuteMsg::RegisterAsset {
                    asset_token: "asset0000".to_string(),
                    auction_discount: Decimal::percent(5),
                    min_collateral_ratio: Decimal::percent(1000),
                    ipo_params: Some(IPOParams {
                        mint_end: mock_env().block.time.plus_seconds(10000u64).nanos()
                            / 1_000_000_000,
                        min_collateral_ratio_after_ipo: Decimal::percent(150),
                        pre_ipo_price: Decimal::percent(1),
                    }),
                })
                .unwrap(),
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "oracle0000".to_string(),
                funds: vec![],
                msg: to_binary(&OracleExecuteMsg::RegisterAsset {
                    asset_token: "asset0000".to_string(),
                    feeder: "feeder0000".to_string(),
                })
                .unwrap(),
            })),
            SubMsg {
                msg: WasmMsg::Execute {
                    contract_addr: "terraswapfactory".to_string(),
                    funds: vec![],
                    msg: to_binary(&TerraswapFactoryExecuteMsg::CreatePair {
                        asset_infos: [
                            AssetInfo::NativeToken {
                                denom: BASE_DENOM.to_string(),
                            },
                            AssetInfo::Token {
                                contract_addr: "asset0000".to_string(),
                            },
                        ],
                    })
                    .unwrap(),
                }
                .into(),
                gas_limit: None,
                id: 2,
                reply_on: ReplyOn::Success,
            }
        ]
    );

    assert_eq!(
        res.attributes,
        vec![
            attr("asset_token_addr", "asset0000"),
            attr("is_pre_ipo", "true"),
            attr(
                "mint_end",
                (mock_env().block.time.plus_seconds(10000u64).seconds()).to_string()
            ),
            attr("min_collateral_ratio_after_ipo", "1.5"),
            attr("pre_ipo_price", "0.01"),
        ]
    );
}
