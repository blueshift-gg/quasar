//! How each field of an accounts struct loads: a wrapper from the next
//! entry, `Option<W>` from an entry a client may mark absent, `Remaining<W>`
//! from every entry left. The derive calls this trait for every field, so it
//! never needs to know which kind of field it is looking at.

use crate::{
    Result,
    cpi::{Check, SIGNER_CHECK},
    effects::Settle,
    input::{Cursor, Entry, Privileges},
    remaining::Remaining,
    wrapper::Wrapper,
};

/// What the derive, a client and a Kani world need to know about a field.
#[derive(Clone, Copy)]
pub struct FieldInfo {
    /// The data length every valid account has, if it is fixed.
    pub len: Option<usize>,
    /// The discriminator width of the kind it holds, if this program owns it.
    pub width: Option<usize>,
    /// Whether it takes every entry left, so it comes last.
    pub rest: bool,
    /// Whether it is a `Pda`, which loads only through `seeds(..)`.
    pub pda: bool,
    /// Whether the account signs.
    pub signer: bool,
}

#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a field of an accounts struct",
    label = "fields are wrappers such as `Signer` or `Account<T>`, or `Option<W>` or `Remaining<W>` of one"
)]
pub trait Field: Sized {
    const INFO: FieldInfo;

    /// Loads the field, writable if declared `mut`.
    fn load(cursor: &mut Cursor, writable: bool) -> Result<Self>;

    /// The name of the check its load makes, if a call can make it instead.
    const CHECK: Option<Check> = None;
    /// The fields of the kind it holds (`Wrapper::FIELDS`).
    const FIELDS: &'static [&'static str] = &[];
    /// Whether the account signs, which `SIGNER_CHECK` names.
    const SIGNS: Option<Check> = None;

    /// `load`, or if `waived` without `CHECK`.
    #[inline(always)]
    fn load_vouched(
        cursor: &mut Cursor,
        writable: bool,
        _waived: bool,
        _at: &crate::error::At,
    ) -> Result<Self> {
        Self::load(cursor, writable)
    }

    /// What brings a writable account's length to its content's after the
    /// handler; `None` for one whose length never changes.
    fn settle(&self) -> Option<Settle> {
        None
    }

    /// The account holding the rent, `Sysvar<Rent>`'s, for `init` and
    /// `realloc`.
    fn rent_account(&self) -> Option<&Entry> {
        None
    }

    /// How a client names the account: an address, `None`, or several.
    #[cfg(not(target_os = "solana"))]
    type Key;

    #[cfg(not(target_os = "solana"))]
    fn metas(key: Self::Key, writable: bool, program: crate::Address, metas: &mut Metas);

    #[cfg(not(target_os = "solana"))]
    fn idl(name: &'static str, writable: bool) -> crate::client::IdlAccount;

    /// The kind it holds, for the IDL.
    #[cfg(not(target_os = "solana"))]
    fn idl_kind() -> Option<crate::client::IdlKind> {
        None
    }
}

#[cfg(not(target_os = "solana"))]
type Metas = crate::client::Vec<crate::client::AccountMeta>;

impl<W: Wrapper> Field for W {
    const INFO: FieldInfo = FieldInfo {
        len: W::LEN,
        width: W::WIDTH,
        rest: false,
        pda: W::PDA,
        signer: W::REQUIRES.includes(Privileges::SIGNER),
    };

    #[inline(always)]
    fn load(cursor: &mut Cursor, writable: bool) -> Result<Self> {
        cursor.load(writable)
    }

    const CHECK: Option<Check> = W::CHECK;
    const FIELDS: &'static [&'static str] = W::FIELDS;
    const SIGNS: Option<Check> =
        if W::REQUIRES.includes(Privileges::SIGNER) { Some(SIGNER_CHECK) } else { None };

    #[inline(always)]
    fn load_vouched(
        cursor: &mut Cursor,
        writable: bool,
        waived: bool,
        at: &crate::error::At,
    ) -> Result<Self> {
        cursor.load_vouched(writable, waived, at)
    }

    fn settle(&self) -> Option<Settle> {
        Settle::new(self)
    }

    fn rent_account(&self) -> Option<&Entry> {
        Wrapper::rent_account(self)
    }

    #[cfg(not(target_os = "solana"))]
    type Key = crate::Address;

    #[cfg(not(target_os = "solana"))]
    fn metas(key: crate::Address, writable: bool, _: crate::Address, metas: &mut Metas) {
        metas.push(crate::client::meta::<W>(key, writable));
    }

    #[cfg(not(target_os = "solana"))]
    fn idl(name: &'static str, writable: bool) -> crate::client::IdlAccount {
        crate::client::idl_account::<W>(name, writable, false, false)
    }

    #[cfg(not(target_os = "solana"))]
    fn idl_kind() -> Option<crate::client::IdlKind> {
        W::idl_kind()
    }
}

impl<W: Wrapper> Field for Option<W> {
    const INFO: FieldInfo = FieldInfo { width: None, ..W::INFO };

    #[inline(always)]
    fn load(cursor: &mut Cursor, writable: bool) -> Result<Self> {
        const { assert!(!W::PDA, "a `Pda` loads only through `seeds(..)`") };
        cursor.load_optional(writable)
    }

    #[cfg(not(target_os = "solana"))]
    type Key = Option<crate::Address>;

    #[cfg(not(target_os = "solana"))]
    fn metas(
        key: Option<crate::Address>,
        writable: bool,
        program: crate::Address,
        metas: &mut Metas,
    ) {
        metas.push(crate::client::optional_meta::<W>(key, writable, program));
    }

    #[cfg(not(target_os = "solana"))]
    fn idl(name: &'static str, writable: bool) -> crate::client::IdlAccount {
        crate::client::idl_account::<W>(name, writable, true, false)
    }
}

impl<W: Wrapper> Field for Remaining<W> {
    const INFO: FieldInfo = FieldInfo { len: None, width: None, rest: true, ..W::INFO };

    #[inline(always)]
    fn load(cursor: &mut Cursor, writable: bool) -> Result<Self> {
        const { assert!(!W::PDA, "a `Pda` loads only through `seeds(..)`") };
        Ok(Remaining::new(cursor, writable))
    }

    #[cfg(not(target_os = "solana"))]
    type Key = crate::client::Vec<crate::Address>;

    #[cfg(not(target_os = "solana"))]
    fn metas(keys: Self::Key, writable: bool, _: crate::Address, metas: &mut Metas) {
        metas.extend(keys.into_iter().map(|key| crate::client::meta::<W>(key, writable)));
    }

    #[cfg(not(target_os = "solana"))]
    fn idl(name: &'static str, writable: bool) -> crate::client::IdlAccount {
        crate::client::idl_account::<W>(name, writable, false, true)
    }
}

/// The accounts the fields take before any `Remaining`, which all must be
/// there; fails to compile unless only the last field takes the rest.
pub const fn required(fields: &[FieldInfo]) -> u64 {
    let mut index = 0;
    while index < fields.len() {
        if fields[index].rest {
            assert!(index + 1 == fields.len(), "`Remaining` is the last field");
            return index as u64;
        }
        index += 1;
    }
    fields.len() as u64
}

/// Fails to compile for a `Pda` loaded without `seeds(..)`.
pub const fn not_pda<F: Field>() {
    assert!(!F::INFO.pda, "a `Pda` field needs `seeds(..)`");
}

/// Fails to compile for a `Remaining` with constraints beyond `mut`.
pub const fn not_remaining<F: Field>() {
    assert!(!F::INFO.rest, "`Remaining` takes only `mut`");
}
