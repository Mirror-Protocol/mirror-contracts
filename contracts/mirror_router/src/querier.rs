use cosmwasm_std::{
    to_binary, Api, Decimal, Extern, HumanAddr, Querier, QueryRequest, StdError, StdResult,
    Storage, Uint128, WasmQuery,
};
use mirror_protocol::oracle::{PriceResponse, QueryMsg as OracleQueryMsg};
use terra_cosmwasm::TerraQuerier;

const PRICE_EXPIRE_TIME: u64 = 60;

static DECIMAL_FRACTION: Uint128 = Uint128(1_000_000_000_000_000_000u128);

pub fn compute_tax<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    amount: Uint128,
    denom: String,
) -> StdResult<Uint128> {
    let terra_querier = TerraQuerier::new(&deps.querier);
    let tax_rate: Decimal = (terra_querier.query_tax_rate()?).rate;
    let tax_cap: Uint128 = (terra_querier.query_tax_cap(denom.to_string())?).cap;
    Ok(std::cmp::min(
        (amount
            - amount.multiply_ratio(
                DECIMAL_FRACTION,
                DECIMAL_FRACTION * tax_rate + DECIMAL_FRACTION,
            ))?,
        tax_cap,
    ))
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
