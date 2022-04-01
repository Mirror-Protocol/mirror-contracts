use crate::migration::{migrate_collateral_infos, migrate_config};
use crate::querier::query_price;
use crate::state::{
    read_collateral_info, read_collateral_infos, read_config, store_collateral_info, store_config,
    CollateralAssetInfo, Config,
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, CanonicalAddr, Decimal, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult,
};
use mirror_protocol::collateral_oracle::{
    CollateralInfoResponse, CollateralInfosResponse, CollateralPriceResponse, ConfigResponse,
    ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, SourceType,
};
use terraswap::asset::AssetInfo;

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
            mint_contract: deps.api.addr_canonicalize(&msg.mint_contract)?,
            base_denom: msg.base_denom,
        },
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::UpdateConfig {
            owner,
            mint_contract,
            base_denom,
        } => update_config(deps, info, owner, mint_contract, base_denom),
        ExecuteMsg::RegisterCollateralAsset {
            asset,
            price_source,
            multiplier,
        } => register_collateral(deps, info, asset, price_source, multiplier),
        ExecuteMsg::RevokeCollateralAsset { asset } => revoke_collateral(deps, info, asset),
        ExecuteMsg::UpdateCollateralPriceSource {
            asset,
            price_source,
        } => update_collateral_source(deps, info, asset, price_source),
        ExecuteMsg::UpdateCollateralMultiplier { asset, multiplier } => {
            update_collateral_multiplier(deps, info, asset, multiplier)
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
    mint_contract: Option<String>,
    base_denom: Option<String>,
) -> StdResult<Response> {
    let mut config: Config = read_config(deps.storage)?;
    if deps.api.addr_canonicalize(info.sender.as_str())? != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(owner) = owner {
        config.owner = deps.api.addr_canonicalize(&owner)?;
    }

    if let Some(mint_contract) = mint_contract {
        config.mint_contract = deps.api.addr_canonicalize(&mint_contract)?;
    }

    if let Some(base_denom) = base_denom {
        config.base_denom = base_denom;
    }

    store_config(deps.storage, &config)?;
    Ok(Response::default())
}

pub fn register_collateral(
    deps: DepsMut,
    info: MessageInfo,
    asset: AssetInfo,
    price_source: SourceType,
    multiplier: Decimal,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let sender_address_raw: CanonicalAddr = deps.api.addr_canonicalize(info.sender.as_str())?;
    // only contract onwner and mint contract can register a new collateral
    if config.owner != sender_address_raw && config.mint_contract != sender_address_raw {
        return Err(StdError::generic_err("unauthorized"));
    }

    if read_collateral_info(deps.storage, &asset.to_string()).is_ok() {
        return Err(StdError::generic_err("Collateral was already registered"));
    }

    if multiplier.is_zero() {
        return Err(StdError::generic_err("Multiplier must be bigger than 0"));
    }

    store_collateral_info(
        deps.storage,
        &CollateralAssetInfo {
            asset: asset.to_string(),
            multiplier,
            price_source,
            is_revoked: false,
        },
    )?;

    Ok(Response::default())
}

pub fn revoke_collateral(
    deps: DepsMut,
    info: MessageInfo,
    asset: AssetInfo,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let sender_address_raw: CanonicalAddr = deps.api.addr_canonicalize(info.sender.as_str())?;
    // only owner and mint contract can revoke a collateral assets
    if config.owner != sender_address_raw && config.mint_contract != sender_address_raw {
        return Err(StdError::generic_err("unauthorized"));
    }

    let mut collateral_info: CollateralAssetInfo =
        if let Ok(collateral) = read_collateral_info(deps.storage, &asset.to_string()) {
            collateral
        } else {
            return Err(StdError::generic_err("Collateral not found"));
        };

    collateral_info.is_revoked = true;

    store_collateral_info(deps.storage, &collateral_info)?;

    Ok(Response::default())
}

pub fn update_collateral_source(
    deps: DepsMut,
    info: MessageInfo,
    asset: AssetInfo,
    price_source: SourceType,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let sender_address_raw: CanonicalAddr = deps.api.addr_canonicalize(info.sender.as_str())?;
    // only contract onwner can update collateral query
    if config.owner != sender_address_raw {
        return Err(StdError::generic_err("unauthorized"));
    }

    let mut collateral_info: CollateralAssetInfo =
        if let Ok(collateral) = read_collateral_info(deps.storage, &asset.to_string()) {
            collateral
        } else {
            return Err(StdError::generic_err("Collateral not found"));
        };

    collateral_info.price_source = price_source;

    store_collateral_info(deps.storage, &collateral_info)?;

    Ok(Response::default())
}

pub fn update_collateral_multiplier(
    deps: DepsMut,
    info: MessageInfo,
    asset: AssetInfo,
    multiplier: Decimal,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let sender_address_raw: CanonicalAddr = deps.api.addr_canonicalize(info.sender.as_str())?;
    // only factory contract can update collateral premium
    if config.owner != sender_address_raw {
        return Err(StdError::generic_err("unauthorized"));
    }

    let mut collateral_info: CollateralAssetInfo =
        if let Ok(collateral) = read_collateral_info(deps.storage, &asset.to_string()) {
            collateral
        } else {
            return Err(StdError::generic_err("Collateral not found"));
        };

    if multiplier.is_zero() {
        return Err(StdError::generic_err("Multiplier must be bigger than 0"));
    }

    collateral_info.multiplier = multiplier;
    store_collateral_info(deps.storage, &collateral_info)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::CollateralPrice { asset, timeframe } => {
            to_binary(&query_collateral_price(deps, env, asset, timeframe)?)
        }
        QueryMsg::CollateralAssetInfo { asset } => to_binary(&query_collateral_info(deps, asset)?),
        QueryMsg::CollateralAssetInfos {} => to_binary(&query_collateral_infos(deps)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = read_config(deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.addr_humanize(&config.owner)?.to_string(),
        mint_contract: deps.api.addr_humanize(&config.mint_contract)?.to_string(),
        base_denom: config.base_denom,
    };

    Ok(resp)
}

pub fn query_collateral_price(
    deps: Deps,
    env: Env,
    quote_asset: String,
    timeframe: Option<u64>,
) -> StdResult<CollateralPriceResponse> {
    let config: Config = read_config(deps.storage)?;

    let collateral: CollateralAssetInfo =
        if let Ok(res) = read_collateral_info(deps.storage, &quote_asset) {
            res
        } else {
            return Err(StdError::generic_err("Collateral asset not found"));
        };

    let (price, last_updated): (Decimal, u64) = query_price(
        deps,
        env,
        &config,
        &quote_asset,
        timeframe,
        &collateral.price_source,
    )?;

    Ok(CollateralPriceResponse {
        asset: collateral.asset,
        rate: price,
        last_updated,
        multiplier: collateral.multiplier,
        is_revoked: collateral.is_revoked,
    })
}

pub fn query_collateral_info(deps: Deps, quote_asset: String) -> StdResult<CollateralInfoResponse> {
    let collateral: CollateralAssetInfo =
        if let Ok(res) = read_collateral_info(deps.storage, &quote_asset) {
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

pub fn query_collateral_infos(deps: Deps) -> StdResult<CollateralInfosResponse> {
    let infos: Vec<CollateralInfoResponse> = read_collateral_infos(deps.storage)?;

    Ok(CollateralInfosResponse { collaterals: infos })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> StdResult<Response> {
    migrate_config(deps.storage)?;

    deps.api.addr_validate(&msg.mirror_tefi_oracle_addr)?;
    migrate_collateral_infos(deps.storage, msg.mirror_tefi_oracle_addr)?;

    Ok(Response::default())
}
