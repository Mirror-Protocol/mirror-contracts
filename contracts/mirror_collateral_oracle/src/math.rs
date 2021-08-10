use cosmwasm_std::{Decimal, Uint128};

const DECIMAL_FRACTIONAL: Uint128 = Uint128::new(1_000_000_000u128);

/// return a / b
pub fn decimal_division(a: Decimal, b: Decimal) -> Decimal {
    Decimal::from_ratio(DECIMAL_FRACTIONAL * a, b * DECIMAL_FRACTIONAL)
}

pub fn decimal_multiplication(a: Decimal, b: Decimal) -> Decimal {
    Decimal::from_ratio(a * DECIMAL_FRACTIONAL * b, DECIMAL_FRACTIONAL)
}
