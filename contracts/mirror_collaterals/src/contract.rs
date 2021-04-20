use crate::querier::query_price;
use crate::state::{
    read_collateral_info, read_collateral_infos, read_config, store_collateral_info, store_config,
    CollateralAssetInfo, Config,
};
use cosmwasm_std::{
    from_binary, to_binary, Api, Binary, Decimal, Env, Extern, HandleResponse, HandleResult,
    HumanAddr, InitResponse, MigrateResponse, MigrateResult, Querier, StdError, StdResult, Storage,
    WasmQuery,
};

use mirror_protocol::collateral_oracle::{
    CollateralInfoResponse, CollateralInfosResponse, CollateralPriceResponse, ConfigResponse,
    HandleMsg, InitMsg, MigrateMsg, QueryMsg,
};

use terraswap::asset::AssetInfo;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    store_config(
        &mut deps.storage,
        &Config {
            owner: deps.api.canonical_address(&msg.owner)?,
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
        HandleMsg::RegisterCollateralAsset {
            asset,
            query_request,
            collateral_premium,
        } => try_register_collateral(deps, env, asset, query_request, collateral_premium),
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

pub fn try_register_collateral<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset: AssetInfo,
    query_request: Binary,
    collateral_premium: Decimal,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    // only contract onwner can register a new collateral
    if config.owner != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    let collateral_id: String = match asset {
        AssetInfo::NativeToken { denom } => denom,
        AssetInfo::Token { contract_addr } => contract_addr.to_string(),
    };

    if read_collateral_info(&deps.storage, &collateral_id).is_ok() {
        return Err(StdError::generic_err("Collateral was already registered"));
    }

    // test the query_request
    if query_price(&deps, query_request.clone()).is_err() {
        return Err(StdError::generic_err(
            "The query request provided is not valid",
        ));
    }

    store_collateral_info(
        &mut deps.storage,
        &CollateralAssetInfo {
            asset: collateral_id,
            collateral_premium,
            query_request,
        },
    )?;

    Ok(HandleResponse::default())
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::CollateralPrice { asset } => to_binary(&query_collateral_price(deps, asset)?),
        QueryMsg::CollateralAssetInfo { asset } => to_binary(&query_collateral_info(deps, asset)?),
        QueryMsg::CollateralAssetInfos {} => to_binary(&query_collateral_infos(deps)?),
    }
}

pub fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let state = read_config(&deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.human_address(&state.owner)?,
    };

    Ok(resp)
}

pub fn query_collateral_price<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    quote_asset: String,
) -> StdResult<CollateralPriceResponse> {
    let _config: Config = read_config(&deps.storage)?;

    let collateral: CollateralAssetInfo =
        if let Ok(res) = read_collateral_info(&deps.storage, &quote_asset) {
            res
        } else {
            return Err(StdError::generic_err("Collateral asset not found"));
        };

    let price: Decimal = query_price(deps, collateral.query_request)?;

    Ok(CollateralPriceResponse {
        asset: collateral.asset,
        rate: price,
        collateral_premium: collateral.collateral_premium,
    })
}

pub fn query_collateral_info<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    quote_asset: String,
) -> StdResult<CollateralInfoResponse> {
    let collateral: CollateralAssetInfo =
        if let Ok(res) = read_collateral_info(&deps.storage, &quote_asset) {
            res
        } else {
            return Err(StdError::generic_err("Collateral asset not found"));
        };

    let wasm_query: WasmQuery = from_binary(&collateral.query_request)?;

    Ok(CollateralInfoResponse {
        asset: collateral.asset,
        query_request: wasm_query,
        collateral_premium: collateral.collateral_premium,
    })
}

pub fn query_collateral_infos<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<CollateralInfosResponse> {
    let infos: Vec<CollateralInfoResponse> = read_collateral_infos(&deps.storage)?;

    Ok(CollateralInfosResponse { collaterals: infos })
}

pub fn migrate<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
    _msg: MigrateMsg,
) -> MigrateResult {
    Ok(MigrateResponse::default())
}
