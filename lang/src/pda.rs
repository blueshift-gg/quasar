//! Program-derived addresses: `sha256(seeds, bump, program, "ProgramDerivedAddress")`
//! off Ed25519. The canonical bump is the highest whose address is off the
//! curve: finding it takes a curve check per bump, done at compile time for
//! literal seeds. An account whose owner shows this program created it
//! (`Wrapper::ASSIGNED`) is at the canonical bump already, and its check needs
//! only the hash; any other account, which anyone can fund at the address of
//! a lower bump, must be at the canonical one.

use core::{
    cell::Cell,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::{Deref, DerefMut, RangeInclusive},
    ptr, slice,
};

use solana_address::{MAX_SEEDS, PDA_MARKER};

use crate::{
    Address, ProgramError, Result,
    account::{Account, Kind},
    input::{Checked, Entry, Privileges},
    syscalls::{self, RawSlice},
    wrapper::{AfterCpi, Wrapper},
};

/// The seeds of a PDA, as `#[seeds(..)]` declares them: `Escrow::seeds(&maker)`.
/// Seeds that are all literals carry their address and bump, found at
/// compile time.
pub struct Seeds<'a, const N: usize> {
    seeds: [&'a [u8]; N],
    found: Option<(Address, u8)>,
}

impl<'a, const N: usize> Seeds<'a, N> {
    pub const fn new(seeds: [&'a [u8]; N]) -> Self {
        Self { seeds, found: None }
    }

    /// Seeds whose address and bump are already known.
    pub const fn found(seeds: [&'a [u8]; N], found: (Address, u8)) -> Self {
        Self { seeds, found: Some(found) }
    }

    /// The bump from which these seeds derive `account`'s address, or an
    /// error if they do not.
    #[inline(always)]
    pub fn bump_of<W: Wrapper>(&self, account: &W, program: &Address) -> Result<u8> {
        match &self.found {
            Some((address, bump)) => check(account.address(), address).map(|()| *bump),
            None if W::ASSIGNED => find_bump(account.address(), &self.seeds, program),
            None => canonical_bump(account.address(), &self.seeds, program),
        }
    }

    /// The canonical address and bump, for creating the account.
    pub fn canonical(&self, program: &Address) -> Result<(Address, u8)> {
        match self.found {
            Some(found) => Ok(found),
            None => find_program_address(&self.seeds, program),
        }
    }

    /// The seeds with their bump, to sign a call for the PDA.
    pub fn with_bump(self, bump: u8) -> SignerSeeds<'a, N> {
        SignerSeeds { seeds: self.seeds, bump: [bump] }
    }
}

/// A PDA's seeds and bump, which sign a call on its behalf.
pub struct SignerSeeds<'a, const N: usize> {
    pub(crate) seeds: [&'a [u8]; N],
    pub(crate) bump: [u8; 1],
}

impl<const N: usize> SignerSeeds<'_, N> {
    /// The bump, after checking that it derives `account`'s address: with
    /// one hash when the bump is stored in an account this program created.
    pub fn bump_of<W: Wrapper>(&self, account: &W, program: &Address) -> Result<u8> {
        let bump = self.bump[0];
        if W::ASSIGNED {
            verify(account.address(), &self.seeds, bump, program).map(|()| bump)
        } else {
            let canonical = canonical_bump(account.address(), &self.seeds, program)?;
            if canonical == bump { Ok(bump) } else { Err(ProgramError::InvalidSeeds) }
        }
    }
}

/// The account an accounts struct declares under the name `name(..)` gives
/// `NAME`: what `seeds`, written without values, takes for a seed of that
/// name. Implemented by `#[derive(Accounts)]`, once per field.
#[diagnostic::on_unimplemented(
    message = "a seed names no field of the kind and no account of the struct",
    label = "write the values: `seeds(..)`"
)]
pub trait Named<const NAME: u64> {
    fn named(&self) -> &Address;
}

/// A name as `Named` takes it: its FNV-1a hash. Two fields of one struct
/// whose names collide are two impls of one trait, which do not compile.
pub const fn name(name: &str) -> u64 {
    let bytes = name.as_bytes();
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut index = 0;
    while index < bytes.len() {
        hash = (hash ^ bytes[index] as u64).wrapping_mul(0x0000_0100_0000_01b3);
        index += 1;
    }
    hash
}

/// An account at a program-derived address, and the bump that derives it.
/// `S` declares the seeds: the kind the account holds, as in
/// `Pda<Account<Escrow>>`, or for an account without one a type with
/// `#[derive(Seeds)]`, as in `Pda<UncheckedAccount, VaultSeeds>`.
///
/// It loads only through `seeds(..)`, which checks the address and finds the
/// bump, and derefs to the account.
pub struct Pda<W, S = <W as Stores>::Kind> {
    account: W,
    bump: u8,
    seeds: PhantomData<S>,
}

impl<W, S> Pda<W, S> {
    pub fn bump(&self) -> u8 {
        self.bump
    }
}

impl<K: Kind, S> Pda<Account<K>, S> {
    /// `Account::set_inner`, here so the value can read the `Pda`, as in
    /// `escrow.set_inner(&Escrow { bump: escrow.bump(), .. })`.
    pub fn set_inner(&mut self, value: &K) -> Result {
        self.account.set_inner(value)
    }
}

/// The kind a wrapper stores, whose seeds a `Pda` of it takes by default.
pub trait Stores {
    type Kind;
}

impl<K> Stores for Account<K> {
    type Kind = K;
}

/// What `seeds(..)` on a field needs: the account it checks, and the type
/// whose `seeds` function it calls.
#[diagnostic::on_unimplemented(
    message = "`seeds(..)` is for a `Pda<..>` field, not `{Self}`",
    label = "declare the field as `Pda<{Self}>`"
)]
pub trait Seeded {
    type Account: Wrapper;
    type Seeds;

    /// The account, found at the address `bump` derives.
    fn found(account: Self::Account, bump: u8) -> Self;
}

impl<W: Wrapper, S> Seeded for Pda<W, S> {
    type Account = W;
    type Seeds = S;

    fn found(account: W, bump: u8) -> Self {
        Self { account, bump, seeds: PhantomData }
    }
}

impl<W: Wrapper, S> Wrapper for Pda<W, S> {
    const REQUIRES: Privileges = W::REQUIRES;
    const LEN: Option<usize> = W::LEN;
    const AFTER_CPI: AfterCpi = W::AFTER_CPI;
    const ASSIGNED: bool = W::ASSIGNED;
    const PROGRAM_OWNED: bool = W::PROGRAM_OWNED;
    const WIDTH: Option<usize> = W::WIDTH;
    const RESIZES: bool = W::RESIZES;
    const PDA: bool = true;
    const CHECK: Option<crate::cpi::Check> = W::CHECK;
    const FIELDS: &'static [&'static str] = W::FIELDS;

    /// Only a recheck after a call builds one this way, to check the account
    /// again; the bump is not read from it.
    fn from_entry(entry: Entry, checked: Checked) -> Result<Self> {
        W::from_entry(entry, checked).map(|account| Self::found(account, 0))
    }

    fn content_len(entry: &Entry) -> usize {
        W::content_len(entry)
    }

    fn why(entry: &Entry, name: &str) -> &'static str {
        W::why(entry, name)
    }

    #[cfg(not(target_os = "solana"))]
    fn idl_kind() -> Option<crate::client::IdlKind> {
        W::idl_kind()
    }

    fn entry(&self) -> &Entry {
        self.account.entry()
    }

    fn entry_mut(&mut self) -> &mut Entry {
        self.account.entry_mut()
    }

    fn into_entry(self) -> Entry {
        self.account.into_entry()
    }
}

impl<W, S> Deref for Pda<W, S> {
    type Target = W;

    fn deref(&self) -> &W {
        &self.account
    }
}

impl<W, S> DerefMut for Pda<W, S> {
    fn deref_mut(&mut self) -> &mut W {
        &mut self.account
    }
}

/// A value a seed of type `T` is made of: an `Address` seed takes an address
/// or any account, its address; a byte array or an integer seed, its bytes,
/// little-endian.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be a seed declared `{T}`",
    label = "an `Address` seed takes an address or an account; other seeds, a value of their type"
)]
pub trait Seed<T> {
    fn seed(&self) -> &[u8];
}

impl Seed<Address> for Address {
    fn seed(&self) -> &[u8] {
        self.as_ref()
    }
}

/// An address as a view's getter returns it.
impl Seed<Address> for &Address {
    fn seed(&self) -> &[u8] {
        self.as_ref()
    }
}

impl<W: Wrapper> Seed<Address> for W {
    fn seed(&self) -> &[u8] {
        self.address().as_ref()
    }
}

impl<const N: usize> Seed<[u8; N]> for [u8; N] {
    fn seed(&self) -> &[u8] {
        self
    }
}

macro_rules! integer_seed {
    ($($ty:ty),*) => {$(
        // Its bytes in memory are its little-endian bytes only on such a target.
        #[cfg(target_endian = "little")]
        impl Seed<$ty> for $ty {
            fn seed(&self) -> &[u8] {
                // SAFETY: an integer's bytes are all initialized, and on a
                // little-endian target they are its little-endian encoding.
                unsafe { slice::from_raw_parts(ptr::from_ref(self).cast(), size_of::<$ty>()) }
            }
        }
    )*};
}

integer_seed!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128);

/// Fails unless `key` is `expected`, a PDA found at compile time.
fn check(key: &Address, expected: &Address) -> Result {
    if key == expected { Ok(()) } else { Err(ProgramError::InvalidSeeds) }
}

/// Fails unless `key` is the address derived from `seeds` and `bump`: one hash.
fn verify<const N: usize>(
    key: &Address,
    seeds: &[&[u8]; N],
    bump: u8,
    program: &Address,
) -> Result {
    match search(seeds, program, bump..=bump, |address| Some(address == key.as_array())) {
        Some((true, _)) => Ok(()),
        _ => Err(ProgramError::InvalidSeeds),
    }
}

/// The bump from which `seeds` derive `key`, searching from 255 down with one
/// `sol_sha256` per bump and no curve check, so the bump need not be
/// canonical.
#[inline(always)]
fn find_bump<const N: usize>(key: &Address, seeds: &[&[u8]; N], program: &Address) -> Result<u8> {
    let found =
        search(seeds, program, ALL_BUMPS, |address| (address == key.as_array()).then_some(()));
    found.map(|((), bump)| bump).ok_or(ProgramError::InvalidSeeds)
}

/// The canonical bump, if `seeds` derive `key` from it.
fn canonical_bump<const N: usize>(
    key: &Address,
    seeds: &[&[u8]; N],
    program: &Address,
) -> Result<u8> {
    let is_key = |address: &[u8; 32]| off_curve(address).then(|| address == key.as_array());
    match search(seeds, program, ALL_BUMPS, is_key) {
        Some((true, bump)) => Ok(bump),
        _ => Err(ProgramError::InvalidSeeds),
    }
}

/// The address and canonical bump for `seeds`, found at run time: each bump,
/// from 255 down, costs one `sol_sha256` and one `sol_curve_validate_point`.
fn find_program_address<const N: usize>(
    seeds: &[&[u8]; N],
    program: &Address,
) -> Result<(Address, u8)> {
    let address =
        |address: &[u8; 32]| off_curve(address).then(|| Address::new_from_array(*address));
    search(seeds, program, ALL_BUMPS, address).ok_or(ProgramError::InvalidSeeds)
}

#[inline(always)]
fn off_curve(address: &[u8; 32]) -> bool {
    !syscalls::is_on_curve(address)
}

const ALL_BUMPS: RangeInclusive<u8> = 0..=u8::MAX;

/// Hashes `seeds, bump, program, "ProgramDerivedAddress"` for each of `bumps`
/// from the highest down, until `decide` answers for an address; that answer
/// and its bump. The hash input is laid out once; only the bump byte changes
/// between tries.
#[inline(always)]
fn search<const N: usize, T>(
    seeds: &[&[u8]; N],
    program: &Address,
    bumps: RangeInclusive<u8>,
    mut decide: impl FnMut(&[u8; 32]) -> Option<T>,
) -> Option<(T, u8)> {
    const { assert!(N < MAX_SEEDS, "a PDA has at most 15 seeds: the bump is the 16th") };
    let bump = Cell::new(0);
    let preimage = Preimage {
        seeds: seeds.map(RawSlice::new),
        bump: RawSlice { addr: bump.as_ptr(), len: 1 },
        program: RawSlice::new(program.as_ref()),
        marker: RawSlice::new(PDA_MARKER),
    };
    // SAFETY: `Preimage` is `N + 3` contiguous `RawSlice`s.
    let parts = unsafe { slice::from_raw_parts(ptr::from_ref(&preimage).cast(), N + 3) };
    // Outside the loop: one buffer the hash is written to and read from.
    let mut hash = MaybeUninit::uninit();
    // A `u64` counter and a hand-written loop: an iterator's costs more on SBF.
    let (lowest, mut candidate) = (u64::from(*bumps.start()), u64::from(*bumps.end()));
    loop {
        bump.set(candidate as u8);
        // SAFETY: every part points to live bytes: the seeds, `bump`, the
        // program id and the marker.
        let address = unsafe { syscalls::sha256(parts, &mut hash) };
        if let Some(answer) = decide(address) {
            return Some((answer, candidate as u8));
        }
        if candidate == lowest {
            return None;
        }
        candidate -= 1;
    }
}

/// A PDA's hash input as `sol_sha256` reads it.
#[repr(C)]
struct Preimage<const N: usize> {
    seeds: [RawSlice; N],
    bump: RawSlice,
    program: RawSlice,
    marker: RawSlice,
}

/// The address and canonical bump for `seeds`: the highest bump whose address
/// is off the curve.
pub const fn find<const N: usize>(seeds: &[&[u8]; N], program: &Address) -> (Address, u8) {
    let mut bump = u8::MAX;
    loop {
        let address = Address::derive_address_const(seeds, Some(bump), program);
        if !is_on_curve(address.as_array()) {
            return (address, bump);
        }
        assert!(bump > 0, "no bump gives an address off the curve");
        bump -= 1;
    }
}

/// Whether `bytes` decompress to a point on Ed25519: with `y` the encoded
/// coordinate, whether `(y² - 1) / (d·y² + 1)` is a square mod p.
pub const fn is_on_curve(bytes: &[u8; 32]) -> bool {
    let mut y = [0u64; 4];
    let mut i = 0;
    while i < 4 {
        let mut limb = [0u8; 8];
        let mut j = 0;
        while j < 8 {
            limb[j] = bytes[i * 8 + j];
            j += 1;
        }
        y[i] = u64::from_le_bytes(limb);
        i += 1;
    }
    y[3] &= (1 << 63) - 1; // the top bit is the sign of x
    let y = reduce(y);
    let y2 = mul(y, y);
    let u = sub(y2, ONE);
    let v = add(mul(D, y2), ONE);
    // `v` is never zero, so `u / v` is a square exactly when `u·v` is.
    let legendre = pow(mul(u, v), P_MINUS_1_OVER_2);
    eq(legendre, ONE) || eq(legendre, ZERO)
}

type Field = [u64; 4];

const ZERO: Field = [0; 4];
const ONE: Field = [1, 0, 0, 0];
/// p = 2^255 - 19.
const P: Field = [0xffff_ffff_ffff_ffed, u64::MAX, u64::MAX, 0x7fff_ffff_ffff_ffff];
/// (p - 1) / 2 = 2^254 - 10.
const P_MINUS_1_OVER_2: Field = [0xffff_ffff_ffff_fff6, u64::MAX, u64::MAX, 0x3fff_ffff_ffff_ffff];
/// d = -121665 / 121666 mod p (RFC 8032, section 5.1).
const D: Field =
    [0x75eb_4dca_1359_78a3, 0x0070_0a4d_4141_d8ab, 0x8cc7_4079_7779_e898, 0x5203_6cee_2b6f_fe73];

const fn eq(a: Field, b: Field) -> bool {
    a[0] == b[0] && a[1] == b[1] && a[2] == b[2] && a[3] == b[3]
}

const fn at_least(a: Field, b: Field) -> bool {
    let mut i = 4;
    while i > 0 {
        i -= 1;
        if a[i] != b[i] {
            return a[i] > b[i];
        }
    }
    true
}

/// `a - b` for `a >= b`.
const fn sub_raw(a: Field, b: Field) -> Field {
    let mut out = [0; 4];
    let mut borrow = false;
    let mut i = 0;
    while i < 4 {
        let (diff, under) = a[i].overflowing_sub(b[i]);
        let (diff, under_again) = diff.overflowing_sub(borrow as u64);
        out[i] = diff;
        borrow = under || under_again;
        i += 1;
    }
    out
}

/// `a mod p` for any `a < 2^256`, which is below `3p`.
const fn reduce(mut a: Field) -> Field {
    while at_least(a, P) {
        a = sub_raw(a, P);
    }
    a
}

const fn add(a: Field, b: Field) -> Field {
    // Both are below p < 2^255, so the sum fits in 256 bits.
    let mut out = [0; 4];
    let mut carry = 0u128;
    let mut i = 0;
    while i < 4 {
        let sum = a[i] as u128 + b[i] as u128 + carry;
        out[i] = sum as u64;
        carry = sum >> 64;
        i += 1;
    }
    reduce(out)
}

const fn sub(a: Field, b: Field) -> Field {
    if at_least(a, b) { sub_raw(a, b) } else { sub_raw(add_raw_p(a), b) }
}

/// `a + p`, which fits in 256 bits for `a < p`.
const fn add_raw_p(a: Field) -> Field {
    let mut out = [0; 4];
    let mut carry = 0u128;
    let mut i = 0;
    while i < 4 {
        let sum = a[i] as u128 + P[i] as u128 + carry;
        out[i] = sum as u64;
        carry = sum >> 64;
        i += 1;
    }
    out
}

const fn mul(a: Field, b: Field) -> Field {
    let mut wide = [0u64; 8];
    let mut i = 0;
    while i < 4 {
        let mut carry = 0u128;
        let mut j = 0;
        while j < 4 {
            let product = wide[i + j] as u128 + a[i] as u128 * b[j] as u128 + carry;
            wide[i + j] = product as u64;
            carry = product >> 64;
            j += 1;
        }
        wide[i + 4] = carry as u64;
        i += 1;
    }
    // 2^256 = 2p + 38, so the high half folds in as `38 · high`.
    let mut folded = [0u64; 4];
    let mut carry = 0u128;
    let mut i = 0;
    while i < 4 {
        let sum = wide[i] as u128 + 38 * wide[i + 4] as u128 + carry;
        folded[i] = sum as u64;
        carry = sum >> 64;
        i += 1;
    }
    // At most one more fold: `carry · 38` is small and can overflow only once.
    let mut extra = carry * 38;
    let mut i = 0;
    while i < 4 && extra > 0 {
        let sum = folded[i] as u128 + extra;
        folded[i] = sum as u64;
        extra = sum >> 64;
        i += 1;
    }
    if extra > 0 {
        folded[0] += 38;
    }
    reduce(folded)
}

const fn pow(base: Field, exponent: Field) -> Field {
    let mut result = ONE;
    let mut bit = 256;
    while bit > 0 {
        bit -= 1;
        result = mul(result, result);
        if (exponent[bit / 64] >> (bit % 64)) & 1 == 1 {
            result = mul(result, base);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::{prelude::*, test_runner::RngSeed};

    #[test]
    fn d_satisfies_its_definition() {
        // d · 121666 = -121665 mod p
        let product = mul(D, [121_666, 0, 0, 0]);
        assert_eq!(add(product, [121_665, 0, 0, 0]), ZERO);
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 512,
            rng_seed: RngSeed::Fixed(0x5044_4153),
            ..ProptestConfig::default()
        })]

        #[test]
        fn the_curve_check_agrees_with_the_reference(bytes in any::<[u8; 32]>()) {
            prop_assert_eq!(is_on_curve(&bytes), solana_address::bytes_are_curve_point(bytes));
        }

        #[test]
        fn find_agrees_with_the_reference(
            seed in prop::collection::vec(any::<u8>(), 0..32),
            program in any::<[u8; 32]>(),
        ) {
            let program = Address::new_from_array(program);
            let seeds = [seed.as_slice()];
            let found = find(&seeds, &program);
            prop_assert_eq!(found, Address::find_program_address(&seeds, &program));
            prop_assert_eq!(find_program_address(&seeds, &program), Ok(found));
            prop_assert_eq!(find_bump(&found.0, &seeds, &program), Ok(found.1));
            prop_assert_eq!(verify(&found.0, &seeds, found.1, &program), Ok(()));
        }
    }

    #[test]
    fn a_wrong_bump_is_rejected() {
        let program = Address::new_from_array([3; 32]);
        let seeds = [b"vault".as_slice()];
        let (address, bump) = find(&seeds, &program);
        assert_eq!(
            verify(&address, &seeds, bump.wrapping_sub(1), &program),
            Err(ProgramError::InvalidSeeds)
        );
    }
}
