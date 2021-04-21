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
            base_denom: msg.base_denom,
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
        HandleMsg::UpdateConfig { owner, base_denom } => {
            update_config(deps, env, owner, base_denom)
        }
        HandleMsg::RegisterCollateralAsset {
            asset,
            query_request,
            collateral_premium,
        } => register_collateral(deps, env, asset, query_request, collateral_premium),
        HandleMsg::UpdateCollateralAsset {
            asset,
            query_request,
            collateral_premium,
        } => update_collateral(deps, env, asset, query_request, collateral_premium),
    }
}

pub fn update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: Option<HumanAddr>,
    base_denom: Option<String>,
) -> HandleResult {
    let mut config: Config = read_config(&deps.storage)?;
    if deps.api.canonical_address(&env.message.sender)? != config.owner {
        return Err(StdError::unauthorized());
    }

    if let Some(owner) = owner {
        config.owner = deps.api.canonical_address(&owner)?;
    }

    if let Some(base_denom) = base_denom {
        config.base_denom = base_denom;
    }

    store_config(&mut deps.storage, &config)?;
    Ok(HandleResponse::default())
}

pub fn register_collateral<S: Storage, A: Api, Q: Querier>(
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
    if query_price(&deps, query_request.clone(), config.base_denom).is_err() {
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

pub fn update_collateral<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset: AssetInfo,
    query_request: Option<Binary>,
    collateral_premium: Option<Decimal>,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    // only contract onwner can update collaterals
    if config.owner != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    let collateral_id: String = match asset {
        AssetInfo::NativeToken { denom } => denom,
        AssetInfo::Token { contract_addr } => contract_addr.to_string(),
    };

    let mut collateral_info =
        if let Ok(collateral) = read_collateral_info(&deps.storage, &collateral_id) {
            collateral
        } else {
            return Err(StdError::generic_err("Collateral not found"));
        };

    if let Some(query_request) = query_request {
        // test the query request
        if query_price(&deps, query_request.clone(), config.base_denom).is_err() {
            return Err(StdError::generic_err(
                "The query request provided is not valid",
            ));
        }
        collateral_info.query_request = query_request;
    }

    if let Some(collateral_premium) = collateral_premium {
        collateral_info.collateral_premium = collateral_premium;
    }

    store_collateral_info(&mut deps.storage, &collateral_info)?;

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
    let config = read_config(&deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.human_address(&config.owner)?,
        base_denom: config.base_denom,
    };

    Ok(resp)
}

pub fn query_collateral_price<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    quote_asset: String,
) -> StdResult<CollateralPriceResponse> {
    let config: Config = read_config(&deps.storage)?;

    let collateral: CollateralAssetInfo =
        if let Ok(res) = read_collateral_info(&deps.storage, &quote_asset) {
            res
        } else {
            return Err(StdError::generic_err("Collateral asset not found"));
        };

    let price: Decimal = query_price(deps, collateral.query_request, config.base_denom)?;

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
