//! The pool's arithmetic never gives a user more than the pool owes.

use amm::math::{FEE_DENOMINATOR, deposit, swap, withdraw};
use proptest::{prelude::*, test_runner::RngSeed};

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 10_000,
        rng_seed: RngSeed::Fixed(0x414d_4d00),
        ..ProptestConfig::default()
    })]

    #[test]
    fn a_swap_never_lowers_the_product_of_the_reserves(
        reserve_in in 1u64..1 << 40,
        reserve_out in 1u64..1 << 40,
        amount_in in 0u64..1 << 40,
        fee in 0u64..FEE_DENOMINATOR,
    ) {
        let out = swap(amount_in, reserve_in, reserve_out, fee).unwrap();
        prop_assert!(out < reserve_out || amount_in == 0 && out == 0);
        let before = u128::from(reserve_in) * u128::from(reserve_out);
        let after = u128::from(reserve_in + amount_in) * u128::from(reserve_out - out);
        prop_assert!(after >= before);
    }

    #[test]
    fn swapping_there_and_back_gives_no_profit(
        reserve_a in 1u64..1 << 40,
        reserve_b in 1u64..1 << 40,
        amount in 0u64..1 << 40,
        fee in 0u64..FEE_DENOMINATOR,
    ) {
        let out = swap(amount, reserve_a, reserve_b, fee).unwrap();
        let back = swap(out, reserve_b - out, reserve_a + amount, fee).unwrap();
        prop_assert!(back <= amount);
    }

    #[test]
    fn depositing_then_withdrawing_gives_no_profit(
        reserve_a in 1u64..1 << 40,
        reserve_b in 1u64..1 << 40,
        supply in 1u64..1 << 40,
        a in 0u64..1 << 40,
        b in 0u64..1 << 40,
    ) {
        let (shares, paid_a, paid_b) = deposit(a, b, reserve_a, reserve_b, supply).unwrap();
        prop_assert!(paid_a <= a && paid_b <= b);
        let (out_a, out_b) =
            withdraw(shares, reserve_a + paid_a, reserve_b + paid_b, supply + shares).unwrap();
        prop_assert!(out_a <= paid_a && out_b <= paid_b);
    }

    #[test]
    fn withdrawing_every_share_takes_at_most_the_reserves(
        reserve_a in 0u64..1 << 40,
        reserve_b in 0u64..1 << 40,
        supply in 1u64..1 << 40,
        shares in 0u64..1 << 40,
    ) {
        let shares = shares.min(supply);
        let (a, b) = withdraw(shares, reserve_a, reserve_b, supply).unwrap();
        prop_assert!(a <= reserve_a && b <= reserve_b);
    }
}
