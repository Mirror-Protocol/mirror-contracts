use cosmwasm_std::{Decimal, Uint128};

const DECIMAL_FRACTIONAL: Uint128 = Uint128(1_000_000_000u128);

pub fn reverse_decimal(decimal: Decimal) -> Decimal {
    Decimal::from_ratio(DECIMAL_FRACTIONAL, decimal * DECIMAL_FRACTIONAL)
}

pub fn decimal_multiplication(a: Decimal, b: Decimal) -> Decimal {
    Decimal::from_ratio(a * DECIMAL_FRACTIONAL * b, DECIMAL_FRACTIONAL)
}

pub fn decimal_subtraction(a: Decimal, b: Decimal) -> Decimal {
    Decimal::from_ratio(
        (DECIMAL_FRACTIONAL * a - DECIMAL_FRACTIONAL * b).unwrap(),
        DECIMAL_FRACTIONAL,
    )
}
