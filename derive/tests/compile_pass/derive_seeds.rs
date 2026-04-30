//! Compile-pass tests for `#[derive(Seeds)]`.
#![allow(unexpected_cfgs)]
extern crate alloc;

use quasar_derive::Seeds;
use quasar_lang::address::AddressVerify;

// -- Basic: prefix only -------------------------------------------------------

#[derive(Seeds)]
#[seeds(b"test")]
pub struct TestPda;

// -- With Address seed --------------------------------------------------------

#[derive(Seeds)]
#[seeds(b"vault", authority: Address)]
pub struct VaultPda;

// -- With Address + u64 seeds -------------------------------------------------

#[derive(Seeds)]
#[seeds(b"indexed", authority: Address, index: u64)]
pub struct IndexedPda;

fn main() {
    // Verify TestPda::seeds() exists and returns the SeedSet.
    let set = TestPda::seeds();
    let slices = set.as_slices();
    assert_eq!(slices.len(), 1);
    assert_eq!(slices[0], b"test");

    // with_bump produces the WithBump variant.
    let bumped = TestPda::seeds().with_bump(254);
    let slices = bumped.as_slices();
    assert_eq!(slices.len(), 2);
    assert_eq!(slices[0], b"test");
    assert_eq!(slices[1], &[254]);

    // VaultPda::seeds() takes an Address ref.
    let addr = solana_address::Address::default();
    let set = VaultPda::seeds(&addr);
    let slices = set.as_slices();
    assert_eq!(slices.len(), 2);
    assert_eq!(slices[0], b"vault");
    assert_eq!(slices[1], &[0u8; 32]);

    // IndexedPda::seeds() takes Address ref + u64.
    let set = IndexedPda::seeds(&addr, 42u64);
    let slices = set.as_slices();
    assert_eq!(slices.len(), 3);
    assert_eq!(slices[0], b"indexed");
    assert_eq!(slices[1], &[0u8; 32]);
    assert_eq!(slices[2], &42u64.to_le_bytes());

    // Verify AddressVerify trait is implemented (type-level check).
    fn _assert_verify<T: AddressVerify>() {}
    _assert_verify::<TestPdaSeedSet>();
    _assert_verify::<TestPdaSeedSetWithBump>();
    _assert_verify::<VaultPdaSeedSet>();
    _assert_verify::<VaultPdaSeedSetWithBump>();
    _assert_verify::<IndexedPdaSeedSet>();
    _assert_verify::<IndexedPdaSeedSetWithBump>();
}
