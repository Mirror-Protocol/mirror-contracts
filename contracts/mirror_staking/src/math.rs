use cosmwasm_std::{Decimal, Uint128};

const DECIMAL_FRACTIONAL: Uint128 = Uint128(1_000_000_000u128);

/// return a / b
pub fn decimal_division(a: Decimal, b: Decimal) -> Decimal {
    Decimal::from_ratio(DECIMAL_FRACTIONAL * a, b * DECIMAL_FRACTIONAL)
}

pub fn decimal_subtraction(a: Decimal, b: Decimal) -> Decimal {
    Decimal::from_ratio(
        (DECIMAL_FRACTIONAL * a - DECIMAL_FRACTIONAL * b).unwrap(),
        DECIMAL_FRACTIONAL,
    )
}

pub fn decimal_multiplication(a: Decimal, b: Decimal) -> Decimal {
    Decimal::from_ratio(a * DECIMAL_FRACTIONAL * b, DECIMAL_FRACTIONAL)
}

pub fn reverse_decimal(decimal: Decimal) -> Decimal {
    Decimal::from_ratio(DECIMAL_FRACTIONAL, decimal * DECIMAL_FRACTIONAL)
}

// p == premium * 100
fn erf_plus_one(sign_x: Sign, x: Decimal) -> Decimal {
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
    let (sign, num) = sum_and_multiply_x(sign, Sign::Positive, sign_x.clone(), num, a5, x);
    let (sign, num) = sum_and_multiply_x(sign, Sign::Positive, sign_x.clone(), num, a4, x);
    let (sign, num) = sum_and_multiply_x(sign, Sign::Positive, sign_x.clone(), num, a3, x);
    let (sign, num) = sum_and_multiply_x(sign, Sign::Positive, sign_x.clone(), num, a2, x);
    let (sign, num) = sum_and_multiply_x(sign, Sign::Positive, sign_x, num, a1, x);
    let num = if sign == Sign::Positive {
        num + one
    } else if num > one {
        decimal_subtraction(num, one)
    } else {
        decimal_subtraction(one, num)
    };

    // ignore sign
    let num2 = decimal_multiplication(num, num);
    let num4 = decimal_multiplication(num2, num2);
    let num8 = decimal_multiplication(num4, num4);
    let num16 = decimal_multiplication(num8, num8);
    let reverse_num16 = reverse_decimal(num16);
    if reverse_num16 > two {
        return Decimal::zero();
    }

    let erf_plus_one = decimal_subtraction(two, reverse_num16);

    // maximum error: 3 * 10^-7
    // so only use 6 decimal digits
    Decimal::from_ratio(erf_plus_one * e6.into(), e6)
}

// (1 + erf(premium * 100)) / 5
pub fn short_reward_weight(premium_rate: Decimal) -> Decimal {
    if premium_rate > Decimal::percent(7) {
        return Decimal::percent(40);
    }

    let one = Decimal::one();
    let two = one + one;
    let e10 = 10000000000u128;
    let sqrt_two = Decimal::from_ratio(14142135624u128, e10);

    let p = decimal_multiplication(premium_rate, Decimal::from_ratio(100u128, 1u128));
    let (sign_x, x) = if p > two {
        (
            Sign::Positive,
            decimal_division(decimal_subtraction(p, two), sqrt_two),
        )
    } else {
        (
            Sign::Negative,
            decimal_division(decimal_subtraction(two, p), sqrt_two),
        )
    };

    return decimal_division(erf_plus_one(sign_x, x), Decimal::from_ratio(5u128, 1u128));
}

#[derive(PartialEq, Clone)]
enum Sign {
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
) -> (Sign, Decimal) {
    if sign_1 == sign_2 {
        let val = decimal_multiplication(num1 + num2, x);
        if sign_1 == sign_x {
            (Sign::Positive, val)
        } else {
            (Sign::Negative, val)
        }
    } else if num1 > num2 {
        let val = decimal_multiplication(decimal_subtraction(num1, num2), x);
        if sign_1 == sign_x {
            (Sign::Positive, val)
        } else {
            (Sign::Negative, val)
        }
    } else {
        let val = decimal_multiplication(decimal_subtraction(num2, num1), x);
        if sign_2 == sign_x {
            (Sign::Positive, val)
        } else {
            (Sign::Negative, val)
        }
    }
}

#[test]
fn erf_plus_one_test() {
    let e6 = 1000000u128;
    let e10 = 10000000000u128;
    assert_eq!(
        erf_plus_one(Sign::Negative, Decimal::from_ratio(21213203435u128, e10)),
        Decimal::zero()
    );
    assert_eq!(
        erf_plus_one(Sign::Negative, Decimal::from_ratio(14142135623u128, e10)),
        Decimal::from_ratio(013090u128, e6)
    );
    assert_eq!(
        erf_plus_one(Sign::Positive, Decimal::zero()),
        Decimal::from_ratio(1000000u128, e6)
    );
    assert_eq!(
        erf_plus_one(Sign::Positive, Decimal::from_ratio(14142135623u128, e10)),
        Decimal::from_ratio(1954499u128, e6)
    );
}
