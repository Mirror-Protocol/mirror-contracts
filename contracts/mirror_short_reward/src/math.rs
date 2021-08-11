use cosmwasm_std::{Decimal, StdResult, Uint128};

const DECIMAL_FRACTIONAL: Uint128 = Uint128::new(1_000_000_000u128);

pub fn decimal_division(a: Decimal, b: Decimal) -> Decimal {
    Decimal::from_ratio(DECIMAL_FRACTIONAL * a, b * DECIMAL_FRACTIONAL)
}

pub fn decimal_subtraction(a: Decimal, b: Decimal) -> StdResult<Decimal> {
    Ok(Decimal::from_ratio(
        (a * DECIMAL_FRACTIONAL).checked_sub(b * DECIMAL_FRACTIONAL)?,
        DECIMAL_FRACTIONAL,
    ))
}

pub fn decimal_multiplication(a: Decimal, b: Decimal) -> Decimal {
    Decimal::from_ratio(a * DECIMAL_FRACTIONAL * b, DECIMAL_FRACTIONAL)
}

pub fn reverse_decimal(decimal: Decimal) -> Decimal {
    Decimal::from_ratio(DECIMAL_FRACTIONAL, decimal * DECIMAL_FRACTIONAL)
}

// p == premium * 100
pub fn erf_plus_one(sign_x: Sign, x: Decimal) -> StdResult<Decimal> {
    let e6 = 1000000u128;
    let e10 = 10000000000u128;
    let a1 = Decimal::from_ratio(705230784u128, e10);
    let a2 = Decimal::from_ratio(422820123u128, e10);
    let a3 = Decimal::from_ratio(92705272u128, e10);
    let a4 = Decimal::from_ratio(1520143u128, e10);
    let a5 = Decimal::from_ratio(2765672u128, e10);
    let a6 = Decimal::from_ratio(430638u128, e10);

    let one = Decimal::one();
    let two = one + one;

    // ((((((a6 * x) + a5) * x + a4 ) * x + a3) * x + a2) * x + a1) * x + 1
    let sign = sign_x.clone();
    let num = decimal_multiplication(a6, x);
    let (sign, num) = sum_and_multiply_x(sign, Sign::Positive, sign_x.clone(), num, a5, x)?;
    let (sign, num) = sum_and_multiply_x(sign, Sign::Positive, sign_x.clone(), num, a4, x)?;
    let (sign, num) = sum_and_multiply_x(sign, Sign::Positive, sign_x.clone(), num, a3, x)?;
    let (sign, num) = sum_and_multiply_x(sign, Sign::Positive, sign_x.clone(), num, a2, x)?;
    let (sign, num) = sum_and_multiply_x(sign, Sign::Positive, sign_x, num, a1, x)?;
    let num = if sign == Sign::Positive {
        num + one
    } else if num > one {
        decimal_subtraction(num, one)?
    } else {
        decimal_subtraction(one, num)?
    };

    // ignore sign
    let num2 = decimal_multiplication(num, num);
    let num4 = decimal_multiplication(num2, num2);
    let num8 = decimal_multiplication(num4, num4);
    let num16 = decimal_multiplication(num8, num8);
    let reverse_num16 = reverse_decimal(num16);
    if reverse_num16 > two {
        return Ok(Decimal::zero());
    }

    let erf_plus_one = decimal_subtraction(two, reverse_num16);

    // maximum error: 3 * 10^-7
    // so only use 6 decimal digits
    Ok(Decimal::from_ratio(erf_plus_one? * e6.into(), e6))
}

#[derive(PartialEq, Clone)]
pub enum Sign {
    Positive,
    Negative,
}

// return (sign, result)
fn sum_and_multiply_x(
    sign_1: Sign,
    sign_2: Sign,
    sign_x: Sign,
    num1: Decimal,
    num2: Decimal,
    x: Decimal,
) -> StdResult<(Sign, Decimal)> {
    if sign_1 == sign_2 {
        let val = decimal_multiplication(num1 + num2, x);
        if sign_1 == sign_x {
            Ok((Sign::Positive, val))
        } else {
            Ok((Sign::Negative, val))
        }
    } else if num1 > num2 {
        let val = decimal_multiplication(decimal_subtraction(num1, num2)?, x);
        if sign_1 == sign_x {
            Ok((Sign::Positive, val))
        } else {
            Ok((Sign::Negative, val))
        }
    } else {
        let val = decimal_multiplication(decimal_subtraction(num2, num1)?, x);
        if sign_2 == sign_x {
            Ok((Sign::Positive, val))
        } else {
            Ok((Sign::Negative, val))
        }
    }
}
