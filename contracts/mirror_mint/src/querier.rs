use cosmwasm_std::{
    to_binary, Api, Decimal, Extern, HumanAddr, Querier, QueryRequest, StdError, StdResult,
    Storage, WasmQuery,
};

use crate::state::{read_config, read_end_price, Config};
use mirror_protocol::collateral_oracle::{
    CollateralPriceResponse, QueryMsg as CollateralOracleQueryMsg,
};
use mirror_protocol::oracle::{PriceResponse, QueryMsg as OracleQueryMsg};
use terraswap::asset::AssetInfoRaw;

const PRICE_EXPIRE_TIME: u64 = 60;

pub fn load_asset_price<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    oracle: &HumanAddr,
    asset: &AssetInfoRaw,
    block_time: Option<u64>,
) -> StdResult<Decimal> {
    let config: Config = read_config(&deps.storage)?;

    let end_price = read_end_price(&deps.storage, &asset);
    let asset_denom: String = (asset.to_normal(&deps)?).to_string();

    let price: Decimal = if let Some(end_price) = end_price {
        end_price
    } else {
        if asset_denom == config.base_denom {
            Decimal::one()
        } else {
            query_price(deps, oracle, asset_denom, config.base_denom, block_time)?
        }
    };

    Ok(price)
}

pub fn load_collateral_info<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    collateral_oracle: &HumanAddr,
    collateral: &AssetInfoRaw,
) -> StdResult<(Decimal, Decimal)> {
    let config: Config = read_config(&deps.storage)?;

    let end_price = read_end_price(&deps.storage, &collateral);
    let collateral_denom: String = (collateral.to_normal(&deps)?).to_string();
    if collateral_denom == config.base_denom {
        return Ok((Decimal::one(), Decimal::zero()));
    }

    let (collateral_oracle_price, collateral_premium) =
        query_collateral(deps, collateral_oracle, collateral_denom.clone())?;

    let price = if let Some(end_price) = end_price {
        end_price
    } else {
        collateral_oracle_price
    };

    Ok((price, collateral_premium))
}

pub fn query_price<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    oracle: &HumanAddr,
    base_asset: String,
    quote_asset: String,
    block_time: Option<u64>,
) -> StdResult<Decimal> {
    let res: PriceResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: HumanAddr::from(oracle),
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

// queries the collateral oracle to get the asset rate and collateral_premium
pub fn query_collateral<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    collateral_oracle: &HumanAddr,
    asset: String,
) -> StdResult<(Decimal, Decimal)> {
    let res: CollateralPriceResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: HumanAddr::from(collateral_oracle),
            msg: to_binary(&CollateralOracleQueryMsg::CollateralPrice { asset })?,
        }))?;

    Ok((res.rate, res.collateral_premium))
}
