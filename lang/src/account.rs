//! Accounts a program owns and stores a `Kind` in: a discriminator, then the
//! kind's zeropod encoding, read and written in place.

use core::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
    slice,
};

use zeropod::Layout;

use crate::{
    Address, ProgramError, Result,
    cpi::{Check, Named},
    error::Error,
    field::FieldInfo,
    input::{Checked, Entry, Privileges},
    wrapper::{AfterCpi, ProgramIds, Wrapper, entry_accessors},
};

/// A type stored in accounts: a zeropod layout, its owner and discriminator.
/// Implemented by `#[account]`, and by hand for layouts another program
/// defines, such as SPL Token accounts.
pub trait Kind: Layout {
    const OWNER: Address;
    /// Empty only for a layout another program defines, whose `validate`
    /// must then reject an account that program has not initialized.
    const DISCRIMINATOR: &'static [u8];
    /// Whether this program is `OWNER`, so it alone can change these accounts.
    const PROGRAM_OWNED: bool;
    /// The name of the check its wrapper makes, if a call can make it instead.
    const CHECK: Option<Named> = None;
    /// Its fields' names.
    const FIELDS: &'static [&'static str] = &[];

    /// Checks beyond the layout's own: an SPL account's state, say.
    fn validate(_view: &Self::View) -> bool {
        true
    }

    /// The kind as the IDL describes it; `#[account]` sets it.
    #[cfg(not(target_os = "solana"))]
    const IDL: Option<crate::client::IdlKind> = None;
}

/// Implemented by `#[account]`, for `__Kinds` in the program's crate, once
/// per discriminator of a kind the program owns. Two such kinds with the same
/// discriminator would load as each other; their two identical impls are a
/// "conflicting implementations" error instead.
#[doc(hidden)]
pub trait UniqueDiscriminator<const LEN: usize, const BYTES: u128> {}

/// `K`'s discriminator width, if this program owns `K`.
pub const fn owned_width<K: Kind>() -> Option<usize> {
    if K::PROGRAM_OWNED { Some(K::DISCRIMINATOR.len()) } else { None }
}

/// Fails to compile unless every width given is the same. With one width,
/// no kind's discriminator can be another's prefix, so two kinds of equal
/// length cannot load as each other.
pub const fn one_width(structs: &[&[FieldInfo]]) {
    let mut first = None;
    let mut index = 0;
    while index < structs.len() {
        let mut field = 0;
        while field < structs[index].len() {
            if let Some(width) = structs[index][field].width {
                match first {
                    None => first = Some(width),
                    Some(first) => assert!(
                        first == width,
                        "the account kinds a program owns share one discriminator width"
                    ),
                }
            }
            field += 1;
        }
        index += 1;
    }
}

/// Data length of an account holding `K`, if every one has the same.
pub const fn len<K: Kind>() -> Option<usize> {
    match K::SIZE {
        Some(size) => Some(K::DISCRIMINATOR.len() + size),
        None => None,
    }
}

/// The most data an account holding `K` takes, every variable-length field
/// full: what `init` creates. `#[account]` bounds every such field.
pub fn capacity<K: Kind>() -> usize {
    let most = K::max_len(&[]).expect("an account kind bounds its fields with `#[max_len]`");
    K::DISCRIMINATOR.len() + most
}

/// Whether none of `owners` is the System Program, whose id is all zeros.
const fn assigned(owners: &[Address]) -> bool {
    let mut index = 0;
    while index < owners.len() {
        if is_zeroed(owners[index].as_array()) {
            return false;
        }
        index += 1;
    }
    true
}

/// Whether `bytes` are all zero, as in an account anyone can create with this
/// program as its owner.
const fn is_zeroed(bytes: &[u8]) -> bool {
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

/// Whether the account is owned by `K::OWNER`, starts with
/// `K::DISCRIMINATOR` and holds a valid `K`. Its length, when fixed, is the
/// caller's to check: the cursor's, for a wrapper.
pub(crate) fn holds<K: Kind>(entry: &Entry) -> bool {
    let data = entry.data();
    if *entry.owner() != K::OWNER || !data.starts_with(K::DISCRIMINATOR) {
        return false;
    }
    // SAFETY: the data starts with the discriminator. Not `strip_prefix`:
    // with it, the Kani proof of this check ran past 12 minutes, against
    // under a second (measured).
    let body = unsafe { data.get_unchecked(K::DISCRIMINATOR.len()..) };
    // SAFETY: the body holds a valid `K` once `check` accepts it.
    K::check(body, &[]).is_ok() && K::validate(unsafe { K::view_unchecked(body) })
}

#[cold]
fn mismatch<K: Kind>(entry: &Entry) -> ProgramError {
    if *entry.owner() != K::OWNER {
        ProgramError::IllegalOwner
    } else if !entry.data().starts_with(K::DISCRIMINATOR) {
        Error::Discriminator.into()
    } else {
        ProgramError::InvalidAccountData
    }
}

/// `Wrapper::why` for `Account<K>`.
fn why<K: Kind>(entry: &Entry, name: &str) -> &'static str {
    use crate::error::head;
    if *entry.owner() != K::OWNER {
        head("IllegalOwner", name)
            .text("is owned by ")
            .key(entry.owner())
            .text(", expected ")
            .key(&K::OWNER)
            .log();
        OWNER_HELP
    } else if !entry.data().starts_with(K::DISCRIMINATOR) {
        let found = &entry.data()[..entry.data().len().min(K::DISCRIMINATOR.len())];
        head("Discriminator", name)
            .text("starts with ")
            .bytes(found)
            .text(", expected the discriminator ")
            .bytes(K::DISCRIMINATOR)
            .log();
        "it is an account of the same program but another type, or was never written: check the address passed, and that the instruction creating it ran"
    } else {
        head("InvalidAccountData", name).text("does not hold a valid value of its type").log();
        "a field holds a value its type does not allow, such as a length past its `#[max_len]`, or the data was written with another layout"
    }
}

const OWNER_HELP: &str = "the account is not one the expected program created: check the address passed; an account that does not exist yet, or was closed, is owned by the System Program (11111111111111111111111111111111)";

/// An account owned by `K::OWNER`, starting with `K::DISCRIMINATOR`, then
/// holding a valid `K`: exactly `len::<K>()` long when that is known.
///
/// It derefs to `K`'s view, a method per field. A write that changes the
/// encoding's length runs on into the account's room to grow, up to its
/// capacity; the data length follows the encoding before a call and after
/// the handler (`Wrapper::flush`).
pub struct Account<K>(Entry, PhantomData<K>);

impl<K: Kind> Wrapper for Account<K> {
    const REQUIRES: Privileges = Privileges::NONE;
    const LEN: Option<usize> = len::<K>();
    const AFTER_CPI: AfterCpi =
        if K::PROGRAM_OWNED { AfterCpi::BrokenByCalling(&K::OWNER) } else { AfterCpi::Broken };
    const ASSIGNED: bool = assigned(&[K::OWNER]);
    const PROGRAM_OWNED: bool = K::PROGRAM_OWNED;
    const WIDTH: Option<usize> = owned_width::<K>();
    const RESIZES: bool = K::SIZE.is_none();
    const FIELDS: &'static [&'static str] = K::FIELDS;
    const CHECK: Option<Check> = match (K::CHECK, K::SIZE) {
        (Some(named), Some(_)) => Some(named.check()),
        _ => None,
    };

    /// The cursor has checked the length, which is all the view reads.
    #[inline(always)]
    fn vouched(entry: Entry, checked: Checked, waived: bool) -> Result<Self> {
        if waived { Ok(Self(entry, PhantomData)) } else { Self::from_entry(entry, checked) }
    }

    fn why(entry: &Entry, name: &str) -> &'static str {
        why::<K>(entry, name)
    }

    fn from_entry(entry: Entry, _: Checked) -> Result<Self> {
        const {
            assert!(
                K::DISCRIMINATOR.is_empty() || !is_zeroed(K::DISCRIMINATOR),
                "a zeroed account would pass as this kind"
            )
        };
        if holds::<K>(&entry) { Ok(Self(entry, PhantomData)) } else { Err(mismatch::<K>(&entry)) }
    }

    /// The encoding's length: a variable kind's writes leave the data
    /// length where it was.
    fn content_len(entry: &Entry) -> usize {
        let (data, len) = room::<K>(entry);
        // SAFETY: the room is inside the input and starts with a valid `K`,
        // which only a wrapper of `entry` could have written, as its view
        // keeps it.
        let view = unsafe { K::view_unchecked(slice::from_raw_parts(data, len)) };
        K::DISCRIMINATOR.len() + K::size(view)
    }

    #[cfg(not(target_os = "solana"))]
    fn idl_kind() -> Option<crate::client::IdlKind> {
        K::IDL
    }

    entry_accessors!();
}

/// The bytes after the discriminator, and the room after the data a
/// variable kind may grow into.
fn room<K: Kind>(entry: &Entry) -> (*mut u8, usize) {
    let prefix = K::DISCRIMINATOR.len();
    let len = match K::SIZE {
        Some(_) => entry.data().len(),
        None => entry.room(capacity::<K>()),
    };
    // SAFETY: the data starts with the discriminator.
    (unsafe { entry.data_ptr().add(prefix) }, len - prefix)
}

impl<K: Kind> Account<K> {
    /// An account `init` has just created with `K`'s owner, length and
    /// discriminator: only its content, all zeros, is checked, and not even
    /// that for a kind any bytes can hold.
    pub(crate) fn created(entry: Entry) -> Result<Self> {
        // SAFETY: `init` made the data at least the discriminator long.
        let body = unsafe { entry.data().get_unchecked(K::DISCRIMINATOR.len()..) };
        if !K::ANY_BYTES && K::check(body, &[]).is_err() {
            return Err(ProgramError::InvalidAccountData);
        }
        // SAFETY: the body holds a valid `K`, checked just now or any bytes.
        if !K::validate(unsafe { K::view_unchecked(body) }) {
            return Err(ProgramError::InvalidAccountData);
        }
        // SAFETY: `init` gave it `K`'s owner and discriminator, and the body
        // is valid.
        Ok(unsafe { Self::new_unchecked(entry) })
    }

    /// # Safety
    /// `K::OWNER` owns the account, whose data is `K::DISCRIMINATOR`, then a
    /// valid `K` accepted by `K::validate`, then only room to grow into.
    pub(crate) unsafe fn new_unchecked(entry: Entry) -> Self {
        Self(entry, PhantomData)
    }

    fn room(&self) -> (*mut u8, usize) {
        room::<K>(&self.0)
    }

    /// Writes the whole value, which may change the encoding's length.
    pub fn set_inner(&mut self, value: &K) -> Result {
        zeropod::write(value, &mut *self.room_mut())?;
        self.flush();
        Ok(())
    }

    fn room_mut(&mut self) -> &mut [u8] {
        let (data, len) = self.room();
        // SAFETY: the room is inside the input, after this account's
        // discriminator, and `&mut self` makes this the only reference.
        unsafe { slice::from_raw_parts_mut(data, len) }
    }

    /// Takes lamports out. Only the owner of an account may debit it, so this
    /// exists only for kinds this program owns.
    #[inline(always)]
    pub fn sub_lamports(&mut self, lamports: u64) -> Result {
        const { assert!(K::PROGRAM_OWNED, "only accounts this program owns can be debited") };
        self.entry_mut().sub_lamports(lamports)
    }
}

impl<K: Kind> Deref for Account<K> {
    type Target = K::View;

    fn deref(&self) -> &K::View {
        let (data, len) = self.room();
        // SAFETY: the room is inside the input and starts with a valid `K`,
        // checked as the account loaded and kept valid by its view.
        unsafe { K::view_unchecked(slice::from_raw_parts(data, len)) }
    }
}

/// Writes compile for any account; the runtime rejects writes to an account
/// the transaction did not mark writable when the instruction ends.
impl<K: Kind> DerefMut for Account<K> {
    fn deref_mut(&mut self) -> &mut K::View {
        // SAFETY: as in `deref`.
        unsafe { K::view_unchecked_mut(self.room_mut()) }
    }
}

/// A layout that any one of several programs may own, and that they may
/// extend past its base: SPL Token accounts under Token and Token-2022.
pub trait InterfaceKind: Layout {
    type Owners: ProgramIds;
    /// The name of the check its wrapper makes, if a call can make it instead.
    const CHECK: Option<Named> = None;

    /// Whether `data` holds this kind, the base layout first.
    fn validate(data: &[u8]) -> bool;

    /// With the `debug` feature, logs why `validate` rejects `data`, the
    /// account `name`'s; how to fix it.
    fn why(_data: &[u8], name: &str) -> &'static str {
        crate::error::head("InvalidAccountData", name)
            .text("does not hold a valid value of its type")
            .log();
        ""
    }
}

/// An account owned by any one of `K::Owners`, holding `K` and possibly more.
pub struct InterfaceAccount<K>(Entry, PhantomData<K>);

impl<K: InterfaceKind> Wrapper for InterfaceAccount<K> {
    const REQUIRES: Privileges = Privileges::NONE;
    const AFTER_CPI: AfterCpi = AfterCpi::Broken;
    const ASSIGNED: bool = assigned(K::Owners::IDS);
    const CHECK: Option<Check> = match K::CHECK {
        Some(named) => Some(named.check()),
        None => None,
    };

    /// The length is checked whatever is waived: the view reads that far.
    #[inline(always)]
    fn vouched(entry: Entry, checked: Checked, waived: bool) -> Result<Self> {
        if !waived {
            return Self::from_entry(entry, checked);
        }
        if entry.data().len() < K::SIZE.unwrap_or(0) {
            return Err(Error::DataLength.into());
        }
        Ok(Self(entry, PhantomData))
    }

    fn from_entry(entry: Entry, _: Checked) -> Result<Self> {
        if !K::Owners::IDS.contains(entry.owner()) {
            return Err(ProgramError::IllegalOwner);
        }
        if entry.data().len() < K::SIZE.unwrap_or(0) {
            return Err(Error::DataLength.into());
        }
        if K::validate(entry.data()) {
            Ok(Self(entry, PhantomData))
        } else {
            Err(ProgramError::InvalidAccountData)
        }
    }

    fn why(entry: &Entry, name: &str) -> &'static str {
        use crate::error::head;
        let (len, base) = (entry.data().len() as u64, K::SIZE.unwrap_or(0) as u64);
        if !K::Owners::IDS.contains(entry.owner()) {
            head("IllegalOwner", name)
                .text("is owned by ")
                .key(entry.owner())
                .text(", expected ")
                .keys(K::Owners::IDS)
                .log();
            OWNER_HELP
        } else if len < base {
            head("DataLength", name)
                .text("has ")
                .number(len)
                .text(" bytes of data, expected at least ")
                .number(base)
                .log();
            "the account holds something else, or was never initialized: check the address passed"
        } else {
            K::why(entry.data(), name)
        }
    }

    entry_accessors!();
}

impl<K: InterfaceKind> Deref for InterfaceAccount<K> {
    type Target = K::View;

    #[inline(always)]
    fn deref(&self) -> &K::View {
        // SAFETY: `validate` accepted the data, which starts with a valid `K`.
        unsafe { K::view_unchecked(self.0.data()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        input::{Cursor, Header, NOT_DUPLICATE, entry_size},
        wrapper::ProgramId,
    };

    struct Owner;

    impl ProgramId for Owner {
        const ID: Address = Address::new_from_array([7; 32]);
    }

    #[derive(zeropod::ZeroPod)]
    struct Probe {
        flag: bool,
    }

    impl InterfaceKind for Probe {
        type Owners = Owner;

        fn validate(data: &[u8]) -> bool {
            Self::view(data).is_ok()
        }
    }

    /// Whether an account of `owner` and `data_len` loads.
    fn loads(owner: [u8; 32], data_len: u64) -> bool {
        const WORDS: usize = (size_of::<u64>() + entry_size(8)) / size_of::<u64>();
        let mut input = [0u64; WORDS];
        input[0] = 1;
        let header = Header {
            dup: NOT_DUPLICATE,
            is_signer: 0,
            is_writable: 1,
            executable: 0,
            resize_delta: 0,
            key: Address::new_from_array([1; 32]),
            owner: Address::new_from_array(owner),
            lamports: 1,
            data_len,
        };
        let bytes = input.as_mut_ptr().cast::<u8>();
        // SAFETY: the header fits after the count, inside the buffer.
        unsafe { bytes.add(size_of::<u64>()).cast::<Header>().write(header) };
        // SAFETY: the buffer is laid out like the runtime's input.
        let mut cursor = unsafe { Cursor::new(bytes, &<Owner as ProgramId>::ID) };
        cursor.load::<InterfaceAccount<Probe>>(true).is_ok()
    }

    #[test]
    fn an_interface_account_is_its_owners_and_long_enough() {
        assert!(loads([7; 32], 1));
        assert!(loads([7; 32], 8), "an owner may extend its layout");
        assert!(!loads([8; 32], 1), "another owner");
        assert!(!loads([7; 32], 0), "shorter than the layout");
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;
    use crate::input::{Cursor, Header, NOT_DUPLICATE, entry_size};

    #[derive(zeropod::ZeroPod)]
    struct Probe {
        value: u64,
    }

    impl Kind for Probe {
        const OWNER: Address = Address::new_from_array([7; 32]);
        const DISCRIMINATOR: &'static [u8] = &[9];
        const PROGRAM_OWNED: bool = true;
    }

    /// Room for the count and one entry with up to 16 bytes of data.
    const INPUT_WORDS: usize = (size_of::<u64>() + entry_size(16)) / size_of::<u64>();

    #[kani::proof]
    fn account_is_built_exactly_when_owner_length_and_discriminator_match() {
        let mut input = [0u64; INPUT_WORDS];
        input[0] = 1;
        let owner_byte: u8 = kani::any();
        let data_len: u64 = kani::any();
        kani::assume(data_len <= 16);
        let first_byte: u8 = kani::any();
        let bytes = input.as_mut_ptr().cast::<u8>();
        let header = Header {
            dup: NOT_DUPLICATE,
            is_signer: 0,
            is_writable: 0,
            executable: 0,
            resize_delta: 0,
            key: Address::default(),
            owner: Address::new_from_array([owner_byte; 32]),
            lamports: 0,
            data_len,
        };
        // SAFETY: the header fits after the count, inside the buffer.
        unsafe { bytes.add(size_of::<u64>()).cast::<Header>().write(header) };
        // SAFETY: the data starts right after the header, inside the buffer.
        unsafe { *bytes.add(size_of::<u64>() + size_of::<Header>()) = first_byte };
        // SAFETY: the buffer is laid out like the runtime's input.
        let mut cursor = unsafe {
            Cursor::new(bytes, &<crate::wrapper::SystemProgram as crate::wrapper::ProgramId>::ID)
        };
        let built = cursor.load::<Account<Probe>>(false).is_ok();
        let holds = owner_byte == 7 && Some(data_len as usize) == len::<Probe>() && first_byte == 9;
        assert_eq!(built, holds);
    }
}
