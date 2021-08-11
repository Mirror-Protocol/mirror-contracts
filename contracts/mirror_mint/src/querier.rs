use cosmwasm_std::{to_binary, Addr, Decimal, Deps, QueryRequest, StdError, StdResult, WasmQuery};

use crate::state::{read_config, read_fixed_price, Config};
use mirror_protocol::collateral_oracle::{
    CollateralPriceResponse, QueryMsg as CollateralOracleQueryMsg,
};
use mirror_protocol::oracle::{FeederResponse, PriceResponse, QueryMsg as OracleQueryMsg};
use terraswap::asset::AssetInfoRaw;

const PRICE_EXPIRE_TIME: u64 = 60;

pub fn load_oracle_feeder(deps: Deps, contract_addr: Addr, asset_token: Addr) -> StdResult<Addr> {
    let res: StdResult<FeederResponse> =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: contract_addr.to_string(),
            msg: to_binary(&OracleQueryMsg::Feeder {
                asset_token: asset_token.to_string(),
            })?,
        }));

    let feeder: Addr = match res {
        Ok(v) => deps.api.addr_validate(v.feeder.as_str())?,
        Err(_) => {
            return Err(StdError::generic_err("Failed to fetch the oracle feeder"));
        }
    };

    Ok(feeder)
}

pub fn load_asset_price(
    deps: Deps,
    oracle: Addr,
    asset: &AssetInfoRaw,
    block_time: Option<u64>,
) -> StdResult<Decimal> {
    let config: Config = read_config(deps.storage)?;

    // check if the asset has a stored end_price or pre_ipo_price
    let stored_price = read_fixed_price(deps.storage, &asset);

    let price: Decimal = if let Some(stored_price) = stored_price {
        stored_price
    } else {
        let asset_denom: String = (asset.to_normal(deps.api)?).to_string();
        if asset_denom == config.base_denom {
            Decimal::one()
        } else {
            // fetch price from oracle
            query_price(deps, oracle, asset_denom, config.base_denom, block_time)?
        }
    };

    Ok(price)
}

pub fn load_collateral_info(
    deps: Deps,
    collateral_oracle: Addr,
    collateral: &AssetInfoRaw,
    block_time: Option<u64>,
    block_height: Option<u64>,
) -> StdResult<(Decimal, Decimal, bool)> {
    let config: Config = read_config(deps.storage)?;
    let collateral_denom: String = (collateral.to_normal(deps.api)?).to_string();

    // base collateral
    if collateral_denom == config.base_denom {
        return Ok((Decimal::one(), Decimal::one(), false));
    }

    // check if the collateral is a revoked mAsset, will ignore pre_ipo_price since all preIPO
    // assets are not whitelisted in collateral oracle
    let end_price = read_fixed_price(deps.storage, &collateral);

    if let Some(end_price) = end_price {
        // load collateral_multiplier from collateral oracle
        // if asset is revoked, no need to check for old price
        let (_, collateral_multiplier, _) = query_collateral(
            deps,
            collateral_oracle,
            collateral_denom,
            None,
            block_height,
        )?;

        Ok((end_price, collateral_multiplier, true))
    } else {
        // load collateral info from collateral oracle
        let (collateral_oracle_price, collateral_multiplier, is_revoked) = query_collateral(
            deps,
            collateral_oracle,
            collateral_denom,
            block_time,
            block_height,
        )?;

        Ok((collateral_oracle_price, collateral_multiplier, is_revoked))
    }
}

pub fn query_price(
    deps: Deps,
    oracle: Addr,
    base_asset: String,
    quote_asset: String,
    block_time: Option<u64>,
) -> StdResult<Decimal> {
    let res: PriceResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: oracle.to_string(),
        msg: to_binary(&OracleQueryMsg::Price {
            base_asset,
            quote_asset,
        })?,
    }))?;

    if let Some(block_time) = block_time {
        if res.last_updated_base < (block_time - PRICE_EXPIRE_TIME)
            || res.last_updated_quote < (block_time - PRICE_EXPIRE_TIME)
        {
            return Err(StdError::generic_err("Price is too old"));
        }
    }

    Ok(res.rate)
}

// queries the collateral oracle to get the asset rate and multiplier
pub fn query_collateral(
    deps: Deps,
    collateral_oracle: Addr,
    asset: String,
    block_time: Option<u64>,
    block_height: Option<u64>,
) -> StdResult<(Decimal, Decimal, bool)> {
    let res: CollateralPriceResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: collateral_oracle.to_string(),
            msg: to_binary(&CollateralOracleQueryMsg::CollateralPrice {
                asset,
                block_height,
            })?,
        }))?;

    if let Some(block_time) = block_time {
        if res.last_updated < (block_time - PRICE_EXPIRE_TIME) {
            return Err(StdError::generic_err("Collateral price is too old"));
        }
    }

    Ok((res.rate, res.multiplier, res.is_revoked))
}
