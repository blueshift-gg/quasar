//! The pool's arithmetic. Every amount a user receives rounds down and every
//! amount a user pays rounds up, so rounding always favours the pool.

use quasar_lang::math::{mul_div_ceil, mul_div_floor};

/// Fees are in basis points of the amount paid in.
pub const FEE_DENOMINATOR: u64 = 10_000;

/// A deposit of at most `a` and `b` into reserves `reserve_a` and `reserve_b`
/// with `supply` shares: the shares it earns and the amounts it takes.
pub fn deposit(
    a: u64,
    b: u64,
    reserve_a: u64,
    reserve_b: u64,
    supply: u64,
) -> Option<(u64, u64, u64)> {
    if supply == 0 {
        // The first deposit sets the price; its shares are `√(a · b)`.
        let shares = (u128::from(a) * u128::from(b)).isqrt();
        return Some((u64::try_from(shares).ok()?, a, b));
    }
    let shares = mul_div_floor(a, supply, reserve_a)?.min(mul_div_floor(b, supply, reserve_b)?);
    Some((
        shares,
        mul_div_ceil(shares, reserve_a, supply)?,
        mul_div_ceil(shares, reserve_b, supply)?,
    ))
}

/// What `shares` of `supply` take out of the reserves.
pub fn withdraw(shares: u64, reserve_a: u64, reserve_b: u64, supply: u64) -> Option<(u64, u64)> {
    Some((mul_div_floor(shares, reserve_a, supply)?, mul_div_floor(shares, reserve_b, supply)?))
}

/// What `amount_in` buys, after the fee, from a pool holding `reserve_in`
/// and `reserve_out`: the product of the reserves never falls.
pub fn swap(amount_in: u64, reserve_in: u64, reserve_out: u64, fee_bps: u64) -> Option<u64> {
    let after_fee =
        mul_div_floor(amount_in, FEE_DENOMINATOR.checked_sub(fee_bps)?, FEE_DENOMINATOR)?;
    mul_div_floor(reserve_out, after_fee, reserve_in.checked_add(after_fee)?)
}
