#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    attr, to_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult,
};

use crate::math::decimal_division;
use crate::state::{
    read_config, read_feeder, read_price, read_prices, store_config, store_feeder, store_price,
    Config, PriceInfo,
};

use mirror_protocol::common::OrderBy;
use mirror_protocol::oracle::{
    ConfigResponse, ExecuteMsg, FeederResponse, InstantiateMsg, MigrateMsg, PriceResponse,
    PricesResponse, PricesResponseElem, QueryMsg,
};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    store_config(
        deps.storage,
        &Config {
            owner: deps.api.addr_canonicalize(&msg.owner)?,
            base_asset: msg.base_asset,
        },
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::UpdateConfig { owner } => try_update_config(deps, info, owner),
        ExecuteMsg::RegisterAsset {
            asset_token,
            feeder,
        } => try_register_asset(deps, info, asset_token, feeder),
        ExecuteMsg::FeedPrice { prices } => try_feed_price(deps, env, info, prices),
    }
}

pub fn try_update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
) -> StdResult<Response> {
    let mut config: Config = read_config(deps.storage)?;
    if deps.api.addr_canonicalize(info.sender.as_str())? != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(owner) = owner {
        config.owner = deps.api.addr_canonicalize(&owner)?;
    }

    store_config(deps.storage, &config)?;
    Ok(Response::default())
}

pub fn try_register_asset(
    deps: DepsMut,
    info: MessageInfo,
    asset_token: String,
    feeder: String,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    if config.owner != deps.api.addr_canonicalize(info.sender.as_str())? {
        return Err(StdError::generic_err("unauthorized"));
    }

    let asset_token_raw = deps.api.addr_canonicalize(&asset_token)?;

    // check if it is a new asset
    if read_feeder(deps.storage, &asset_token_raw).is_err() {
        store_price(
            deps.storage,
            &asset_token_raw,
            &PriceInfo {
                price: Decimal::zero(),
                last_updated_time: 0u64,
            },
        )?;
    }

    // update/store feeder
    store_feeder(
        deps.storage,
        &asset_token_raw,
        &deps.api.addr_canonicalize(&feeder)?,
    )?;

    Ok(Response::default())
}

pub fn try_feed_price(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    prices: Vec<(String, Decimal)>,
) -> StdResult<Response> {
    let feeder_raw = deps.api.addr_canonicalize(info.sender.as_str())?;

    let mut attributes = vec![attr("action", "price_feed")];
    for price in prices {
        attributes.push(attr("asset", price.0.to_string()));
        attributes.push(attr("price", price.1.to_string()));

        // Check feeder permission
        let asset_token_raw = deps.api.addr_canonicalize(&price.0)?;
        if feeder_raw != read_feeder(deps.storage, &asset_token_raw)? {
            return Err(StdError::generic_err("unauthorized"));
        }

        let mut state: PriceInfo = read_price(deps.storage, &asset_token_raw)?;
        state.last_updated_time = env.block.time.seconds();
        state.price = price.1;

        store_price(deps.storage, &asset_token_raw, &state)?;
    }

    Ok(Response::new().add_attributes(attributes))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
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
        } => to_binary(&query_prices(deps, start_after, limit, order_by)?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let state = read_config(deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.addr_humanize(&state.owner)?.to_string(),
        base_asset: state.base_asset,
    };

    Ok(resp)
}

fn query_feeder(deps: Deps, asset_token: String) -> StdResult<FeederResponse> {
    let feeder = read_feeder(deps.storage, &deps.api.addr_canonicalize(&asset_token)?)?;
    let resp = FeederResponse {
        asset_token,
        feeder: deps.api.addr_humanize(&feeder)?.to_string(),
    };

    Ok(resp)
}

fn query_price(deps: Deps, base: String, quote: String) -> StdResult<PriceResponse> {
    let config: Config = read_config(deps.storage)?;
    let quote_price = if config.base_asset == quote {
        PriceInfo {
            price: Decimal::one(),
            last_updated_time: u64::MAX,
        }
    } else {
        read_price(deps.storage, &deps.api.addr_canonicalize(quote.as_str())?)?
    };

    let base_price = if config.base_asset == base {
        PriceInfo {
            price: Decimal::one(),
            last_updated_time: u64::MAX,
        }
    } else {
        read_price(deps.storage, &deps.api.addr_canonicalize(base.as_str())?)?
    };

    Ok(PriceResponse {
        rate: decimal_division(base_price.price, quote_price.price),
        last_updated_base: base_price.last_updated_time,
        last_updated_quote: quote_price.last_updated_time,
    })
}

fn query_prices(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<PricesResponse> {
    let start_after = if let Some(start_after) = start_after {
        Some(deps.api.addr_canonicalize(&start_after)?)
    } else {
        None
    };

    let prices: Vec<PricesResponseElem> = read_prices(deps, start_after, limit, order_by)?;

    Ok(PricesResponse { prices })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
