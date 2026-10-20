//! Sysvars, read in place from their account (`Sysvar<Clock>`) or copied
//! through a syscall (`Clock::get()`).

use core::{cell::Cell, marker::PhantomData, ops::Deref};

use zeropod::{Layout, ZeroPod};

use crate::{
    Address, Result,
    error::Error,
    input::{Checked, Entry, Privileges},
    syscalls,
    wrapper::{Wrapper, entry_accessors},
};

/// A sysvar's data, laid out as the runtime serializes it: little-endian
/// fields, as zeropod stores them.
pub trait SysvarData: Layout {
    const ID: Address;

    /// Copies the sysvar through `sol_get_sysvar`, without its account.
    fn get() -> Result<Self> {
        let size = Self::SIZE.expect("a sysvar has one size");
        let mut bytes = [0; 64];
        let bytes = &mut bytes[..size];
        syscalls::sysvar(&Self::ID, bytes)?;
        // SAFETY: the runtime wrote the sysvar, which is always valid.
        Ok(Self::own(unsafe { Self::read(bytes) }))
    }

    /// Whether this is `Rent`, whose account `init` and settling read
    /// instead of the syscall.
    const IS_RENT: bool = false;
}

/// `solana_clock::Clock`.
#[derive(ZeroPod, Clone, Copy, Debug, PartialEq, Eq)]
#[zeropod(crate = zeropod)]
pub struct Clock {
    pub slot: u64,
    pub epoch_start_timestamp: i64,
    pub epoch: u64,
    pub leader_schedule_epoch: u64,
    pub unix_timestamp: i64,
}

impl SysvarData for Clock {
    const ID: Address = Address::from_str_const("SysvarC1ock11111111111111111111111111111111");
}

/// `solana_rent::Rent`.
#[derive(ZeroPod, Clone, Copy, Debug, PartialEq)]
#[zeropod(crate = zeropod)]
pub struct Rent {
    pub lamports_per_byte_year: u64,
    /// The `f64` threshold's bits: zeropod stores no floats.
    pub exemption_threshold: u64,
    pub burn_percent: u8,
}

impl SysvarData for Rent {
    const ID: Address = Address::from_str_const("SysvarRent111111111111111111111111111111111");

    const IS_RENT: bool = true;
}

impl Rent {
    /// The lamports an account of `len` bytes needs to be rent exempt:
    /// `(128 + len) · lamports_per_byte_year · exemption_threshold`.
    pub fn minimum_balance(&self, len: usize) -> u64 {
        const TWO: u64 = 2f64.to_bits();
        const ONE: u64 = 1f64.to_bits();
        let lamports = (128 + len as u64) * self.lamports_per_byte_year;
        // SBF has no floating point, so the thresholds the runtime uses, 2 and
        // after SIMD-0194 1, are matched by their bits; only others multiply.
        match self.exemption_threshold {
            TWO => lamports * 2,
            ONE => lamports,
            bits => scale(lamports, f64::from_bits(bits)),
        }
    }
}

#[cold]
fn scale(lamports: u64, threshold: f64) -> u64 {
    (lamports as f64 * threshold) as u64
}

/// Where `init` and settling read the rent: the instruction's `Sysvar<Rent>`
/// account if it has one, or else the syscall; once, the first time an
/// account is created or resized.
pub struct RentSource {
    account: Option<Entry>,
    read: Cell<Option<Rent>>,
}

impl RentSource {
    /// From each field's `Field::rent_account`.
    #[inline(always)]
    pub fn new(account: Option<&Entry>) -> Self {
        // SAFETY: the sysvar's account is read-only, and only read.
        let account = account.map(|account| unsafe { account.alias() });
        Self { account, read: Cell::new(None) }
    }

    /// A second handle to the same account, for after the handler.
    ///
    /// # Safety
    /// As for `Entry::alias`.
    pub(crate) unsafe fn alias(&self) -> Self {
        // SAFETY: forwarded.
        let account = self.account.as_ref().map(|account| unsafe { account.alias() });
        Self { account, read: Cell::new(self.read.get()) }
    }

    /// Read once, from the account or the syscall.
    pub fn get(&self) -> Result<Rent> {
        if let Some(rent) = self.read.get() {
            return Ok(rent);
        }
        let rent = match &self.account {
            // SAFETY: `Sysvar<Rent>` checked the account's address, and the
            // runtime keeps the sysvar there valid.
            Some(account) => Rent::own(unsafe { Rent::view_unchecked(account.data()) }),
            None => Rent::get()?,
        };
        self.read.set(Some(rent));
        Ok(rent)
    }
}

/// The sysvar account of `S`, read in place.
pub struct Sysvar<S>(Entry, PhantomData<S>);

impl<S: SysvarData> Wrapper for Sysvar<S> {
    const REQUIRES: Privileges = Privileges::NONE;
    const LEN: Option<usize> = S::SIZE;

    fn from_entry(entry: Entry, _: Checked) -> Result<Self> {
        // The account at the sysvar's address holds it, and the runtime keeps
        // it valid; the cursor checked its length.
        if *entry.key() == S::ID { Ok(Self(entry, PhantomData)) } else { Err(Error::Sysvar.into()) }
    }

    fn why(entry: &Entry, name: &str) -> &'static str {
        crate::error::head("Sysvar", name)
            .text("is ")
            .key(entry.key())
            .text(", expected the sysvar ")
            .key(&S::ID)
            .log();
        "its field takes a sysvar account: pass that sysvar's address"
    }

    fn rent_account(&self) -> Option<&Entry> {
        S::IS_RENT.then_some(&self.0)
    }

    entry_accessors!();
}

impl<S: SysvarData> Deref for Sysvar<S> {
    type Target = S::View;

    fn deref(&self) -> &S::View {
        // SAFETY: the account at the sysvar's address holds a valid `S`, which
        // the runtime keeps, `S::SIZE` long.
        unsafe { S::view_unchecked(self.0.data()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(deprecated)] // the fields `Rent` still has at the pinned version
    fn rent_matches_the_runtime_for_any_threshold() {
        for exemption_threshold in [2.0, 1.0, 1.5, 0.5] {
            let reference = solana_rent::Rent {
                lamports_per_byte_year: 3480,
                exemption_threshold,
                burn_percent: 50,
            };
            let rent = Rent {
                lamports_per_byte_year: 3480,
                exemption_threshold: f64::to_bits(exemption_threshold),
                burn_percent: 50,
            };
            for len in [0, 41, 10_240] {
                assert_eq!(rent.minimum_balance(len), reference.minimum_balance(len));
            }
        }
    }
}
