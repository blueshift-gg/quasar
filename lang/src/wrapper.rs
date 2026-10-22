//! Wrappers are the only way to get a typed account. The cursor checks the
//! privileges a wrapper requires, and its fixed length, before building it.

use core::marker::PhantomData;

use crate::{
    Address, ProgramError, Result,
    cpi::Check,
    input::{Checked, Entry, Privileges},
};

#[diagnostic::on_unimplemented(
    message = "`{Self}` is not an account wrapper",
    label = "fields of an accounts struct are wrappers such as `Signer` or `Account<T>`"
)]
pub trait Wrapper: Sized {
    const REQUIRES: Privileges;
    /// The data length every valid account has, if it is fixed.
    const LEN: Option<usize> = None;
    /// Whether the checks still hold after a call that takes the account
    /// writable.
    const AFTER_CPI: AfterCpi = AfterCpi::Holds;
    /// Whether every valid account is owned by a program other than System.
    /// Only a signature from the address assigns that owner: at a PDA of this
    /// program, this program's, which creates PDAs at the canonical bump. So
    /// checking such an account's PDA needs only the hash.
    const ASSIGNED: bool = false;
    /// Whether this program owns every valid account, so it alone can
    /// debit, resize or close one.
    const PROGRAM_OWNED: bool = false;
    /// The discriminator width of the kind it holds, if this program owns it.
    const WIDTH: Option<usize> = None;
    /// Whether writes can change the data's length, which then follows its
    /// content (`flush`).
    const RESIZES: bool = false;
    /// Whether it is a `Pda`, which loads only through `seeds(..)`.
    const PDA: bool = false;
    /// The fields of the kind it holds, whose getters and setters keep the
    /// account valid.
    const FIELDS: &'static [&'static str] = &[];
    /// The name of the check `from_entry` makes, if a call can make it instead.
    const CHECK: Option<Check> = None;

    /// Builds the wrapper from an entry that already has `REQUIRES` and, if
    /// set, `LEN`, checking whatever else the wrapper promises. Only the
    /// cursor and a recheck after a call can call it: they alone make a `Checked`.
    fn from_entry(entry: Entry, checked: Checked) -> Result<Self>;

    /// `from_entry`, or if `waived` without `CHECK`, which a call the handler
    /// always makes makes instead (`cpi::Vouches`).
    #[inline(always)]
    fn vouched(entry: Entry, checked: Checked, _waived: bool) -> Result<Self> {
        Self::from_entry(entry, checked)
    }

    /// The length of the content in `entry`'s data: all of it, unless the
    /// wrapper `RESIZES`.
    fn content_len(entry: &Entry) -> usize {
        entry.data().len()
    }

    /// With the `debug` feature, logs why `from_entry` rejects `entry`, the
    /// account `name`: what was found and what was expected (`error::head`);
    /// how to fix it. Called where a rejection is a failure; a check that
    /// only asks whether an account is already there (`init(idempotent)`)
    /// stays silent.
    fn why(_entry: &Entry, name: &str) -> &'static str {
        crate::error::head("InvalidAccountData", name).text("is not what its field declares").log();
        ""
    }

    /// Checks the account again after a call that may have changed it, as it
    /// was checked when it loaded. Until then the wrapper is unusable: any
    /// read through it ends the program (`Entry::poison`).
    fn reload(&mut self) -> Result {
        let entry = self.entry_mut();
        entry.restore();
        // SAFETY: the wrapper built is dropped at once.
        let checked = crate::input::finish::<Self>(unsafe { entry.alias() }).map(drop);
        if checked.is_err() {
            if cfg!(feature = "debug") {
                let help = crate::input::why::<Self>(entry, "");
                crate::error::located(&crate::error::At::UNNAMED, help);
            }
            entry.poison();
        }
        checked
    }

    /// Brings the account's data length to its content's, before a call or
    /// the end of the instruction reads it.
    fn flush(&mut self) {
        flush::<Self>(self.entry_mut());
    }

    fn entry(&self) -> &Entry;

    fn entry_mut(&mut self) -> &mut Entry;

    fn into_entry(self) -> Entry;

    fn address(&self) -> &Address {
        self.entry().key()
    }

    fn owner(&self) -> &Address {
        self.entry().owner()
    }

    fn lamports(&self) -> u64 {
        self.entry().lamports()
    }

    fn data(&self) -> &[u8] {
        self.entry().data()
    }

    /// The account holding the rent: `Sysvar<Rent>`'s.
    fn rent_account(&self) -> Option<&Entry> {
        None
    }

    /// The kind it holds, for the IDL.
    #[cfg(not(target_os = "solana"))]
    fn idl_kind() -> Option<crate::client::IdlKind> {
        None
    }

    /// Adds lamports. Any writable account can receive them.
    fn add_lamports(&mut self, lamports: u64) -> Result {
        let balance =
            self.lamports().checked_add(lamports).ok_or(ProgramError::ArithmeticOverflow)?;
        self.entry_mut().set_lamports(balance);
        Ok(())
    }
}

/// Sets the data length of `entry`, which `W` wraps, to its content's.
pub(crate) fn flush<W: Wrapper>(entry: &mut Entry) {
    if W::RESIZES {
        let len = W::content_len(entry);
        // The content ends within the room it was written in, which
        // `set_len` allows.
        let _ = entry.set_len(len);
    }
}

/// What a call can do to a wrapper's checks.
pub enum AfterCpi {
    Holds,
    /// Only the program that owns the account can change it: calling itself,
    /// the one reentrant call the runtime allows.
    BrokenByCalling(&'static Address),
    /// The callee may change the account's owner or data.
    Broken,
}

/// `entry`, `entry_mut` and `into_entry` for a wrapper whose entry is its
/// first field.
macro_rules! entry_accessors {
    () => {
        fn entry(&self) -> &Entry {
            &self.0
        }

        fn entry_mut(&mut self) -> &mut Entry {
            &mut self.0
        }

        fn into_entry(self) -> Entry {
            self.0
        }
    };
}
pub(crate) use entry_accessors;

/// Implements `Wrapper` for a wrapper that is just its entry, with no check
/// beyond the privileges it requires.
macro_rules! entry_wrapper {
    ($name:ident, $requires:expr) => {
        impl Wrapper for $name {
            const REQUIRES: Privileges = $requires;

            fn from_entry(entry: Entry, _: Checked) -> Result<Self> {
                Ok(Self(entry))
            }

            entry_accessors!();
        }
    };
}

/// An account that signed the transaction.
pub struct Signer(Entry);
entry_wrapper!(Signer, Privileges::SIGNER);

/// Any account, with no checks beyond what the accounts struct adds.
pub struct UncheckedAccount(Entry);
entry_wrapper!(UncheckedAccount, Privileges::NONE);

/// An account the System Program owns, such as a wallet.
pub struct SystemAccount(Entry);

impl Wrapper for SystemAccount {
    const REQUIRES: Privileges = Privileges::NONE;
    /// A call it signs can assign it to another program.
    const AFTER_CPI: AfterCpi = AfterCpi::Broken;

    fn from_entry(entry: Entry, _: Checked) -> Result<Self> {
        if *entry.owner() == SystemProgram::ID {
            Ok(Self(entry))
        } else {
            Err(ProgramError::IllegalOwner)
        }
    }

    fn why(entry: &Entry, name: &str) -> &'static str {
        crate::error::head("IllegalOwner", name)
            .text("is owned by ")
            .key(entry.owner())
            .text(", expected the System Program ")
            .key(&SystemProgram::ID)
            .log();
        "its field takes a wallet, or an account not created yet; this one belongs to a program: check the address passed"
    }

    entry_accessors!();
}

/// A program's address, for `Program<P>`.
pub trait ProgramId {
    const ID: Address;
}

/// The programs a call may go to: one for `Program<P>`, several for
/// `Interface<I>`, such as Token and Token-2022.
pub trait ProgramIds {
    const IDS: &'static [Address];
}

impl<P: ProgramId> ProgramIds for P {
    const IDS: &'static [Address] = &[P::ID];
}

/// An executable account a call can go to.
pub trait Callee: Wrapper {
    type Ids: ProgramIds;
}

/// The System Program.
pub struct SystemProgram;

impl ProgramId for SystemProgram {
    const ID: Address = Address::new_from_array([0; 32]);
}

/// The program `P`, to call. Its account is not compared with `P::ID`: a call
/// names its program by `address`, which is `P::ID`, and the runtime fails one
/// whose program is not among the instruction's accounts.
pub struct Program<P>(Entry, PhantomData<P>);

impl<P> Program<P> {
    /// A second handle to the program's account.
    ///
    /// # Safety
    /// As for `Entry::alias`.
    pub(crate) unsafe fn alias(&self) -> Self {
        // SAFETY: forwarded from this function's contract.
        Self(unsafe { self.0.alias() }, PhantomData)
    }
}

impl<P: ProgramId> Callee for Program<P> {
    type Ids = P;
}

/// The executable account at any one of `I::IDS`.
pub struct Interface<I>(Entry, PhantomData<I>);

impl<I: ProgramIds> Callee for Interface<I> {
    type Ids = I;
}

impl<I: ProgramIds> Wrapper for Interface<I> {
    const REQUIRES: Privileges = Privileges::EXECUTABLE;

    #[inline(always)]
    fn from_entry(entry: Entry, _: Checked) -> Result<Self> {
        if I::IDS.contains(entry.key()) {
            Ok(Self(entry, PhantomData))
        } else {
            Err(ProgramError::IncorrectProgramId)
        }
    }

    fn why(entry: &Entry, name: &str) -> &'static str {
        crate::error::head("IncorrectProgramId", name)
            .text("is ")
            .key(entry.key())
            .text(", expected the program ")
            .keys(I::IDS)
            .log();
        "its field takes one of these programs: pass that program's address"
    }

    entry_accessors!();
}

impl<P: ProgramId> Wrapper for Program<P> {
    const REQUIRES: Privileges = Privileges::EXECUTABLE;

    fn from_entry(entry: Entry, _: Checked) -> Result<Self> {
        Ok(Self(entry, PhantomData))
    }

    fn address(&self) -> &Address {
        &P::ID
    }

    entry_accessors!();
}
