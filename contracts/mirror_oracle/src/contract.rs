use cosmwasm_std::{
    log, to_binary, Api, Binary, Decimal, Env, Extern, HandleResponse, HandleResult, HumanAddr,
    InitResponse, MigrateResponse, MigrateResult, Querier, StdError, StdResult, Storage,
};

use crate::math::decimal_division;
use crate::msg::{
    ConfigResponse, FeederResponse, HandleMsg, InitMsg, MigrateMsg, OrderBy, PriceResponse,
    PricesResponse, PricesResponseElem, QueryMsg,
};
use crate::state::{
    read_config, read_feeder, read_price, read_prices, store_config, store_feeder, store_price,
    Config, PriceInfo,
};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    store_config(
        &mut deps.storage,
        &Config {
            owner: deps.api.canonical_address(&msg.owner)?,
            base_asset: msg.base_asset,
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
        HandleMsg::FeedPrice { prices } => try_feed_price(deps, env, prices),
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
    if read_feeder(&deps.storage, &asset_token_raw).is_ok() {
        return Err(StdError::generic_err("Asset was already registered"));
    }

    store_price(
        &mut deps.storage,
        &asset_token_raw,
        &PriceInfo {
            price: Decimal::zero(),
            last_updated_time: 0u64,
        },
    )?;

    store_feeder(
        &mut deps.storage,
        &asset_token_raw,
        &deps.api.canonical_address(&feeder)?,
    )?;

    Ok(HandleResponse::default())
}

pub fn try_feed_price<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    prices: Vec<(HumanAddr, Decimal)>,
) -> HandleResult {
    let feeder_raw = deps.api.canonical_address(&env.message.sender)?;

    let mut logs = vec![log("action", "price_feed")];
    for price in prices {
        logs.push(log("asset", price.0.to_string()));
        logs.push(log("price", price.1.to_string()));

        // Check feeder permission
        let asset_token_raw = deps.api.canonical_address(&price.0)?;
        if feeder_raw != read_feeder(&deps.storage, &asset_token_raw)? {
            return Err(StdError::unauthorized());
        }

        let mut state: PriceInfo = read_price(&deps.storage, &asset_token_raw)?;
        state.last_updated_time = env.block.time;
        state.price = price.1;

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
        QueryMsg::Feeder { asset_token } => to_binary(&query_feeder(deps, asset_token)?),
        QueryMsg::Price {
            base_asset,
            quote_asset,
        } => to_binary(&query_price(deps, base_asset, quote_asset)?),
        QueryMsg::Prices {
            start_after,
            limit,
            order_by,
        } => to_binary(&query_prices(deps, start_after, limit, order_by)),
    }
}

fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let state = read_config(&deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.human_address(&state.owner)?,
        base_asset: state.base_asset,
    };

    Ok(resp)
}

fn query_feeder<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    asset_token: HumanAddr,
) -> StdResult<FeederResponse> {
    let feeder = read_feeder(&deps.storage, &deps.api.canonical_address(&asset_token)?)?;
    let resp = FeederResponse {
        asset_token,
        feeder: deps.api.human_address(&feeder)?,
    };

    Ok(resp)
}

fn query_price<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    base: String,
    quote: String,
) -> StdResult<PriceResponse> {
    let config: Config = read_config(&deps.storage)?;
    let quote_price = if config.base_asset == quote {
        PriceInfo {
            price: Decimal::one(),
            last_updated_time: u64::MAX,
        }
    } else {
        read_price(
            &deps.storage,
            &deps.api.canonical_address(&HumanAddr::from(quote))?,
        )?
    };

    let base_price = if config.base_asset == base {
        PriceInfo {
            price: Decimal::one(),
            last_updated_time: u64::MAX,
        }
    } else {
        read_price(
            &deps.storage,
            &deps.api.canonical_address(&HumanAddr::from(base))?,
        )?
    };

    Ok(PriceResponse {
        rate: decimal_division(base_price.price, quote_price.price),
        last_updated_base: base_price.last_updated_time,
        last_updated_quote: quote_price.last_updated_time,
    })
}

fn query_prices<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    start_after: Option<HumanAddr>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<PricesResponse> {
    let start_after = if let Some(start_after) = start_after {
        Some(deps.api.canonical_address(&start_after)?)
    } else {
        None
    };

    let prices: Vec<PricesResponseElem> = read_prices(&deps, start_after, limit, order_by)?;

    Ok(PricesResponse { prices })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cosmwasm_std::StdError;

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            owner: HumanAddr("owner0000".to_string()),
            base_asset: "base0000".to_string(),
        };

        let env = mock_env("addr0000", &[]);

        // we can just call .unwrap() to assert this was a success
        let res = init(&mut deps, env, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let value = query_config(&deps).unwrap();
        assert_eq!("owner0000", value.owner.as_str());
        assert_eq!("base0000", &value.base_asset.to_string());
    }

    #[test]
    fn update_config() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            owner: HumanAddr("owner0000".to_string()),
            base_asset: "base0000".to_string(),
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
        assert_eq!("base0000", &value.base_asset.to_string());

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
            base_asset: "base0000".to_string(),
        };

        let env = mock_env("addr0000", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        // update price
        let env = mock_env("addr0000", &[]);
        let msg = HandleMsg::FeedPrice {
            prices: vec![(
                HumanAddr::from("mAAPL"),
                Decimal::from_ratio(12u128, 10u128),
            )],
        };

        let _res = handle(&mut deps, env, msg).unwrap_err();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("mAAPL"),
            feeder: HumanAddr::from("addr0000"),
        };

        let env = mock_env("addr0000", &[]);
        let res = handle(&mut deps, env, msg).unwrap_err();
        match res {
            StdError::Unauthorized { .. } => {}
            _ => panic!("DO NOT ENTER HERE"),
        }

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("mAAPL"),
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
            asset_token: HumanAddr::from("mAAPL"),
            feeder: HumanAddr::from("addr0000"),
        };

        let env = mock_env("owner0000", &[]);
        let res = handle(&mut deps, env.clone(), msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => assert_eq!(msg, "Asset was already registered"),
            _ => panic!("DO NOT ENTER HERE"),
        }

        let value: FeederResponse = query_feeder(&deps, HumanAddr::from("mAAPL")).unwrap();
        assert_eq!(
            value,
            FeederResponse {
                asset_token: HumanAddr::from("mAAPL"),
                feeder: HumanAddr::from("addr0000"),
            }
        );

        let value: PriceResponse =
            query_price(&deps, "mAAPL".to_string(), "base0000".to_string()).unwrap();
        assert_eq!(
            value,
            PriceResponse {
                rate: Decimal::zero(),
                last_updated_base: 0u64,
                last_updated_quote: u64::MAX,
            }
        );

        let msg = HandleMsg::FeedPrice {
            prices: vec![
                (
                    HumanAddr::from("mAAPL"),
                    Decimal::from_ratio(12u128, 10u128),
                ),
                (
                    HumanAddr::from("mGOGL"),
                    Decimal::from_ratio(22u128, 10u128),
                ),
            ],
        };
        let env = mock_env("addr0000", &[]);
        let _res = handle(&mut deps, env.clone(), msg).unwrap();
        let value: PriceResponse =
            query_price(&deps, "mAAPL".to_string(), "base0000".to_string()).unwrap();
        assert_eq!(
            value,
            PriceResponse {
                rate: Decimal::from_ratio(12u128, 10u128),
                last_updated_base: env.block.time,
                last_updated_quote: u64::MAX,
            }
        );

        let value: PricesResponse = query_prices(&deps, None, None, Some(OrderBy::Aes)).unwrap();
        assert_eq!(
            value,
            PricesResponse {
                prices: vec![
                    PricesResponseElem {
                        asset_token: HumanAddr::from("mAAPL"),
                        price: Decimal::from_ratio(12u128, 10u128),
                        last_updated_time: env.block.time,
                    },
                    PricesResponseElem {
                        asset_token: HumanAddr::from("mGOGL"),
                        price: Decimal::from_ratio(22u128, 10u128),
                        last_updated_time: env.block.time,
                    }
                ],
            }
        );

        // Unautorized try
        let env = mock_env("addr0001", &[]);
        let msg = HandleMsg::FeedPrice {
            prices: vec![(
                HumanAddr::from("mAAPL"),
                Decimal::from_ratio(12u128, 10u128),
            )],
        };

        let res = handle(&mut deps, env, msg);
        match res {
            Err(StdError::Unauthorized { .. }) => {}
            _ => panic!("Must return unauthorized error"),
        }
    }
}

pub fn migrate<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
    _msg: MigrateMsg,
) -> MigrateResult {
    Ok(MigrateResponse::default())
}
