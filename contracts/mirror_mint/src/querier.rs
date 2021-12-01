use cosmwasm_std::{
    to_binary, Addr, Decimal, Deps, QuerierWrapper, QueryRequest, StdResult, WasmQuery,
};

use crate::{
    math::decimal_division,
    state::{read_config, read_fixed_price, Config},
};
use mirror_protocol::collateral_oracle::{
    CollateralInfoResponse, CollateralPriceResponse, QueryMsg as CollateralOracleQueryMsg,
};
use tefi_oracle::hub::{HubQueryMsg as OracleQueryMsg, PriceResponse};
use terraswap::asset::AssetInfoRaw;

const PRICE_EXPIRE_TIME: u64 = 60;

pub fn load_asset_price(
    deps: Deps,
    oracle: Addr,
    asset: &AssetInfoRaw,
    check_expire: bool,
) -> StdResult<Decimal> {
    let config: Config = read_config(deps.storage)?;

    // check if the asset has a stored end_price or pre_ipo_price
    let stored_price = read_fixed_price(deps.storage, asset);

    let price: Decimal = if let Some(stored_price) = stored_price {
        stored_price
    } else {
        let asset_denom: String = (asset.to_normal(deps.api)?).to_string();
        if asset_denom == config.base_denom {
            Decimal::one()
        } else {
            // fetch price from oracle
            query_price(&deps.querier, oracle, asset_denom, None, check_expire)?
        }
    };

    Ok(price)
}

pub fn load_collateral_info(
    deps: Deps,
    collateral_oracle: Addr,
    collateral: &AssetInfoRaw,
    check_expire: bool,
) -> StdResult<(Decimal, Decimal, bool)> {
    let config: Config = read_config(deps.storage)?;
    let collateral_denom: String = (collateral.to_normal(deps.api)?).to_string();

    // base collateral
    if collateral_denom == config.base_denom {
        return Ok((Decimal::one(), Decimal::one(), false));
    }

    // check if the collateral is a revoked mAsset, will ignore pre_ipo_price since all preIPO
    // assets are not whitelisted in collateral oracle
    let end_price = read_fixed_price(deps.storage, collateral);

    if let Some(end_price) = end_price {
        // load collateral_multiplier from collateral oracle
        // if asset is revoked, no need to check for old price
        let (collateral_multiplier, _) =
            query_collateral_info(&deps.querier, collateral_oracle, collateral_denom)?;

        Ok((end_price, collateral_multiplier, true))
    } else {
        // load collateral info from collateral oracle
        let (collateral_oracle_price, collateral_multiplier, is_revoked) = query_collateral(
            &deps.querier,
            collateral_oracle,
            collateral_denom,
            check_expire,
        )?;

        Ok((collateral_oracle_price, collateral_multiplier, is_revoked))
    }
}

pub fn query_price(
    querier: &QuerierWrapper,
    oracle: Addr,
    base_asset: String,
    quote_asset: Option<String>,
    check_expire: bool,
) -> StdResult<Decimal> {
    let timeframe: Option<u64> = if check_expire {
        Some(PRICE_EXPIRE_TIME)
    } else {
        None
    };
    let base_res: PriceResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: oracle.to_string(),
        msg: to_binary(&OracleQueryMsg::Price {
            asset_token: base_asset,
            timeframe,
        })?,
    }))?;

    let rate = if let Some(quote_asset) = quote_asset {
        let quote_res: PriceResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: oracle.to_string(),
            msg: to_binary(&OracleQueryMsg::Price {
                asset_token: quote_asset,
                timeframe,
            })?,
        }))?;

        decimal_division(base_res.rate, quote_res.rate)
    } else {
        base_res.rate
    };

    Ok(rate)
}

// queries the collateral oracle to get the asset rate and multiplier
pub fn query_collateral(
    querier: &QuerierWrapper,
    collateral_oracle: Addr,
    asset: String,
    check_expire: bool,
) -> StdResult<(Decimal, Decimal, bool)> {
    let timeframe: Option<u64> = if check_expire {
        Some(PRICE_EXPIRE_TIME)
    } else {
        None
    };
    let res: CollateralPriceResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: collateral_oracle.to_string(),
        msg: to_binary(&CollateralOracleQueryMsg::CollateralPrice { asset, timeframe })?,
    }))?;

    Ok((res.rate, res.multiplier, res.is_revoked))
}

// queries only collateral information (multiplier and is_revoked), without price
pub fn query_collateral_info(
    querier: &QuerierWrapper,
    collateral_oracle: Addr,
    asset: String,
) -> StdResult<(Decimal, bool)> {
    let res: CollateralInfoResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: collateral_oracle.to_string(),
        msg: to_binary(&CollateralOracleQueryMsg::CollateralAssetInfo { asset })?,
    }))?;

    Ok((res.multiplier, res.is_revoked))
}
