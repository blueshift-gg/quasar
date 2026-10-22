//! Changes a handler makes beyond writing account data, and what an accounts
//! struct does after the handler succeeds: `close` and settling rent.

use core::cmp::Ordering;

use crate::{
    ProgramError, Result,
    cpi::{Cpi, CpiAccount},
    input::Entry,
    syscalls,
    system::transfer_data,
    sysvar::RentSource,
    wrapper::{Program, ProgramId, SystemProgram, UncheckedAccount, Wrapper, flush},
};

impl UncheckedAccount {
    /// Changes the data length; new bytes are zero. The runtime lets a
    /// program grow an account it owns by up to 10 KiB per instruction.
    pub fn resize(&mut self, len: usize) -> Result {
        self.entry_mut().resize(len)
    }

    /// The raw data, to write. The runtime accepts the writes only if this
    /// program owns the account.
    pub fn data_mut(&mut self) -> &mut [u8] {
        self.entry_mut().data_mut()
    }

    /// Takes lamports out. The runtime accepts it only if this program owns
    /// the account.
    #[inline(always)]
    pub fn sub_lamports(&mut self, lamports: u64) -> Result {
        self.entry_mut().sub_lamports(lamports)
    }
}

/// After the handler, for a writable account whose writes resize it: its
/// data length follows its content, and with a payer (`realloc(payer = ..)`,
/// or `init`'s) it holds exactly the rent-exempt balance for that length, the
/// payer topping it up through the System Program or taking back the excess,
/// as Anchor's `realloc::payer` does.
pub struct Settle {
    account: Entry,
    flush: fn(&mut Entry),
    rent: Option<Payer>,
}

/// Who settles a resized account's rent, and the rent if the instruction
/// has its account.
struct Payer {
    payer: Entry,
    system: Program<SystemProgram>,
    rent: RentSource,
}

impl Settle {
    /// `None` for an account whose length never changes.
    pub fn new<W: Wrapper>(account: &W) -> Option<Self> {
        // SAFETY: the handle is used only by `run`, after the handler has
        // returned and every other handle to the account is gone.
        let account = unsafe { account.entry().alias() };
        W::RESIZES.then_some(Self { account, flush: flush::<W>, rent: None })
    }

    /// Settles the rent with `payer` too: `init`'s payer, for an account
    /// created at its capacity.
    pub fn paid<W: Wrapper, P: Wrapper>(
        account: &W,
        payer: &P,
        system: &Program<SystemProgram>,
        rent: &RentSource,
    ) -> Option<Self> {
        // SAFETY: as in `new`.
        let (payer, system, rent) =
            unsafe { (payer.entry().alias(), system.alias(), rent.alias()) };
        Some(Self { rent: Some(Payer { payer, system, rent }), ..Self::new(account)? })
    }

    /// `realloc(payer = ..)`, which only an account this program owns and
    /// whose size varies takes.
    pub fn realloc<W: Wrapper, P: Wrapper>(
        account: &W,
        payer: &P,
        system: &Program<SystemProgram>,
        rent: &RentSource,
    ) -> Option<Self> {
        const { resizable::<W>() };
        Self::paid(account, payer, system, rent)
    }

    pub fn run(mut self) -> Result {
        (self.flush)(&mut self.account);
        let Some(Payer { mut payer, system, rent }) = self.rent else { return Ok(()) };
        let mut account = self.account;
        let rent = rent.get()?.minimum_balance(account.data().len());
        let balance = account.lamports();
        match balance.cmp(&rent) {
            Ordering::Less => {
                let accounts = [
                    CpiAccount::unchecked(payer, true).signer(),
                    CpiAccount::unchecked(account, true),
                ];
                Cpi::new(&system, accounts, transfer_data(rent - balance)).invoke()
            }
            Ordering::Greater => {
                let excess = balance - rent;
                account.sub_lamports(excess)?;
                let refunded =
                    payer.lamports().checked_add(excess).ok_or(ProgramError::ArithmeticOverflow)?;
                payer.set_lamports(refunded);
                Ok(())
            }
            Ordering::Equal => Ok(()),
        }
    }
}

/// Fails to compile for `close` on an account this program does not own.
pub const fn closable<W: Wrapper>() {
    assert!(W::PROGRAM_OWNED, "only accounts this program owns can be closed");
}

/// Fails to compile for `realloc` on an account this program cannot resize.
pub const fn resizable<W: Wrapper>() {
    assert!(W::RESIZES, "`realloc` is for an account whose size varies");
    assert!(W::PROGRAM_OWNED, "only accounts this program owns are resized");
}

/// `close(to = ..)`: once the handler has succeeded, every lamport goes to
/// the destination, the data is emptied, and the System Program owns the
/// account again.
pub struct Close {
    account: Entry,
    destination: Entry,
}

impl Close {
    pub fn new<W: Wrapper, D: Wrapper>(account: &W, destination: &D) -> Self {
        const { closable::<W>() };
        // SAFETY: the handles are used only by `run`, after the handler has
        // returned and every other handle to these accounts is gone.
        let (account, destination) =
            unsafe { (account.entry().alias(), destination.entry().alias()) };
        Self { account, destination }
    }

    pub fn run(mut self) -> Result {
        let destination = self.destination.lamports().wrapping_add(self.account.lamports());
        self.destination.set_lamports(destination);
        self.account.set_lamports(0);
        self.account.clear();
        self.account.set_owner(SystemProgram::ID);
        Ok(())
    }
}

/// A value logged for off-chain readers. Implemented by `#[event]`.
pub trait Event {
    fn emit(&self);
}

/// Logs an event: `emit!(Deposited { amount })`.
#[macro_export]
macro_rules! emit {
    ($event:expr) => {
        $crate::effects::Event::emit(&$event)
    };
}

/// Logs `data` as one `Program data:` entry.
pub fn log_data(data: &[u8]) {
    syscalls::log_data(&[data]);
}

/// The most return data an instruction can set.
pub const MAX_RETURN_DATA: usize = 1024;

/// Sets the instruction's return data, readable by the caller.
pub fn set_return_data(data: &[u8]) {
    syscalls::set_return_data(data);
}
