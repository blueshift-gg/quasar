//! Calls into other programs through `sol_invoke_signed_c`, with account infos
//! pointing into the input. Afterwards, the writable accounts whose checks the
//! callee could break are checked again, chosen at compile time
//! (`Wrapper::AFTER_CPI`).

use core::{marker::PhantomData, mem::MaybeUninit, ptr};

#[cfg(test)]
pub(crate) mod miri;

use crate::{
    Address, Result,
    field::Field,
    input::{self, Entry},
    pda::SignerSeeds,
    syscalls::{self, RawSlice},
    wrapper::{AfterCpi, Callee, ProgramIds, Wrapper},
};

/// A check, by name: `SIGNER_CHECK`, a library's `MINT_CHECK`. An account's
/// load or constraint makes it, and so may a call the account is passed to;
/// a failed call fails the transaction, so when the handler always makes a
/// call that makes a check, the account's own check of that name is left out.
pub type Check = &'static str;

/// The account signed. The runtime makes this check of every call, and the
/// load makes it in the compare that rejects duplicates, at no cost.
pub const SIGNER_CHECK: Check = "signer";
/// The account is writable; as `SIGNER_CHECK`.
pub const MUTABLE_CHECK: Check = "mutable";

/// A check a call makes of one of its arguments: about another argument,
/// `of`, if it relates the two, as an account's mint.
#[derive(Clone, Copy)]
pub struct Enforced {
    pub check: Check,
    pub of: Option<usize>,
}

impl Enforced {
    pub const fn just(check: Check) -> Self {
        Self { check, of: None }
    }

    pub const fn of(check: Check, argument: usize) -> Self {
        Self { check, of: Some(argument) }
    }
}

/// What a call does with an account it is passed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum After {
    /// Anything its owner may: its wrapper is unusable until `reload()` has
    /// checked it again.
    Unknown,
    /// Neither its owner nor its data's length changes, and its data stays
    /// valid for what it held: its wrapper reads on.
    Kept,
    /// It is closed: nothing reads it again.
    Closed,
}

/// What a call does with one argument: the checks it makes of it before it
/// can succeed, and what it leaves of it after.
#[derive(Clone, Copy)]
pub struct Argument {
    pub checks: &'static [Enforced],
    pub after: After,
}

impl Argument {
    /// An argument that is not an account, or one nothing is known of.
    pub const NONE: Self = Self { checks: &[], after: After::Unknown };

    pub const fn checked(checks: &'static [Enforced]) -> Self {
        Self { checks, after: After::Unknown }
    }

    pub const fn closed(checks: &'static [Enforced]) -> Self {
        Self { checks, after: After::Closed }
    }

    /// # Safety
    /// The call changes neither the account's owner nor its data's length,
    /// and leaves its data valid for what it held.
    pub const unsafe fn kept(checks: &'static [Enforced]) -> Self {
        Self { checks, after: After::Kept }
    }
}

/// What a function that builds a call does with each argument: the const
/// `ENFORCES` of the module named like the function, or of the accounts
/// struct for its `cpi_accounts`. A function without one says nothing.
pub type Enforces = &'static [Argument];

/// The name of the check a kind's wrapper makes as it loads.
#[derive(Clone, Copy)]
pub struct Named(Check);

impl Named {
    /// # Safety
    /// The kind's view reads any bytes at least as long as the kind without
    /// undefined behavior: a handler may read an account before the call
    /// that checks it.
    pub const unsafe fn new(check: Check) -> Self {
        Self(check)
    }

    /// `check` for a kind of one size that any bytes hold; `None` for others.
    pub const fn plain<K: zeropod::ZeroPod>(check: Check) -> Option<Self> {
        if K::ANY_BYTES && K::SIZE.is_some() { Some(Self(check)) } else { None }
    }

    pub const fn check(self) -> Check {
        self.0
    }
}

/// A call a handler always makes with a field as its argument `argument`.
#[derive(Clone, Copy)]
pub struct Call {
    pub enforces: Enforces,
    /// The field each argument is, or `""`.
    pub arguments: &'static [&'static str],
    pub argument: usize,
}

/// The calls a handler always makes, by the field they take: one per path
/// through the handler. Implemented by `#[instruction(n)]`, which reads it.
pub trait Vouches {
    const VOUCHES: &'static [(&'static str, &'static [Call])];
}

/// `ENFORCES` for a type without one of its own.
pub trait Unvouched {
    const ENFORCES: Enforces = &[];
}

impl<T> Unvouched for T {}

/// `KEYS` for a library constraint whose keys no call checks.
pub trait Unkeyed {
    const KEYS: &'static [(&'static str, Check)] = &[];
}

impl<T> Unkeyed for T {}

/// The check a library constraint's key makes, if a call can make it.
pub const fn key(keys: &[(&str, Check)], key: &str) -> Option<Check> {
    let mut index = 0;
    while index < keys.len() {
        if same(keys[index].0, key) {
            return Some(keys[index].1);
        }
        index += 1;
    }
    None
}

/// No call is made: every check runs.
pub struct Nothing;

impl Vouches for Nothing {
    const VOUCHES: &'static [(&'static str, &'static [Call])] = &[];
}

/// Whether the handler's calls make `check` of `field`, on every path. A
/// check that relates two accounts is about the field `of`, and counts only
/// from a call that also makes `kind`, the check of what `field` holds: the
/// same name on another kind is another check.
///
/// Built with the `debug` feature, nothing is: the program makes every check
/// itself, to say which one fails, and for tests to compare the two builds.
pub const fn vouched(
    vouches: &[(&str, &[Call])],
    field: &str,
    check: Option<Check>,
    of: &str,
    kind: Option<Check>,
) -> bool {
    if cfg!(feature = "debug") {
        return false;
    }
    let Some(check) = check else { return false };
    let related = !of.is_empty();
    if related && kind.is_none() {
        return false;
    }
    let mut index = 0;
    while index < vouches.len() {
        let (name, calls) = vouches[index];
        if same(name, field) {
            let mut path = 0;
            while path < calls.len() {
                if !makes(calls[path], check, of) {
                    return false;
                }
                if let (true, Some(kind)) = (related, kind)
                    && !makes(calls[path], kind, "")
                {
                    return false;
                }
                path += 1;
            }
            return !calls.is_empty();
        }
        index += 1;
    }
    false
}

/// Whether `call` makes `check` of its argument, about the field `of`.
const fn makes(call: Call, check: Check, of: &str) -> bool {
    if call.argument >= call.enforces.len() {
        return false;
    }
    let enforced = call.enforces[call.argument].checks;
    let mut index = 0;
    while index < enforced.len() {
        let about = match enforced[index].of {
            Some(argument) if argument < call.arguments.len() => call.arguments[argument],
            Some(_) => return false,
            None => "",
        };
        if same(enforced[index].check, check) && same(about, of) {
            return true;
        }
        index += 1;
    }
    false
}

/// Whether `made`, the check an `init` in the accounts struct makes of a
/// field, is the one `F` makes as it loads. `init` runs after every load and
/// before the handler, on every run.
pub const fn made_by_init<F: Field>(made: Option<Check>) -> bool {
    match (F::CHECK, made) {
        (Some(check), Some(made)) => !cfg!(feature = "debug") && same(check, made),
        _ => false,
    }
}

/// Whether the handler's calls make the check `F` makes as it loads.
pub const fn waived_field<F: Field>(vouches: &[(&str, &[Call])], field: &str) -> bool {
    vouched(vouches, field, F::CHECK, "", None)
}

/// `check` about argument `of`, as a call's `ENFORCES` lists it; a check no
/// call can make, `None`, is listed as one that matches nothing.
pub const fn enforced(check: Option<Check>, of: Option<usize>) -> Enforced {
    match check {
        Some(check) => Enforced { check, of },
        None => Enforced { check: "", of: None },
    }
}

/// The name of `has_one(field)`: the account stores the address of the
/// account passed as `field`.
#[macro_export]
macro_rules! has_one_check {
    ($field:ident) => {
        concat!("has_one.", stringify!($field))
    };
}

const fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut index = 0;
    while index < a.len() {
        if a[index] != b[index] {
            return false;
        }
        index += 1;
    }
    true
}

/// Whether every one of `ids` is one of `accepted`: a call to a program
/// whose checks `ENFORCES` names.
pub const fn within(ids: &[Address], accepted: &[Address]) -> bool {
    let mut index = 0;
    while index < ids.len() {
        let mut found = false;
        let mut other = 0;
        while other < accepted.len() {
            let (a, b) = (ids[index].as_array(), accepted[other].as_array());
            let mut byte = 0;
            let mut equal = true;
            while byte < 32 {
                equal &= a[byte] == b[byte];
                byte += 1;
            }
            found |= equal;
            other += 1;
        }
        if !found {
            return false;
        }
        index += 1;
    }
    true
}

/// An account's recheck: its wrapper's checks, run again after a call to one
/// of the programs given. Only for an account whose length follows its
/// content, which what runs after the handler reads; any other is left to
/// `reload()`.
type Recheck = fn(&Entry, &[Address]) -> Result;

/// One account of a call, as the runtime's `SolAccountMeta` and
/// `SolAccountInfo` need it.
pub struct CpiAccount<'a> {
    entry: Entry,
    /// The wrapper the entry came from stays borrowed for the call.
    borrow: PhantomData<&'a ()>,
    is_writable: bool,
    is_signer: bool,
    recheck: Option<Recheck>,
}

impl<'a> CpiAccount<'a> {
    pub fn readonly<W: Wrapper>(account: &'a W) -> Self {
        Self::new(account, false)
    }

    /// A read-only account known only by its entry, as a constraint holds it.
    pub fn readonly_entry(entry: &'a Entry) -> Self {
        // SAFETY: `entry` stays borrowed for `'a`, the life of the call.
        Self::unchecked(unsafe { entry.alias() }, false)
    }

    /// The callee may change the account, so nothing may read it during the call.
    pub fn writable<W: Wrapper>(account: &'a mut W) -> Self {
        Self::passed(account, false, None)
    }

    /// As `writable`, for a call to `program`: an account `program` cannot
    /// change stays usable.
    pub fn writable_to<C: Callee, W: Wrapper>(_program: &C, account: &'a mut W) -> Self {
        Self::passed(account, false, Some(C::Ids::IDS))
    }

    /// A writable account as argument `index` of a call that `enforces`
    /// describes: usable afterwards if the call keeps it.
    #[inline(always)]
    pub fn argument<W: Wrapper>(enforces: Enforces, index: usize, account: &'a mut W) -> Self {
        Self::passed(account, matches!(enforces[index].after, After::Kept), None)
    }

    /// A writable account the call keeps what it is.
    pub(crate) fn kept<W: Wrapper>(account: &'a mut W) -> Self {
        Self::passed(account, true, None)
    }

    /// A writable account. Unless the call keeps it, or is to none of the
    /// programs that could change it (`callees`, if known), its wrapper is
    /// unusable afterwards until `reload()`: decided here, when compiling.
    #[inline(always)]
    fn passed<W: Wrapper>(account: &'a mut W, kept: bool, callees: Option<&[Address]>) -> Self {
        account.flush();
        // SAFETY: `account` stays borrowed for `'a`, the life of the call, so
        // nothing else uses its entry meanwhile.
        let entry = unsafe { account.entry().alias() };
        let breakable = !kept
            && match (W::AFTER_CPI, callees) {
                (AfterCpi::Holds, _) => false,
                (AfterCpi::BrokenByCalling(owner), Some(callees)) => callees.contains(owner),
                (AfterCpi::BrokenByCalling(_) | AfterCpi::Broken, _) => true,
            };
        // What runs after the handler reads a variable kind through a handle
        // of its own, so that one is checked at once, as before.
        let recheck = (breakable && W::RESIZES).then_some(recheck::<W> as Recheck);
        if breakable && !W::RESIZES {
            account.entry_mut().poison();
        }
        Self { recheck, ..Self::unchecked(entry, true) }
    }

    /// A writable payer that the caller still reads while the call runs, as
    /// when a PDA's seeds include the payer's address.
    ///
    /// # Safety
    /// The call only changes the payer's lamports, which are read by value and
    /// never borrowed; everything a reference can point to, such as the key a
    /// seed borrows, stays unchanged, and so do the payer's checks.
    pub unsafe fn paying<W: Wrapper>(payer: &'a W) -> Self {
        Self::new(payer, true).signer()
    }

    /// An account the call closes. It is taken by value, so nothing reads it
    /// afterwards, and it is not reloaded.
    pub fn closing<W: Wrapper>(account: W) -> CpiAccount<'static> {
        CpiAccount::unchecked(account.into_entry(), true)
    }

    /// An account that is not reloaded after the call.
    pub(crate) fn unchecked(entry: Entry, is_writable: bool) -> Self {
        let (borrow, is_signer) = (PhantomData, false);
        Self { entry, borrow, is_writable, is_signer, recheck: None }
    }

    fn new<W: Wrapper>(account: &'a W, is_writable: bool) -> Self {
        // SAFETY: `account` stays borrowed for `'a`, the life of the call, so
        // nothing else uses its entry meanwhile.
        Self::unchecked(unsafe { account.entry().alias() }, is_writable)
    }

    /// `account` as a `mut` field `F` of a callee's accounts struct:
    /// signing if `F` signs.
    pub fn writable_field<F: Field, W: Wrapper>(account: &'a mut W, callee: &Address) -> Self {
        Self::field::<F>(Self::passed(account, false, Some(core::slice::from_ref(callee))))
    }

    /// `account` as a field `F` of a callee's accounts struct.
    pub fn readonly_field<F: Field, W: Wrapper>(account: &'a W) -> Self {
        Self::field::<F>(Self::readonly(account))
    }

    fn field<F: Field>(account: Self) -> Self {
        const { assert!(!F::INFO.rest, "a `Remaining` field takes its accounts one by one") };
        if F::INFO.signer { account.signer() } else { account }
    }

    /// Passes the account as a signer: it signed the transaction, or it is
    /// the PDA whose seeds sign the call.
    pub fn signer(self) -> Self {
        Self { is_signer: true, ..self }
    }
}

/// Whether a call to one of `callees` can break `W`'s checks: any call, or
/// only its owner calling itself.
fn broken_by<W: Wrapper>(callees: &[Address]) -> bool {
    match W::AFTER_CPI {
        AfterCpi::Holds => false,
        AfterCpi::BrokenByCalling(owner) => callees.contains(owner),
        AfterCpi::Broken => true,
    }
}

fn recheck<W: Wrapper>(entry: &Entry, callees: &[Address]) -> Result {
    if !broken_by::<W>(callees) {
        return Ok(());
    }
    // SAFETY: the wrapper built here is dropped before this returns, and the
    // caller's handle is not used meanwhile: it is borrowed by the call.
    input::finish::<W>(unsafe { entry.alias() }).map(drop)
}

/// A call to `P`, ready to invoke.
#[must_use = "a call does nothing until it is invoked"]
pub struct Cpi<'a, P, const N: usize, const D: usize> {
    program: &'a Address,
    callee: PhantomData<P>,
    accounts: [CpiAccount<'a>; N],
    data: [u8; D],
}

impl<'a, P: ProgramIds, const N: usize, const D: usize> Cpi<'a, P, N, D> {
    pub fn new<C: Callee<Ids = P>>(
        program: &'a C,
        accounts: [CpiAccount<'a>; N],
        data: [u8; D],
    ) -> Self {
        Self { program: program.address(), callee: PhantomData, accounts, data }
    }

    /// Sends only the initialized encoding's prefix, for variable-length instructions.
    pub fn with_data_len(self, len: usize) -> Result<CpiWithDataLen<'a, P, N, D>> {
        if len > D {
            return Err(crate::ProgramError::InvalidInstructionData);
        }
        Ok(CpiWithDataLen { call: self, len })
    }

    /// The instruction a call will submit, for host clients and wire-format tests.
    #[cfg(not(target_os = "solana"))]
    pub fn instruction(&self) -> solana_instruction::Instruction {
        solana_instruction::Instruction {
            program_id: *self.program,
            accounts: self
                .accounts
                .iter()
                .map(|account| solana_instruction::AccountMeta {
                    pubkey: *account.entry.key(),
                    is_signer: account.is_signer,
                    is_writable: account.is_writable,
                })
                .collect(),
            data: self.data.into(),
        }
    }

    #[inline(always)]
    pub fn invoke(self) -> Result {
        let rechecks = self.rechecks();
        // SAFETY: no signer seeds.
        unsafe { self.call(&[], D) };
        self.recheck_changed(rechecks)
    }

    /// Invokes with `signer` signing for its PDA.
    #[inline(always)]
    pub fn invoke_signed<const S: usize>(self, signer: &SignerSeeds<'_, S>) -> Result {
        let seeds = SignerSlices {
            seeds: signer.seeds.map(RawSlice::new),
            bump: RawSlice::new(&signer.bump),
        };
        let signers = [RawSlice { addr: ptr::from_ref(&seeds).cast(), len: S as u64 + 1 }];
        let rechecks = self.rechecks();
        // SAFETY: `SignerSlices` is `S + 1` contiguous `RawSlice`s.
        unsafe { self.call(&signers, D) };
        self.recheck_changed(rechecks)
    }

    /// Read before the call: the call's pointers escape, so values read after
    /// it are no longer known to the optimizer, and the checks would not fold away.
    #[inline(always)]
    fn rechecks(&self) -> [Option<Recheck>; N] {
        let mut rechecks = [None; N];
        for (recheck, account) in rechecks.iter_mut().zip(&self.accounts) {
            *recheck = account.recheck;
        }
        rechecks
    }

    fn recheck_changed(&self, rechecks: [Option<Recheck>; N]) -> Result {
        for (account, recheck) in self.accounts.iter().zip(rechecks) {
            if let Some(recheck) = recheck {
                recheck(&account.entry, P::IDS)?;
            }
        }
        Ok(())
    }

    /// # Safety
    /// Each signer in `signers` points to its seeds laid out for the runtime.
    /// `data_len` must be at most `D`.
    unsafe fn call(&self, signers: &[RawSlice], data_len: usize) {
        // Filled in place: `array::map` is a call SBF does not inline (measured).
        let mut metas = [const { MaybeUninit::<SolAccountMeta>::uninit() }; N];
        let mut infos = [const { MaybeUninit::<SolAccountInfo>::uninit() }; N];
        for ((meta, info), account) in metas.iter_mut().zip(&mut infos).zip(&self.accounts) {
            let entry = &account.entry;
            meta.write(SolAccountMeta {
                pubkey: entry.key(),
                is_writable: account.is_writable,
                is_signer: account.is_signer,
            });
            info.write(SolAccountInfo {
                key: entry.key(),
                lamports: entry.lamports_ptr(),
                data_len: entry.data().len() as u64,
                data: entry.data_ptr(),
                owner: entry.owner(),
                rent_epoch: 0,
                flags: entry.flags(),
            });
        }
        // A copy: a pointer into `self` would reach the syscall, and every
        // field of `self` would then be written to the stack first (measured).
        let data = self.data;
        let instruction = SolInstruction {
            program_id: self.program,
            accounts: metas.as_ptr().cast(),
            accounts_len: N as u64,
            data: data.as_ptr(),
            data_len: data_len as u64,
        };
        // SAFETY: every pointer refers to data that outlives the call: the
        // entries live in the input, the arrays on this stack frame. Writable
        // accounts are borrowed mutably, so no reference observes the
        // callee's changes during the call; `signers` is as the caller promised.
        unsafe {
            syscalls::invoke(
                ptr::from_ref(&instruction).cast(),
                infos.as_ptr().cast(),
                N as u64,
                signers.as_ptr().cast(),
                signers.len() as u64,
            )
        }
    }
}

/// A call that submits a checked prefix of its data buffer.
/// Fixed-size calls retain their original layout and compile-time length.
#[must_use = "a call does nothing until it is invoked"]
pub struct CpiWithDataLen<'a, P, const N: usize, const D: usize> {
    call: Cpi<'a, P, N, D>,
    len: usize,
}

impl<P: ProgramIds, const N: usize, const D: usize> CpiWithDataLen<'_, P, N, D> {
    /// The instruction a call will submit, for host clients and wire-format tests.
    #[cfg(not(target_os = "solana"))]
    pub fn instruction(&self) -> solana_instruction::Instruction {
        let mut instruction = self.call.instruction();
        instruction.data.truncate(self.len);
        instruction
    }

    pub fn invoke(self) -> Result {
        let rechecks = self.call.rechecks();
        // SAFETY: no signer seeds; `with_data_len` checked the length.
        unsafe { self.call.call(&[], self.len) };
        self.call.recheck_changed(rechecks)
    }

    /// Invokes with `signer` signing for its PDA.
    pub fn invoke_signed<const S: usize>(self, signer: &SignerSeeds<'_, S>) -> Result {
        let seeds = SignerSlices {
            seeds: signer.seeds.map(RawSlice::new),
            bump: RawSlice::new(&signer.bump),
        };
        let signers = [RawSlice { addr: ptr::from_ref(&seeds).cast(), len: S as u64 + 1 }];
        let rechecks = self.call.rechecks();
        // SAFETY: `SignerSlices` is `S + 1` contiguous `RawSlice`s, and
        // `with_data_len` checked the length.
        unsafe { self.call.call(&signers, self.len) };
        self.call.recheck_changed(rechecks)
    }
}

/// A signer's seeds then its bump: `S + 1` contiguous `RawSlice`s.
#[repr(C)]
struct SignerSlices<const S: usize> {
    seeds: [RawSlice; S],
    bump: RawSlice,
}

#[repr(C)]
struct SolInstruction {
    program_id: *const crate::Address,
    accounts: *const SolAccountMeta,
    accounts_len: u64,
    data: *const u8,
    data_len: u64,
}

#[repr(C)]
struct SolAccountMeta {
    pubkey: *const crate::Address,
    is_writable: bool,
    is_signer: bool,
}

#[repr(C)]
struct SolAccountInfo {
    key: *const crate::Address,
    lamports: *mut u64,
    data_len: u64,
    data: *mut u8,
    owner: *const crate::Address,
    rent_epoch: u64,
    /// `is_signer`, `is_writable` and `executable`, C `bool`s, in its low
    /// three bytes; the rest is the struct's padding.
    flags: u64,
}
