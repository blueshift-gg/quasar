//! `a · b / c` without overflow in between, rounded the way the caller
//! chooses: down for what a user receives, up for what a user pays, so
//! rounding never favours the user.

/// `⌊a · b / c⌋`, or `None` if `c` is zero or the result does not fit a `u64`.
pub fn mul_div_floor(a: u64, b: u64, c: u64) -> Option<u64> {
    if c == 0 {
        return None;
    }
    // Dividing a `u128` is a library call on SBF: about 190 compute units more,
    // measured. Most products fit a `u64`.
    match a.checked_mul(b) {
        Some(product) => Some(product / c),
        None => u64::try_from(u128::from(a) * u128::from(b) / u128::from(c)).ok(),
    }
}

/// `⌈a · b / c⌉`, or `None` if `c` is zero or the result does not fit a `u64`.
pub fn mul_div_ceil(a: u64, b: u64, c: u64) -> Option<u64> {
    if c == 0 {
        return None;
    }
    match a.checked_mul(b) {
        Some(product) => Some(product.div_ceil(c)),
        None => u64::try_from((u128::from(a) * u128::from(b)).div_ceil(u128::from(c))).ok(),
    }
}

#[cfg(test)]
mod tests {
    use proptest::{prelude::*, test_runner::RngSeed};

    use super::*;

    /// The exact result, from `u128` arithmetic alone.
    fn exact(a: u64, b: u64, c: u64) -> Option<(u128, u128)> {
        let product = u128::from(a) * u128::from(b);
        (c != 0).then(|| (product / u128::from(c), product.div_ceil(u128::from(c))))
    }

    /// Operands that often overflow a `u64` product, and often do not.
    fn operand() -> impl Strategy<Value = u64> {
        prop_oneof![any::<u64>(), 0u64..1 << 20, u64::MAX - (1 << 20)..=u64::MAX]
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 20_000,
            rng_seed: RngSeed::Fixed(0x4d41_5448),
            ..ProptestConfig::default()
        })]

        #[test]
        fn both_roundings_equal_the_exact_result(a in operand(), b in operand(), c in operand()) {
            let fits = |value: u128| u64::try_from(value).ok();
            let (floor, ceil) = match exact(a, b, c) {
                Some((floor, ceil)) => (fits(floor), fits(ceil)),
                None => (None, None),
            };
            prop_assert_eq!(mul_div_floor(a, b, c), floor);
            prop_assert_eq!(mul_div_ceil(a, b, c), ceil);
        }
    }
}
