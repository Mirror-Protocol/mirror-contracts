use crate::math::{
    decimal_division, decimal_multiplication, decimal_subtraction, erf_plus_one, Sign,
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};
use mirror_protocol::short_reward::{
    ExecuteMsg, InstantiateMsg, QueryMsg, ShortRewardWeightResponse,
};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> StdResult<Response> {
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: ExecuteMsg,
) -> StdResult<Response> {
    Ok(Response::default())
}

pub fn query(_deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::ShortRewardWeight { premium_rate } => {
            to_binary(&query_short_reward_weight(premium_rate)?)
        }
    }
}

pub fn query_short_reward_weight(premium_rate: Decimal) -> StdResult<ShortRewardWeightResponse> {
    if premium_rate > Decimal::percent(7) {
        return Ok(ShortRewardWeightResponse {
            short_reward_weight: Decimal::percent(40),
        });
    }

    let one = Decimal::one();
    let two = one + one;
    let e10 = 10000000000u128;
    let sqrt_two = Decimal::from_ratio(14142135624u128, e10);

    let p = decimal_multiplication(premium_rate, Decimal::from_ratio(100u128, 1u128));
    let (sign_x, x) = if p > two {
        (
            Sign::Positive,
            decimal_division(decimal_subtraction(p, two)?, sqrt_two),
        )
    } else {
        (
            Sign::Negative,
            decimal_division(decimal_subtraction(two, p)?, sqrt_two),
        )
    };

    let short_reward_weight: Decimal =
        decimal_division(erf_plus_one(sign_x, x)?, Decimal::from_ratio(5u128, 1u128));
    return Ok(ShortRewardWeightResponse {
        short_reward_weight,
    });
}
