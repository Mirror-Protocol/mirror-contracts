use crate::contract::{execute, instantiate, query};
use crate::error::ContractError;
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{
    attr, from_binary, to_binary, BlockInfo, CosmosMsg, Empty, Env, SubMsg, Timestamp, WasmMsg,
};
use mirror_protocol::admin_manager::{
    AuthRecordResponse, AuthRecordsResponse, ConfigResponse, ExecuteMsg, InstantiateMsg,
    MigrationItem, MigrationRecordResponse, MigrationRecordsResponse, QueryMsg,
};

fn mock_env_with_block_time(time: u64) -> Env {
    let env = mock_env();
    // register time
    Env {
        block: BlockInfo {
            height: 1,
            time: Timestamp::from_seconds(time),
            chain_id: "columbus".to_string(),
        },
        ..env
    }
}

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        admin_claim_period: 100u64,
    };
    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            owner: "owner0000".to_string(),
            admin_claim_period: 100u64,
        }
    )
}

#[test]
fn update_owner() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        admin_claim_period: 100u64,
    };
    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // update owner, unauth attempt
    let info = mock_info("addr0000", &[]);
    let msg = ExecuteMsg::UpdateOwner {
        owner: "owner0001".to_string(),
    };
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // successfull attempt
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.attributes, vec![attr("action", "update_owner")]);

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            owner: "owner0001".to_string(),
            admin_claim_period: 100u64,
        }
    )
}

#[test]
fn execute_migrations() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        admin_claim_period: 100u64,
    };
    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::ExecuteMigrations {
        migrations: vec![
            (
                "contract0000".to_string(),
                12u64,
                to_binary(&Empty {}).unwrap(),
            ),
            (
                "contract0001".to_string(),
                13u64,
                to_binary(&Empty {}).unwrap(),
            ),
        ],
    };

    // unauthorized attempt
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // successfull attempt
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.attributes, vec![attr("action", "execute_migrations"),]);

    let res: MigrationRecordsResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::MigrationRecords {
                start_after: None,
                limit: None,
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        res,
        MigrationRecordsResponse {
            records: vec![MigrationRecordResponse {
                executor: "owner0000".to_string(),
                time: mock_env().block.time.seconds(),
                migrations: vec![
                    MigrationItem {
                        contract: "contract0000".to_string(),
                        new_code_id: 12u64,
                        msg: to_binary(&Empty {}).unwrap(),
                    },
                    MigrationItem {
                        contract: "contract0001".to_string(),
                        new_code_id: 13u64,
                        msg: to_binary(&Empty {}).unwrap(),
                    }
                ],
            }]
        }
    );
}

#[test]
fn authorize_claim() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        admin_claim_period: 100u64,
    };
    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::AuthorizeClaim {
        authorized_addr: "auth0000".to_string(),
    };

    // unauthorized attempt
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // successfull attempt
    let info = mock_info("owner0000", &[]);
    let env = mock_env_with_block_time(10u64);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "authorize_claim"),
            attr("claim_start", "10"),
            attr("claim_end", "110"), // 10 + 100
        ]
    );

    let res: AuthRecordsResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::AuthRecords {
                start_after: None,
                limit: None,
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        res,
        AuthRecordsResponse {
            records: vec![AuthRecordResponse {
                address: "auth0000".to_string(),
                start_time: 10u64,
                end_time: 110u64,
            }]
        }
    );
}

#[test]
fn claim_admin() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        admin_claim_period: 100u64,
    };
    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::AuthorizeClaim {
        authorized_addr: "auth0000".to_string(),
    };

    let info = mock_info("owner0000", &[]);
    let env = mock_env_with_block_time(10u64);
    execute(deps.as_mut(), env, info, msg).unwrap();

    let msg = ExecuteMsg::ClaimAdmin {
        contract: "contract0000".to_string(),
    };

    // unauthorized attempt
    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // claim after claim_end, exepct unauthorized error
    let info = mock_info("auth0000", &[]);
    let env = mock_env_with_block_time(111u64);
    let err = execute(deps.as_mut(), env, info.clone(), msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // successful attempt
    let env = mock_env_with_block_time(109u64);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim_admin"),
            attr("contract", "contract0000"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::UpdateAdmin {
            contract_addr: "contract0000".to_string(),
            admin: "auth0000".to_string(),
        }))]
    )
}

#[test]
fn query_auth_records() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        admin_claim_period: 100u64,
    };
    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::AuthorizeClaim {
        authorized_addr: "auth0000".to_string(),
    };
    let info = mock_info("owner0000", &[]);
    let env = mock_env_with_block_time(10u64);
    execute(deps.as_mut(), env, info.clone(), msg).unwrap();

    let msg = ExecuteMsg::AuthorizeClaim {
        authorized_addr: "auth0001".to_string(),
    };
    let env = mock_env_with_block_time(20u64);
    execute(deps.as_mut(), env, info.clone(), msg).unwrap();

    let msg = ExecuteMsg::AuthorizeClaim {
        authorized_addr: "auth0002".to_string(),
    };
    let env = mock_env_with_block_time(30u64);
    execute(deps.as_mut(), env, info, msg).unwrap();

    let res: AuthRecordsResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::AuthRecords {
                start_after: None,
                limit: None,
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        res,
        AuthRecordsResponse {
            records: vec![
                AuthRecordResponse {
                    address: "auth0002".to_string(),
                    start_time: 30u64,
                    end_time: 130u64,
                },
                AuthRecordResponse {
                    address: "auth0001".to_string(),
                    start_time: 20u64,
                    end_time: 120u64,
                },
                AuthRecordResponse {
                    address: "auth0000".to_string(),
                    start_time: 10u64,
                    end_time: 110u64,
                }
            ]
        }
    );

    let res: AuthRecordsResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::AuthRecords {
                start_after: Some(21u64),
                limit: Some(1u32),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        res,
        AuthRecordsResponse {
            records: vec![AuthRecordResponse {
                address: "auth0001".to_string(),
                start_time: 20u64,
                end_time: 120u64,
            },]
        }
    );
}

#[test]
fn query_migration_records() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        admin_claim_period: 100u64,
    };
    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::ExecuteMigrations {
        migrations: vec![(
            "contract0000".to_string(),
            12u64,
            to_binary(&Empty {}).unwrap(),
        )],
    };
    let info = mock_info("owner0000", &[]);
    let env = mock_env_with_block_time(10u64);
    execute(deps.as_mut(), env, info.clone(), msg).unwrap();

    let msg = ExecuteMsg::ExecuteMigrations {
        migrations: vec![(
            "contract0001".to_string(),
            13u64,
            to_binary(&Empty {}).unwrap(),
        )],
    };
    let env = mock_env_with_block_time(20u64);
    execute(deps.as_mut(), env, info.clone(), msg).unwrap();

    let msg = ExecuteMsg::ExecuteMigrations {
        migrations: vec![(
            "contract0002".to_string(),
            14u64,
            to_binary(&Empty {}).unwrap(),
        )],
    };
    let env = mock_env_with_block_time(30u64);
    execute(deps.as_mut(), env, info, msg).unwrap();

    let res: MigrationRecordsResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::MigrationRecords {
                start_after: None,
                limit: None,
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        res,
        MigrationRecordsResponse {
            records: vec![
                MigrationRecordResponse {
                    executor: "owner0000".to_string(),
                    time: 30u64,
                    migrations: vec![MigrationItem {
                        contract: "contract0002".to_string(),
                        new_code_id: 14u64,
                        msg: to_binary(&Empty {}).unwrap(),
                    },],
                },
                MigrationRecordResponse {
                    executor: "owner0000".to_string(),
                    time: 20u64,
                    migrations: vec![MigrationItem {
                        contract: "contract0001".to_string(),
                        new_code_id: 13u64,
                        msg: to_binary(&Empty {}).unwrap(),
                    },],
                },
                MigrationRecordResponse {
                    executor: "owner0000".to_string(),
                    time: 10u64,
                    migrations: vec![MigrationItem {
                        contract: "contract0000".to_string(),
                        new_code_id: 12u64,
                        msg: to_binary(&Empty {}).unwrap(),
                    },],
                },
            ]
        }
    );

    let res: MigrationRecordsResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::MigrationRecords {
                start_after: Some(21u64),
                limit: Some(1u32),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        res,
        MigrationRecordsResponse {
            records: vec![MigrationRecordResponse {
                executor: "owner0000".to_string(),
                time: 20u64,
                migrations: vec![MigrationItem {
                    contract: "contract0001".to_string(),
                    new_code_id: 13u64,
                    msg: to_binary(&Empty {}).unwrap(),
                },],
            },]
        }
    );
}
