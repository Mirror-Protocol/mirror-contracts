use cosmwasm_std::{
    log, to_binary, Api, Binary, Decimal, Env, Extern, HandleResponse, HandleResult, HumanAddr,
    InitResponse, Querier, StdError, StdResult, Storage,
};

use crate::msg::{
    AssetResponse, ConfigResponse, HandleMsg, InitMsg, PriceResponse, PricesResponse, QueryMsg,
};

use crate::state::{
    read_asset_config, read_config, read_price, read_prices, store_asset_config, store_config,
    store_price, AssetConfig, Config, PriceInfo,
};

use uniswap::{AssetInfo, AssetInfoRaw};

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
        HandleMsg::RegisterAsset { asset_info, feeder } => {
            try_register_asset(deps, env, asset_info, feeder)
        }
        HandleMsg::FeedPrice {
            asset_info,
            price,
            price_multiplier,
        } => try_feed_price(deps, env, asset_info, price, price_multiplier),
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
    asset_info: AssetInfo,
    feeder: HumanAddr,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    if config.owner != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    let raw_info = asset_info.to_raw(&deps)?;
    if read_asset_config(&deps.storage, &raw_info).is_ok() {
        return Err(StdError::generic_err("Asset was already registered"));
    }

    store_price(
        &mut deps.storage,
        &raw_info,
        &PriceInfo {
            asset_info: raw_info.clone(),
            price: Decimal::zero(),
            price_multiplier: Decimal::one(),
            last_update_time: 0u64,
        },
    )?;

    store_asset_config(
        &mut deps.storage,
        &raw_info,
        &AssetConfig {
            asset_info: raw_info.clone(),
            feeder: deps.api.canonical_address(&feeder)?,
        },
    )?;

    Ok(HandleResponse::default())
}

pub fn try_feed_price<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_info: AssetInfo,
    price: Decimal,
    price_multiplier: Option<Decimal>,
) -> HandleResult {
    let raw_info = asset_info.to_raw(&deps)?;
    let asset: AssetConfig = read_asset_config(&deps.storage, &raw_info)?;
    if deps.api.canonical_address(&env.message.sender)? != asset.feeder {
        return Err(StdError::unauthorized());
    }

    let mut state: PriceInfo = read_price(&deps.storage, &raw_info)?;
    state.last_update_time = env.block.time;
    state.price = price;
    if let Some(price_multiplier) = price_multiplier {
        state.price_multiplier = price_multiplier;
    }

    store_price(&mut deps.storage, &raw_info, &state)?;
    let res = HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "price_feed"),
            log("price", &price.to_string()),
        ],
        data: None,
    };

    Ok(res)
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::Asset { asset_info } => to_binary(&query_asset(deps, asset_info)?),
        QueryMsg::Price { asset_info } => to_binary(&query_price(deps, asset_info)?),
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
    asset_info: AssetInfo,
) -> StdResult<AssetResponse> {
    let raw_info: AssetInfoRaw = asset_info.to_raw(&deps)?;
    let state = read_asset_config(&deps.storage, &raw_info)?;
    let resp = AssetResponse {
        asset_info: raw_info.to_normal(&deps)?,
        feeder: deps.api.human_address(&state.feeder)?,
    };

    Ok(resp)
}

fn query_price<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    asset_info: AssetInfo,
) -> StdResult<PriceResponse> {
    let raw_info: AssetInfoRaw = asset_info.to_raw(&deps)?;
    let state = read_price(&deps.storage, &raw_info)?;
    let resp = PriceResponse {
        asset_info: raw_info.to_normal(&deps)?,
        price: state.price,
        price_multiplier: state.price_multiplier,
        last_update_time: state.last_update_time,
    };

    Ok(resp)
}

fn query_prices<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<PricesResponse> {
    let states: Vec<PriceInfo> = read_prices(&deps.storage)?;

    let mut prices: Vec<PriceResponse> = vec![];
    for state in states {
        prices.push(PriceResponse {
            asset_info: state.asset_info.to_normal(&deps)?,
            price: state.price,
            price_multiplier: state.price_multiplier,
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
            asset_info: AssetInfo::Token {
                contract_addr: HumanAddr::from("mAPPL"),
            },
            price: Decimal::from_str("1.2").unwrap(),
            price_multiplier: None,
        };

        let res = handle(&mut deps, env, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => assert_eq!(msg, "no asset data stored"),
            _ => panic!("DO NOT ENTER HERE"),
        }

        let msg = HandleMsg::RegisterAsset {
            asset_info: AssetInfo::Token {
                contract_addr: HumanAddr::from("mAPPL"),
            },
            feeder: HumanAddr::from("addr0000"),
        };

        let env = mock_env("addr0000", &[]);
        let res = handle(&mut deps, env, msg).unwrap_err();
        match res {
            StdError::Unauthorized { .. } => {}
            _ => panic!("DO NOT ENTER HERE"),
        }

        let msg = HandleMsg::RegisterAsset {
            asset_info: AssetInfo::Token {
                contract_addr: HumanAddr::from("mAPPL"),
            },
            feeder: HumanAddr::from("addr0000"),
        };

        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();
        let msg = HandleMsg::RegisterAsset {
            asset_info: AssetInfo::Token {
                contract_addr: HumanAddr::from("mGOGL"),
            },
            feeder: HumanAddr::from("addr0001"),
        };

        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        // try register the asset is already exists
        let msg = HandleMsg::RegisterAsset {
            asset_info: AssetInfo::Token {
                contract_addr: HumanAddr::from("mAPPL"),
            },
            feeder: HumanAddr::from("addr0000"),
        };

        let env = mock_env("owner0000", &[]);
        let res = handle(&mut deps, env, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => assert_eq!(msg, "Asset was already registered"),
            _ => panic!("DO NOT ENTER HERE"),
        }

        let value: AssetResponse = query_asset(
            &deps,
            AssetInfo::Token {
                contract_addr: HumanAddr::from("mAPPL"),
            },
        )
        .unwrap();
        assert_eq!(
            value,
            AssetResponse {
                asset_info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("mAPPL")
                },
                feeder: HumanAddr::from("addr0000"),
            }
        );

        let value: PriceResponse = query_price(
            &deps,
            AssetInfo::Token {
                contract_addr: HumanAddr::from("mAPPL"),
            },
        )
        .unwrap();
        assert_eq!(
            value,
            PriceResponse {
                asset_info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("mAPPL")
                },
                price: Decimal::zero(),
                price_multiplier: Decimal::one(),
                last_update_time: 0u64,
            }
        );

        let msg = HandleMsg::FeedPrice {
            asset_info: AssetInfo::Token {
                contract_addr: HumanAddr::from("mAPPL"),
            },
            price: Decimal::from_str("1.2").unwrap(),
            price_multiplier: None,
        };
        let env = mock_env("addr0000", &[]);
        let _res = handle(&mut deps, env.clone(), msg).unwrap();
        let value: PriceResponse = query_price(
            &deps,
            AssetInfo::Token {
                contract_addr: HumanAddr::from("mAPPL"),
            },
        )
        .unwrap();
        assert_eq!(
            value,
            PriceResponse {
                asset_info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("mAPPL")
                },
                price: Decimal::from_str("1.2").unwrap(),
                price_multiplier: Decimal::one(),
                last_update_time: env.block.time,
            }
        );

        let msg = HandleMsg::FeedPrice {
            asset_info: AssetInfo::Token {
                contract_addr: HumanAddr::from("mGOGL"),
            },
            price: Decimal::from_str("2.2").unwrap(),
            price_multiplier: None,
        };
        let env = mock_env("addr0001", &[]);
        let _res = handle(&mut deps, env.clone(), msg).unwrap();
        let value: PricesResponse = query_prices(&deps).unwrap();
        assert_eq!(
            value,
            PricesResponse {
                prices: vec![
                    PriceResponse {
                        asset_info: AssetInfo::Token {
                            contract_addr: HumanAddr::from("mAPPL")
                        },
                        price: Decimal::from_str("1.2").unwrap(),
                        price_multiplier: Decimal::one(),
                        last_update_time: env.block.time,
                    },
                    PriceResponse {
                        asset_info: AssetInfo::Token {
                            contract_addr: HumanAddr::from("mGOGL")
                        },
                        price: Decimal::from_str("2.2").unwrap(),
                        price_multiplier: Decimal::one(),
                        last_update_time: env.block.time,
                    }
                ],
            }
        );

        // Unautorized try
        let env = mock_env("addr0001", &[]);
        let msg = HandleMsg::FeedPrice {
            asset_info: AssetInfo::Token {
                contract_addr: HumanAddr::from("mAPPL"),
            },
            price: Decimal::from_str("1.2").unwrap(),
            price_multiplier: None,
        };

        let res = handle(&mut deps, env, msg);
        match res {
            Err(StdError::Unauthorized { .. }) => {}
            _ => panic!("Must return unauthorized error"),
        }
    }
}
