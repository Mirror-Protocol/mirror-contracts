use cosmwasm_std::{
    log, to_binary, Api, Binary, Decimal, Env, Extern, HandleResponse, HandleResult, HumanAddr,
    InitResponse, Querier, StdError, StdResult, Storage,
};

use crate::msg::{
    AssetResponse, ConfigResponse, HandleMsg, InitMsg, PriceInfo, PriceResponse, PricesResponse,
    QueryMsg,
};

use crate::state::{
    read_asset_config, read_config, read_price, read_prices, remove_asset_config, remove_price,
    store_asset_config, store_config, store_price, AssetConfig, Config, PriceInfoRaw,
};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let base_asset_info = msg.base_asset_info.to_raw(&deps)?;
    store_config(
        &mut deps.storage,
        &Config {
            owner: deps.api.canonical_address(&msg.owner)?,
            base_asset_info,
        },
    )?;

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> HandleResult {
    match msg {
        HandleMsg::UpdateConfig { owner } => try_update_config(deps, env, owner),
        HandleMsg::RegisterAsset {
            asset_token,
            feeder,
        } => try_register_asset(deps, env, asset_token, feeder),
        HandleMsg::MigrateAsset {
            from_token,
            to_token,
        } => try_migrate_asset(deps, env, from_token, to_token),
        HandleMsg::FeedPrice { price_infos } => try_feed_price(deps, env, price_infos),
    }
}

pub fn try_update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: Option<HumanAddr>,
) -> HandleResult {
    let mut config: Config = read_config(&deps.storage)?;
    if deps.api.canonical_address(&env.message.sender)? != config.owner {
        return Err(StdError::unauthorized());
    }

    if let Some(owner) = owner {
        config.owner = deps.api.canonical_address(&owner)?;
    }

    store_config(&mut deps.storage, &config)?;
    Ok(HandleResponse::default())
}

pub fn try_register_asset<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: HumanAddr,
    feeder: HumanAddr,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    if config.owner != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    let asset_token_raw = deps.api.canonical_address(&asset_token)?;
    if read_asset_config(&deps.storage, &asset_token_raw).is_ok() {
        return Err(StdError::generic_err("Asset was already registered"));
    }

    store_price(
        &mut deps.storage,
        &asset_token_raw,
        &PriceInfoRaw {
            asset_token: asset_token_raw.clone(),
            price: Decimal::zero(),
            last_update_time: 0u64,
        },
    )?;

    store_asset_config(
        &mut deps.storage,
        &asset_token_raw,
        &AssetConfig {
            asset_token: asset_token_raw.clone(),
            feeder: deps.api.canonical_address(&feeder)?,
        },
    )?;

    Ok(HandleResponse::default())
}

pub fn try_migrate_asset<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    from_token: HumanAddr,
    to_token: HumanAddr,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    if config.owner != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    let from_token_raw = deps.api.canonical_address(&from_token)?;
    let to_token_raw = deps.api.canonical_address(&to_token)?;

    let from_asset_config = read_asset_config(&deps.storage, &from_token_raw)?;
    if read_asset_config(&deps.storage, &to_token_raw).is_ok() {
        return Err(StdError::generic_err("Asset was already registered"));
    }

    // remove from_asset config & price
    remove_asset_config(&mut deps.storage, &from_token_raw)?;
    remove_price(&mut deps.storage, &from_token_raw);
    // update assset config
    store_asset_config(
        &mut deps.storage,
        &to_token_raw,
        &AssetConfig {
            asset_token: to_token_raw.clone(),
            ..from_asset_config
        },
    )?;

    // reset price
    store_price(
        &mut deps.storage,
        &to_token_raw,
        &PriceInfoRaw {
            asset_token: to_token_raw.clone(),
            price: Decimal::zero(),
            last_update_time: 0u64,
        },
    )?;

    Ok(HandleResponse::default())
}

pub fn try_feed_price<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    price_infos: Vec<PriceInfo>,
) -> HandleResult {
    let feeder_raw = deps.api.canonical_address(&env.message.sender)?;

    let mut logs = vec![log("action", "price_feed")];
    for price_info in price_infos {
        logs.push(log("asset_info", price_info.asset_token.to_string()));
        logs.push(log("price", price_info.price.to_string()));

        let asset_token_raw = deps.api.canonical_address(&price_info.asset_token)?;
        let asset: AssetConfig = read_asset_config(&deps.storage, &asset_token_raw)?;

        // Check feeder permission
        if feeder_raw != asset.feeder {
            return Err(StdError::unauthorized());
        }

        let mut state: PriceInfoRaw = read_price(&deps.storage, &asset_token_raw)?;
        state.last_update_time = env.block.time;
        state.price = price_info.price;

        store_price(&mut deps.storage, &asset_token_raw, &state)?;
    }

    Ok(HandleResponse {
        messages: vec![],
        log: logs,
        data: None,
    })
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::Asset { asset_token } => to_binary(&query_asset(deps, asset_token)?),
        QueryMsg::Price { asset_token } => to_binary(&query_price(deps, asset_token)?),
        QueryMsg::Prices {} => to_binary(&query_prices(deps)),
    }
}

fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let state = read_config(&deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.human_address(&state.owner)?,
        base_asset_info: state.base_asset_info.to_normal(&deps)?,
    };

    Ok(resp)
}

fn query_asset<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    asset_token: HumanAddr,
) -> StdResult<AssetResponse> {
    let asset_token_raw = deps.api.canonical_address(&asset_token)?;
    let state = read_asset_config(&deps.storage, &asset_token_raw)?;
    let resp = AssetResponse {
        asset_token,
        feeder: deps.api.human_address(&state.feeder)?,
    };

    Ok(resp)
}

fn query_price<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    asset_token: HumanAddr,
) -> StdResult<PriceResponse> {
    let asset_token_raw = deps.api.canonical_address(&asset_token)?;
    let state = read_price(&deps.storage, &asset_token_raw)?;
    let resp = PriceResponse {
        asset_token,
        price: state.price,
        last_update_time: state.last_update_time,
    };

    Ok(resp)
}

fn query_prices<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<PricesResponse> {
    let states: Vec<PriceInfoRaw> = read_prices(&deps.storage)?;

    let mut prices: Vec<PriceResponse> = vec![];
    for state in states {
        prices.push(PriceResponse {
            asset_token: deps.api.human_address(&state.asset_token)?,
            price: state.price,
            last_update_time: state.last_update_time,
        });
    }

    Ok(PricesResponse { prices })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cosmwasm_std::StdError;
    use std::str::FromStr;
    use terraswap::AssetInfo;

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            owner: HumanAddr("owner0000".to_string()),
            base_asset_info: AssetInfo::NativeToken {
                denom: "base0000".to_string(),
            },
        };

        let env = mock_env("addr0000", &[]);

        // we can just call .unwrap() to assert this was a success
        let res = init(&mut deps, env, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let value = query_config(&deps).unwrap();
        assert_eq!("owner0000", value.owner.as_str());
        assert_eq!("base0000", &value.base_asset_info.to_string());
    }

    #[test]
    fn update_config() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            owner: HumanAddr("owner0000".to_string()),
            base_asset_info: AssetInfo::NativeToken {
                denom: "base0000".to_string(),
            },
        };

        let env = mock_env("addr0000", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        // update owner
        let env = mock_env("owner0000", &[]);
        let msg = HandleMsg::UpdateConfig {
            owner: Some(HumanAddr("owner0001".to_string())),
        };

        let res = handle(&mut deps, env, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let value = query_config(&deps).unwrap();
        assert_eq!("owner0001", value.owner.as_str());
        assert_eq!("base0000", &value.base_asset_info.to_string());

        // Unauthorzied err
        let env = mock_env("owner0000", &[]);
        let msg = HandleMsg::UpdateConfig { owner: None };

        let res = handle(&mut deps, env, msg);
        match res {
            Err(StdError::Unauthorized { .. }) => {}
            _ => panic!("Must return unauthorized error"),
        }
    }

    #[test]
    fn feed_price() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            owner: HumanAddr("owner0000".to_string()),
            base_asset_info: AssetInfo::NativeToken {
                denom: "base0000".to_string(),
            },
        };

        let env = mock_env("addr0000", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        // update price
        let env = mock_env("addr0000", &[]);
        let msg = HandleMsg::FeedPrice {
            price_infos: vec![PriceInfo {
                asset_token: HumanAddr::from("mAPPL"),
                price: Decimal::from_str("1.2").unwrap(),
            }],
        };

        let res = handle(&mut deps, env, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => assert_eq!(msg, "no asset data stored"),
            _ => panic!("DO NOT ENTER HERE"),
        }

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("mAPPL"),
            feeder: HumanAddr::from("addr0000"),
        };

        let env = mock_env("addr0000", &[]);
        let res = handle(&mut deps, env, msg).unwrap_err();
        match res {
            StdError::Unauthorized { .. } => {}
            _ => panic!("DO NOT ENTER HERE"),
        }

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("mAPPL"),
            feeder: HumanAddr::from("addr0000"),
        };

        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();
        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("mGOGL"),
            feeder: HumanAddr::from("addr0000"),
        };

        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        // try register the asset is already exists
        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("mAPPL"),
            feeder: HumanAddr::from("addr0000"),
        };

        let env = mock_env("owner0000", &[]);
        let res = handle(&mut deps, env, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => assert_eq!(msg, "Asset was already registered"),
            _ => panic!("DO NOT ENTER HERE"),
        }

        let value: AssetResponse = query_asset(&deps, HumanAddr::from("mAPPL")).unwrap();
        assert_eq!(
            value,
            AssetResponse {
                asset_token: HumanAddr::from("mAPPL"),
                feeder: HumanAddr::from("addr0000"),
            }
        );

        let value: PriceResponse = query_price(&deps, HumanAddr::from("mAPPL")).unwrap();
        assert_eq!(
            value,
            PriceResponse {
                asset_token: HumanAddr::from("mAPPL"),
                price: Decimal::zero(),
                last_update_time: 0u64,
            }
        );

        let msg = HandleMsg::FeedPrice {
            price_infos: vec![
                PriceInfo {
                    asset_token: HumanAddr::from("mAPPL"),
                    price: Decimal::from_str("1.2").unwrap(),
                },
                PriceInfo {
                    asset_token: HumanAddr::from("mGOGL"),
                    price: Decimal::from_str("2.2").unwrap(),
                },
            ],
        };
        let env = mock_env("addr0000", &[]);
        let _res = handle(&mut deps, env.clone(), msg).unwrap();
        let value: PriceResponse = query_price(&deps, HumanAddr::from("mAPPL")).unwrap();
        assert_eq!(
            value,
            PriceResponse {
                asset_token: HumanAddr::from("mAPPL"),
                price: Decimal::from_str("1.2").unwrap(),
                last_update_time: env.block.time,
            }
        );

        let value: PricesResponse = query_prices(&deps).unwrap();
        assert_eq!(
            value,
            PricesResponse {
                prices: vec![
                    PriceResponse {
                        asset_token: HumanAddr::from("mAPPL"),
                        price: Decimal::from_str("1.2").unwrap(),
                        last_update_time: env.block.time,
                    },
                    PriceResponse {
                        asset_token: HumanAddr::from("mGOGL"),
                        price: Decimal::from_str("2.2").unwrap(),
                        last_update_time: env.block.time,
                    }
                ],
            }
        );

        // Unautorized try
        let env = mock_env("addr0001", &[]);
        let msg = HandleMsg::FeedPrice {
            price_infos: vec![PriceInfo {
                asset_token: HumanAddr::from("mAPPL"),
                price: Decimal::from_str("1.2").unwrap(),
            }],
        };

        let res = handle(&mut deps, env, msg);
        match res {
            Err(StdError::Unauthorized { .. }) => {}
            _ => panic!("Must return unauthorized error"),
        }
    }

    #[test]
    fn migrate_asset() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            owner: HumanAddr("owner0000".to_string()),
            base_asset_info: AssetInfo::NativeToken {
                denom: "base0000".to_string(),
            },
        };

        let env = mock_env("addr0000", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("mAPPL"),
            feeder: HumanAddr::from("addr0000"),
        };

        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("mGOGL"),
            feeder: HumanAddr::from("addr0000"),
        };

        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::FeedPrice {
            price_infos: vec![
                PriceInfo {
                    asset_token: HumanAddr::from("mAPPL"),
                    price: Decimal::from_str("1.2").unwrap(),
                },
                PriceInfo {
                    asset_token: HumanAddr::from("mGOGL"),
                    price: Decimal::from_str("2.2").unwrap(),
                },
            ],
        };

        let env = mock_env("addr0000", &[]);
        let _res = handle(&mut deps, env.clone(), msg).unwrap();
        let value: PriceResponse = query_price(&deps, HumanAddr::from("mAPPL")).unwrap();
        assert_eq!(
            value,
            PriceResponse {
                asset_token: HumanAddr::from("mAPPL"),
                price: Decimal::from_str("1.2").unwrap(),
                last_update_time: env.block.time,
            }
        );

        let msg = HandleMsg::MigrateAsset {
            from_token: HumanAddr::from("mAPPL"),
            to_token: HumanAddr::from("mGOGL"),
        };

        let env = mock_env("addr0000", &[]);
        let res = handle(&mut deps, env, msg.clone());
        match res {
            Err(StdError::Unauthorized { .. }) => {}
            _ => panic!("DO NOT ENTER HERE"),
        }

        let env = mock_env("owner0000", &[]);
        let res = handle(&mut deps, env, msg);
        match res {
            Err(StdError::GenericErr { msg, .. }) => {
                assert_eq!(msg, "Asset was already registered")
            }
            _ => panic!("DO NOT ENTER HERE"),
        }

        let msg = HandleMsg::MigrateAsset {
            from_token: HumanAddr::from("mAPPL"),
            to_token: HumanAddr::from("mAPPL2"),
        };
        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();
        query_asset(&deps, HumanAddr::from("mAPPL")).unwrap_err();
        query_price(&deps, HumanAddr::from("mAPPL")).unwrap_err();

        let res = query_asset(&deps, HumanAddr::from("mAPPL2")).unwrap();
        assert_eq!(
            res,
            AssetResponse {
                asset_token: HumanAddr::from("mAPPL2"),
                feeder: HumanAddr::from("addr0000"),
            }
        );

        let res = query_price(&deps, HumanAddr::from("mAPPL2")).unwrap();
        assert_eq!(
            res,
            PriceResponse {
                price: Decimal::zero(),
                last_update_time: 0u64,
                asset_token: HumanAddr::from("mAPPL2"),
            }
        );
    }
}
