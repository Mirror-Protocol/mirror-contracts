use crate::querier::query_price;
use crate::state::{
    read_collateral_info, read_collateral_infos, read_config, store_collateral_info, store_config,
    CollateralAssetInfo, Config,
};
use cosmwasm_std::{
    to_binary, Api, Binary, CanonicalAddr, Decimal, Env, Extern, HandleResponse, HandleResult,
    HumanAddr, InitResponse, MigrateResponse, MigrateResult, Querier, StdError, StdResult, Storage,
};

use mirror_protocol::collateral_oracle::{
    CollateralInfoResponse, CollateralInfosResponse, CollateralPriceResponse, ConfigResponse,
    HandleMsg, InitMsg, MigrateMsg, QueryMsg, SourceType,
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
            mint_contract: deps.api.canonical_address(&msg.mint_contract)?,
            factory_contract: deps.api.canonical_address(&msg.factory_contract)?,
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
        HandleMsg::UpdateConfig {
            owner,
            mint_contract,
            factory_contract,
            base_denom,
        } => update_config(
            deps,
            env,
            owner,
            mint_contract,
            factory_contract,
            base_denom,
        ),
        HandleMsg::RegisterCollateralAsset {
            asset,
            price_source,
            multiplier,
        } => register_collateral(deps, env, asset, price_source, multiplier),
        HandleMsg::RevokeCollateralAsset { asset } => revoke_collateral(deps, env, asset),
        HandleMsg::UpdateCollateralPriceSource {
            asset,
            price_source,
        } => update_collateral_source(deps, env, asset, price_source),
        HandleMsg::UpdateCollateralMultiplier { asset, multiplier } => {
            update_collateral_multiplier(deps, env, asset, multiplier)
        }
    }
}

pub fn update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: Option<HumanAddr>,
    mint_contract: Option<HumanAddr>,
    factory_contract: Option<HumanAddr>,
    base_denom: Option<String>,
) -> HandleResult {
    let mut config: Config = read_config(&deps.storage)?;
    if deps.api.canonical_address(&env.message.sender)? != config.owner {
        return Err(StdError::unauthorized());
    }

    if let Some(owner) = owner {
        config.owner = deps.api.canonical_address(&owner)?;
    }

    if let Some(mint_contract) = mint_contract {
        config.mint_contract = deps.api.canonical_address(&mint_contract)?;
    }

    if let Some(factory_contract) = factory_contract {
        config.factory_contract = deps.api.canonical_address(&factory_contract)?;
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
    price_source: SourceType,
    multiplier: Decimal,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    let sender_address_raw: CanonicalAddr = deps.api.canonical_address(&env.message.sender)?;
    // only contract onwner and mint contract can register a new collateral
    if config.owner != sender_address_raw && config.mint_contract != sender_address_raw {
        return Err(StdError::unauthorized());
    }

    if read_collateral_info(&deps.storage, &asset.to_string()).is_ok() {
        return Err(StdError::generic_err("Collateral was already registered"));
    }

    if multiplier.is_zero() {
        return Err(StdError::generic_err("Multiplier must be bigger than 0"));
    }

    store_collateral_info(
        &mut deps.storage,
        &CollateralAssetInfo {
            asset: asset.to_string(),
            multiplier,
            price_source,
            is_revoked: false,
        },
    )?;

    Ok(HandleResponse::default())
}

pub fn revoke_collateral<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset: AssetInfo,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    let sender_address_raw: CanonicalAddr = deps.api.canonical_address(&env.message.sender)?;
    // only owner and mint contract can revoke a collateral assets
    if config.owner != sender_address_raw && config.mint_contract != sender_address_raw {
        return Err(StdError::unauthorized());
    }

    let mut collateral_info: CollateralAssetInfo =
        if let Ok(collateral) = read_collateral_info(&deps.storage, &asset.to_string()) {
            collateral
        } else {
            return Err(StdError::generic_err("Collateral not found"));
        };

    collateral_info.is_revoked = true;

    store_collateral_info(&mut deps.storage, &collateral_info)?;

    Ok(HandleResponse::default())
}

pub fn update_collateral_source<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset: AssetInfo,
    price_source: SourceType,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    let sender_address_raw: CanonicalAddr = deps.api.canonical_address(&env.message.sender)?;
    // only contract onwner can update collateral query
    if config.owner != sender_address_raw {
        return Err(StdError::unauthorized());
    }

    let mut collateral_info: CollateralAssetInfo =
        if let Ok(collateral) = read_collateral_info(&deps.storage, &asset.to_string()) {
            collateral
        } else {
            return Err(StdError::generic_err("Collateral not found"));
        };

    collateral_info.price_source = price_source;

    store_collateral_info(&mut deps.storage, &collateral_info)?;

    Ok(HandleResponse::default())
}

pub fn update_collateral_multiplier<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset: AssetInfo,
    multiplier: Decimal,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    let sender_address_raw: CanonicalAddr = deps.api.canonical_address(&env.message.sender)?;
    // only factory contract can update collateral premium
    if config.factory_contract != sender_address_raw {
        return Err(StdError::unauthorized());
    }

    let mut collateral_info: CollateralAssetInfo =
        if let Ok(collateral) = read_collateral_info(&deps.storage, &asset.to_string()) {
            collateral
        } else {
            return Err(StdError::generic_err("Collateral not found"));
        };

    if multiplier.is_zero() {
        return Err(StdError::generic_err("Multiplier must be bigger than 0"));
    }

    collateral_info.multiplier = multiplier;
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
        mint_contract: deps.api.human_address(&config.mint_contract)?,
        factory_contract: deps.api.human_address(&config.factory_contract)?,
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

    let (price, last_updated): (Decimal, u64) =
        query_price(deps, collateral.price_source, config.base_denom)?;

    Ok(CollateralPriceResponse {
        asset: collateral.asset,
        rate: price,
        last_updated,
        multiplier: collateral.multiplier,
        is_revoked: collateral.is_revoked,
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

    Ok(CollateralInfoResponse {
        asset: collateral.asset,
        source_type: collateral.price_source.to_string(),
        multiplier: collateral.multiplier,
        is_revoked: collateral.is_revoked,
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
