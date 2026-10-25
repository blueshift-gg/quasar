//! The input the runtime serializes before calling a program (ABIv1, Agave
//! `serialization.rs`), read front to back one account entry at a time.

use core::{mem::size_of, ptr::NonNull, slice};

use crate::{Address, ProgramError, Result, error::Error, wrapper::Wrapper};

/// The fixed prefix of a non-duplicate account entry.
#[repr(C)]
pub(crate) struct Header {
    pub(crate) dup: u8,
    pub(crate) is_signer: u8,
    pub(crate) is_writable: u8,
    pub(crate) executable: u8,
    /// Zero in the input. The program keeps its growth here, which must stay
    /// within the realloc reserve (pinocchio uses these bytes the same way).
    pub(crate) resize_delta: i32,
    pub(crate) key: Address,
    pub(crate) owner: Address,
    pub(crate) lamports: u64,
    pub(crate) data_len: u64,
}

const _: () = assert!(size_of::<Header>() == 88);

/// `dup` of an entry that is not a repeat of an earlier account.
pub(crate) const NOT_DUPLICATE: u8 = u8::MAX;

/// The address bit `Entry::poison` sets: no region the runtime maps has it.
const POISON: core::num::NonZero<usize> = core::num::NonZero::new(1 << (usize::BITS - 1)).unwrap();

/// Space the runtime reserves after account data so the account can grow.
const REALLOC_RESERVE: usize = 10 * 1024;

/// Bytes a non-duplicate entry occupies: header, data and reserve padded to
/// eight bytes, then the rent epoch.
#[inline(always)]
pub(crate) const fn entry_size(data_len: usize) -> usize {
    // `next_multiple_of(8)` without its overflow check: `data_len` comes from
    // the runtime and is at most 10 MiB.
    ((size_of::<Header>() + data_len + REALLOC_RESERVE + 7) & !7) + size_of::<u64>()
}

/// Privileges a wrapper requires, as bits of an entry's first four bytes read
/// as a little-endian `u32`: `[dup, is_signer, is_writable, executable]`.
#[derive(Clone, Copy)]
pub struct Privileges(u32);

impl Privileges {
    pub const NONE: Self = Self(0);
    pub const SIGNER: Self = Self(1 << 8);
    pub const WRITABLE: Self = Self(1 << 16);
    pub const EXECUTABLE: Self = Self(1 << 24);

    pub const fn and(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn includes(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// One compare checks both that the entry is not a duplicate and that it has
/// every required privilege. Relies on the runtime writing flags as 0 or 1.
#[inline(always)]
fn check_header(word: u32, required: Privileges) -> Result {
    let expected = NOT_DUPLICATE as u32 | required.0;
    if word & expected == expected { Ok(()) } else { Err(header_error(word, required)) }
}

/// The first requirement `word` fails, in the order duplicate, signer,
/// writable, executable.
#[cold]
fn header_error(word: u32, required: Privileges) -> ProgramError {
    let missing = |privilege: Privileges| required.includes(privilege) && word & privilege.0 == 0;
    if word as u8 != NOT_DUPLICATE {
        Error::DuplicateAccount.into()
    } else if missing(Privileges::SIGNER) {
        ProgramError::MissingRequiredSignature
    } else if missing(Privileges::WRITABLE) {
        ProgramError::Immutable
    } else {
        Error::NotExecutable.into()
    }
}

/// With the `debug` feature, logs which requirement `word` fails for the
/// account `name`; how to fix it.
#[cold]
#[inline(never)]
fn header_why(word: u32, required: Privileges, name: &str) -> &'static str {
    use crate::error::head;
    let missing = |privilege: Privileges| required.includes(privilege) && word & privilege.0 == 0;
    if word as u8 != NOT_DUPLICATE {
        head("DuplicateAccount", name)
            .text("is the same account as account #")
            .number((word as u8).into())
            .text(" of the instruction")
            .log();
        "an instruction takes each account once: two of its accounts were given the same address; pass a different account for one of them"
    } else if missing(Privileges::SIGNER) {
        head("MissingRequiredSignature", name).text("did not sign the transaction").log();
        "mark it as a signer in the instruction (`AccountMeta::new(address, true)`) and sign the transaction with its keypair; if it is a PDA, the program that owns it signs for it with `invoke_signed`"
    } else if missing(Privileges::WRITABLE) {
        head("Immutable", name).text("is read-only in the instruction").log();
        "the program writes to it (`mut`, `init` or `close`): pass it writable (`AccountMeta::new`, not `AccountMeta::new_readonly`)"
    } else {
        head("NotExecutable", name).text("is not an executable program").log();
        "its field takes a program: pass the program's address"
    }
}

/// One account from the input. The cursor hands out at most one `Entry` per
/// account and rejects duplicates, so two entries never alias.
pub struct Entry(NonNull<Header>);

impl Entry {
    /// A second handle to this account, for checks that read it after a call
    /// or at the end of the instruction.
    ///
    /// # Safety
    /// No reference obtained through either handle may be live while the
    /// other one is used.
    pub(crate) unsafe fn alias(&self) -> Self {
        Self(self.0)
    }

    /// Makes the handle unusable until `restore`: it points nowhere the
    /// runtime maps, so any read or write through it ends the program with
    /// an access violation, at no cost to the reads that are allowed. The
    /// address keeps the entry's, to restore.
    pub(crate) fn poison(&mut self) {
        self.0 = self.0.map_addr(|address| address | POISON);
    }

    pub(crate) fn restore(&mut self) {
        // SAFETY: an entry's own address is never zero.
        self.0 = self.0.map_addr(|address| unsafe {
            core::num::NonZero::new_unchecked(address.get() & !POISON.get())
        });
    }

    /// `is_signer`, `is_writable` and `executable` in the low three bytes, as
    /// the runtime writes them: 0 or 1. One load and a shift, not three copies.
    pub(crate) fn flags(&self) -> u64 {
        // SAFETY: the header starts with these four bytes.
        u64::from(unsafe { self.0.cast::<u32>().read() } >> 8)
    }

    /// With the `debug` feature, says why before a poisoned handle faults.
    #[inline(always)]
    fn live(&self) {
        if cfg!(feature = "debug") && self.0.addr().get() & POISON.get() != 0 {
            self.say_reload();
        }
    }

    #[cold]
    #[inline(never)]
    fn say_reload(&self) -> ! {
        // SAFETY: the address without the mark is the entry's own.
        let key = unsafe { &(*self.0.as_ptr().map_addr(|address| address & !POISON.get())).key };
        crate::error::Line::new()
            .text("error[reload]: account ")
            .key(key)
            .text(" was used after a call that may have changed or closed it")
            .log();
        crate::error::located(
            &crate::error::At::UNNAMED,
            "it was passed writable to a call into another program, so what was read from it before may no longer hold: call `.reload()?` on it right after that call and before using it again; if nothing needs it after the call, remove that use",
        );
        crate::syscalls::end()
    }

    pub(crate) fn header(&self) -> &Header {
        self.live();
        // SAFETY: the cursor only builds an `Entry` over a complete header
        // inside the input, which outlives the program's execution.
        unsafe { self.0.as_ref() }
    }

    pub fn key(&self) -> &Address {
        &self.header().key
    }

    pub fn owner(&self) -> &Address {
        &self.header().owner
    }

    pub fn lamports(&self) -> u64 {
        self.header().lamports
    }

    pub fn is_signer(&self) -> bool {
        self.header().is_signer != 0
    }

    pub fn is_writable(&self) -> bool {
        self.header().is_writable != 0
    }

    pub fn executable(&self) -> bool {
        self.header().executable != 0
    }

    pub fn data(&self) -> &[u8] {
        // SAFETY: the runtime places `data_len` bytes of data right after the header.
        unsafe { slice::from_raw_parts(self.data_ptr(), self.header().data_len as usize) }
    }

    /// The first data byte, derived from the entry's own pointer rather than
    /// a reference, so it may be written through.
    pub(crate) fn data_ptr(&self) -> *mut u8 {
        self.live();
        // SAFETY: the data follows the header inside the input.
        unsafe { self.0.as_ptr().add(1).cast() }
    }

    fn header_mut(&mut self) -> &mut Header {
        // SAFETY: as in `header`; `&mut self` makes this the only reference.
        unsafe { self.0.as_mut() }
    }

    pub(crate) fn data_mut(&mut self) -> &mut [u8] {
        let len = self.header().data_len as usize;
        // SAFETY: as in `data`; `&mut self` makes this the only reference.
        unsafe { slice::from_raw_parts_mut(self.data_ptr(), len) }
    }

    /// The balance, derived like `data_ptr`, for a callee to write.
    pub(crate) fn lamports_ptr(&self) -> *mut u64 {
        self.live();
        // SAFETY: the header is inside the input; no reference is created.
        unsafe { &raw mut (*self.0.as_ptr()).lamports }
    }

    pub(crate) fn set_lamports(&mut self, lamports: u64) {
        self.header_mut().lamports = lamports;
    }

    pub(crate) fn sub_lamports(&mut self, lamports: u64) -> Result {
        let balance =
            self.lamports().checked_sub(lamports).ok_or(ProgramError::InsufficientFunds)?;
        self.set_lamports(balance);
        Ok(())
    }

    pub(crate) fn set_owner(&mut self, owner: Address) {
        self.header_mut().owner = owner;
    }

    /// Changes the data length, zeroing any new bytes.
    pub(crate) fn resize(&mut self, len: usize) -> Result {
        let old = self.header().data_len as usize;
        self.set_len(len)?;
        if len > old {
            // SAFETY: `set_len` kept the data inside the realloc reserve that
            // follows it in the input.
            unsafe { self.data_ptr().add(old).write_bytes(0, len - old) };
        }
        Ok(())
    }

    /// Empties the data of an account being closed; nothing resizes it after.
    pub(crate) fn clear(&mut self) {
        self.header_mut().data_len = 0;
    }

    /// Changes the data length and nothing else, for a writer that already
    /// filled any new bytes through `data_up_to`.
    pub(crate) fn set_len(&mut self, len: usize) -> Result {
        let header = self.header_mut();
        let delta = grown(header.data_len, header.resize_delta, len as u64)
            .ok_or(ProgramError::InvalidRealloc)?;
        header.data_len = len as u64;
        header.resize_delta = delta;
        Ok(())
    }

    /// `len`, capped at the most the data can grow to: the realloc reserve
    /// past its original length.
    pub(crate) fn room(&self, len: usize) -> usize {
        let header = self.header();
        let original = header.data_len as i64 - i64::from(header.resize_delta);
        len.min((original + REALLOC_RESERVE as i64) as usize)
    }
}

/// The growth after resizing from `len` (already grown by `delta`) to `new`,
/// or `None` if it would pass the realloc reserve.
fn grown(len: u64, delta: i32, new: u64) -> Option<i32> {
    let original = len as i64 - i64::from(delta);
    let growth = new as i64 - original;
    if growth > REALLOC_RESERVE as i64 { None } else { i32::try_from(growth).ok() }
}

/// Proof that the cursor, or a recheck, checked an entry's privileges. Only
/// this crate can make one, so only checked entries become wrappers.
pub struct Checked(());

/// Checks a wrapper's fixed length and builds it from an entry whose
/// privileges already hold, as they do for good once the cursor checked them:
/// for an account being loaded, or one a call has just changed.
#[inline(always)]
pub(crate) fn finish<W: Wrapper>(entry: Entry) -> Result<W> {
    fixed_len::<W>(&entry)?;
    W::from_entry(entry, Checked(()))
}

#[inline(always)]
fn fixed_len<W: Wrapper>(entry: &Entry) -> Result {
    if W::LEN.is_some_and(|len| len != entry.data().len()) {
        return Err(Error::DataLength.into());
    }
    Ok(())
}

/// With the `debug` feature, logs why `entry`, the account `name`, does not
/// load as a `W`: its length if that is fixed and differs, or else what `W`
/// says; how to fix it.
#[cold]
#[inline(never)]
pub(crate) fn why<W: Wrapper>(entry: &Entry, name: &str) -> &'static str {
    match W::LEN {
        Some(len) if len != entry.data().len() => {
            crate::error::head("DataLength", name)
                .text("has ")
                .number(entry.data().len() as u64)
                .text(" bytes of data, expected ")
                .number(len as u64)
                .log();
            "the account holds something else: another type, an older layout, or nothing yet (0 bytes: it was never created, or was closed); check the address passed"
        }
        _ => W::why(entry, name),
    }
}

/// Builds `W` from an account a call has just made one: only its length is
/// checked.
///
/// # Safety
/// A call that succeeded made the check `W::CHECK` names of the account, and
/// nothing has changed the account since.
pub unsafe fn made<W: Wrapper>(entry: Entry) -> Result<W> {
    fixed_len::<W>(&entry)?;
    W::vouched(entry, Checked(()), true)
}

/// `account` as a `W` if it already is one, as `init(idempotent)` finds it;
/// otherwise `account` back, to create. `account`'s privileges, checked as it
/// loaded, must cover `W`'s.
pub fn existing<W: Wrapper, A: Wrapper>(account: A) -> core::result::Result<W, A> {
    const { assert!(A::REQUIRES.includes(W::REQUIRES), "the account was loaded with fewer privileges") };
    // SAFETY: only one of the two handles survives: the new wrapper, or
    // `account` when there is none.
    match finish::<W>(unsafe { account.entry().alias() }) {
        Ok(existing) => Ok(existing),
        Err(_) => Err(account),
    }
}

/// Reads account entries in order and turns each into a wrapper.
pub struct Cursor {
    next: *mut u8,
    left: u64,
    count: u64,
    /// The running program, whose address a client passes in place of an
    /// optional account it leaves out.
    program: &'static Address,
    /// The index of the first such entry, which later ones duplicate.
    absent: Option<u8>,
}

impl Cursor {
    /// # Safety
    /// `input` is the pointer the runtime passed to the entrypoint, and no
    /// other `Cursor` over it exists.
    pub(crate) unsafe fn new(input: *mut u8, program: &'static Address) -> Self {
        // SAFETY: the input starts with the account count.
        let count = unsafe { input.cast::<u64>().read() };
        Self {
            // SAFETY: the first entry follows the eight-byte count.
            next: unsafe { input.add(size_of::<u64>()) },
            left: count,
            count,
            program,
            absent: None,
        }
    }

    /// A cursor over every account this one has not loaded, leaving this one empty.
    pub(crate) fn take_rest(&mut self) -> Self {
        let rest = Self { ..*self };
        self.left = 0;
        rest
    }

    /// Accounts not yet loaded.
    pub(crate) fn len(&self) -> usize {
        self.left as usize
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.left == 0
    }

    #[cold]
    #[inline(never)]
    fn missing(&self, at: &crate::error::At) {
        crate::error::head("NotEnoughAccountKeys", at.name)
            .text("was not passed: the instruction received ")
            .number(self.count)
            .text(" accounts")
            .log();
        crate::error::located(
            at,
            "pass every account of the instruction, in the order its accounts struct declares them",
        );
    }

    /// Fails unless at least `count` accounts are left, so the loads that
    /// follow need not each check.
    #[inline(always)]
    pub fn require(&self, count: u64) -> Result {
        if self.left < count { Err(ProgramError::NotEnoughAccountKeys) } else { Ok(()) }
    }

    /// Loads `W` from the next entry, or `None` if the client passed the
    /// program's address in its place to mark the account as absent.
    pub fn load_optional<W: Wrapper>(&mut self, writable: bool) -> Result<Option<W>> {
        if self.left == 0 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        // Entries are indexed by a `u8`: a transaction has at most 256 accounts.
        let index = (self.count - self.left) as u8;
        // SAFETY: `left > 0`, so an entry starts at `next` with its `dup` byte.
        let dup = unsafe { *self.next };
        let size = if dup != NOT_DUPLICATE {
            // A duplicate entry is its index byte padded to eight.
            (Some(dup) == self.absent).then_some(size_of::<u64>())
        } else {
            // SAFETY: the entry is not a duplicate, so a full header starts at `next`.
            let header = unsafe { &*self.next.cast::<Header>() };
            (header.key == *self.program).then(|| {
                self.absent = Some(index);
                entry_size(header.data_len as usize)
            })
        };
        let Some(size) = size else {
            return self.load(writable).map(Some);
        };
        // SAFETY: `size` is the entry's size in the input.
        self.next = unsafe { self.next.add(size) };
        self.left -= 1;
        Ok(None)
    }

    /// Checks the next entry's privileges (with the writable one added for a
    /// field declared `mut`) and, for fixed-size wrappers, its data length,
    /// then builds `W` from it.
    #[inline(always)]
    pub fn load<W: Wrapper>(&mut self, writable: bool) -> Result<W> {
        self.load_vouched(writable, false, &crate::error::At::UNNAMED)
    }

    /// `load`, or if `waived` without the wrapper's named check (`Wrapper::vouched`).
    #[inline(always)]
    pub fn load_vouched<W: Wrapper>(
        &mut self,
        writable: bool,
        waived: bool,
        at: &crate::error::At,
    ) -> Result<W> {
        let required = if writable { W::REQUIRES.and(Privileges::WRITABLE) } else { W::REQUIRES };
        if self.left == 0 {
            if cfg!(feature = "debug") {
                self.missing(at);
            }
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        // SAFETY: `left > 0`, so an entry starts at `next`, and every entry
        // starts with at least the four bytes of the header word.
        let word = unsafe { self.next.cast::<u32>().read() };
        if let Err(error) = check_header(word, required) {
            if cfg!(feature = "debug") {
                crate::error::located(at, header_why(word, required, at.name));
            }
            return Err(error);
        }
        // SAFETY: the entry is not a duplicate, so a full header starts at `next`.
        let entry = Entry(unsafe { NonNull::new_unchecked(self.next.cast()) });
        let data_len = entry.header().data_len as usize;
        // SAFETY: used only to say why the entry did not load, once it has not.
        let rejected = unsafe { entry.alias() };
        let loaded = match fixed_len::<W>(&entry) {
            Ok(()) => W::vouched(entry, Checked(()), waived),
            Err(error) => Err(error),
        };
        let wrapper = match loaded {
            Ok(wrapper) => wrapper,
            Err(error) => {
                if cfg!(feature = "debug") {
                    crate::error::located(at, why::<W>(&rejected, at.name));
                }
                return Err(error);
            }
        };
        // SAFETY: the runtime lays out the entry exactly as `entry_size`
        // describes, and `W::LEN`, when set, equals `data_len`.
        self.next = unsafe { self.next.add(entry_size(W::LEN.unwrap_or(data_len))) };
        self.left -= 1;
        Ok(wrapper)
    }
}

/// The instruction data the runtime passes in register r2 (SIMD-0321),
/// preceded by its length.
///
/// # Safety
/// `data` is the pointer the runtime passed to the entrypoint in r2.
pub(crate) unsafe fn instruction_data(data: *const u8) -> &'static [u8] {
    // SAFETY: the data length is the `u64` right before the data.
    let len = unsafe { data.sub(size_of::<u64>()).cast::<u64>().read() } as usize;
    // SAFETY: `len` bytes of data start at `data`.
    unsafe { slice::from_raw_parts(data, len) }
}

#[cfg(kani)]
mod proofs {
    use super::*;
    use crate::{verify::any_flag, wrapper::Signer};

    /// Room for the count and one entry with no data.
    const INPUT_WORDS: usize = (size_of::<u64>() + entry_size(0)) / size_of::<u64>();

    #[kani::proof]
    fn the_header_check_passes_exactly_when_privileges_hold() {
        let [dup, is_signer, is_writable, executable] =
            [kani::any(), any_flag(), any_flag(), any_flag()];
        let [needs_signer, needs_writable, needs_executable]: [bool; 3] = kani::any();
        let mut required = Privileges::NONE;
        if needs_signer {
            required = required.and(Privileges::SIGNER);
        }
        if needs_writable {
            required = required.and(Privileges::WRITABLE);
        }
        if needs_executable {
            required = required.and(Privileges::EXECUTABLE);
        }
        let word = u32::from_le_bytes([dup, is_signer, is_writable, executable]);
        let holds = dup == NOT_DUPLICATE
            && (!needs_signer || is_signer == 1)
            && (!needs_writable || is_writable == 1)
            && (!needs_executable || executable == 1);
        assert_eq!(check_header(word, required).is_ok(), holds);
    }

    #[kani::proof]
    fn resizing_never_grows_past_the_reserve() {
        let (original, first, second): (u32, u32, u32) = kani::any();
        let original = u64::from(original);
        let mut len = original;
        let mut delta = 0;
        for new in [u64::from(first), u64::from(second)] {
            if let Some(grown) = grown(len, delta, new) {
                len = new;
                delta = grown;
            }
        }
        assert!(len <= original + REALLOC_RESERVE as u64);
        assert_eq!(i64::from(delta), len as i64 - original as i64);
    }

    #[kani::proof]
    fn signer_is_built_exactly_when_the_account_signed() {
        let mut input = [0u64; INPUT_WORDS];
        input[0] = 1;
        let dup: u8 = kani::any();
        let is_signer = any_flag();
        let bytes = input.as_mut_ptr().cast::<u8>();
        // SAFETY: the buffer holds the count and one full entry.
        unsafe { bytes.add(size_of::<u64>()).cast::<[u8; 2]>().write([dup, is_signer]) };
        // SAFETY: the buffer is laid out like the runtime's input.
        let mut cursor = unsafe {
            Cursor::new(bytes, &<crate::wrapper::SystemProgram as crate::wrapper::ProgramId>::ID)
        };
        let built = cursor.load::<Signer>(false).is_ok();
        assert_eq!(built, dup == NOT_DUPLICATE && is_signer == 1);
    }
}
