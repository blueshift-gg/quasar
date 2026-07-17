//! Host unit tests for the account validation predicates.
//!
//! These are the primary mutation-testing oracle for the validation core
//! (TESTING.md): every predicate has an accepting case, a rejecting case per
//! failure mode, and boundary cases chosen so the classic mutants — `replace
//! check -> Ok(())`, a flipped comparison, an off-by-one bound, a skipped
//! word in a key comparison — each fail at least one test here without
//! needing an SBF build.

extern crate alloc;

use {
    super::{
        Address as AddressCheck, DataLen, Discriminator as DiscriminatorCheck, Executable, Mutable,
        Owner as OwnerCheck, Signer as SignerCheck, ZeroPod as ZeroPodCheck,
    },
    crate::{
        __internal::{AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED},
        __zeropod::{LayoutKind, ZeroPodError, ZeroPodFixed, ZeroPodSchema},
        account_layout::AccountLayout,
        traits,
    },
    alloc::{vec, vec::Vec},
    core::mem::size_of,
    solana_address::Address,
    solana_program_error::ProgramError,
};

const PROGRAM_ID: [u8; 32] = [7; 32];
const OWNER_ID: [u8; 32] = [9; 32];

/// One corrupted byte per 8-byte word plus both ends, so a key comparison
/// that skips a word or truncates the tail is always caught.
const CORRUPT_POSITIONS: [usize; 8] = [0, 7, 8, 15, 16, 23, 24, 31];

/// 8-byte-aligned buffer holding a `RuntimeAccount` header followed by data,
/// mirroring the layout the SVM entrypoint hands to programs (same
/// construction as `lang/tests/miri.rs`).
struct AccountBuffer {
    inner: Vec<u64>,
}

impl AccountBuffer {
    fn build(
        address: [u8; 32],
        owner: [u8; 32],
        data: &[u8],
        is_signer: bool,
        is_writable: bool,
        executable: bool,
    ) -> Self {
        let byte_len = size_of::<RuntimeAccount>()
            + data.len()
            + MAX_PERMITTED_DATA_INCREASE
            + size_of::<u64>();
        let mut buf = Self {
            inner: vec![0; byte_len.div_ceil(8)],
        };
        let raw = buf.raw();
        // SAFETY: the buffer is sized and 8-aligned for a leading
        // RuntimeAccount; all fields are written before any view is taken.
        unsafe {
            (*raw).borrow_state = NOT_BORROWED;
            (*raw).is_signer = is_signer as u8;
            (*raw).is_writable = is_writable as u8;
            (*raw).executable = executable as u8;
            (*raw).padding = [0u8; 4];
            (*raw).address = Address::new_from_array(address);
            (*raw).owner = Address::new_from_array(owner);
            (*raw).lamports = 1;
            (*raw).data_len = data.len() as u64;
        }
        let data_start = size_of::<RuntimeAccount>();
        // SAFETY: the buffer reserves `data.len()` bytes directly after the
        // RuntimeAccount header.
        let dst = unsafe {
            core::slice::from_raw_parts_mut(
                (buf.inner.as_mut_ptr() as *mut u8).add(data_start),
                data.len(),
            )
        };
        dst.copy_from_slice(data);
        buf
    }

    fn raw(&mut self) -> *mut RuntimeAccount {
        self.inner.as_mut_ptr() as *mut RuntimeAccount
    }

    fn view(&mut self) -> AccountView {
        // SAFETY: `build` fully initialized the header and data region, and
        // the buffer outlives the view for the duration of each test.
        unsafe { AccountView::new_unchecked(self.raw()) }
    }
}

fn header_account(is_signer: bool, is_writable: bool, executable: bool) -> AccountBuffer {
    AccountBuffer::build(
        PROGRAM_ID,
        OWNER_ID,
        &[],
        is_signer,
        is_writable,
        executable,
    )
}

// --- fixture types -------------------------------------------------------

struct TheProgram;
impl traits::Id for TheProgram {
    const ID: Address = Address::new_from_array(PROGRAM_ID);
}
impl AddressCheck for TheProgram {}

struct Owned;
impl traits::Owner for Owned {
    const OWNER: Address = Address::new_from_array(OWNER_ID);
}
impl OwnerCheck for Owned {}

struct Header;
impl SignerCheck for Header {}
impl Mutable for Header {}
impl Executable for Header {}

/// Two-byte schema whose first byte must be 0 or 1 (`PodBool`-like), giving
/// `validate` a real rejection path.
#[derive(Clone, Copy)]
#[repr(transparent)]
struct GuardZc([u8; 2]);

struct Guard;
impl ZeroPodSchema for Guard {
    const LAYOUT: LayoutKind = LayoutKind::Fixed;
}
impl ZeroPodFixed for Guard {
    type Zc = GuardZc;
    const SIZE: usize = 2;

    fn from_bytes(data: &[u8]) -> Result<&GuardZc, ZeroPodError> {
        Self::validate(data)?;
        // SAFETY: validate confirmed len >= SIZE; GuardZc is
        // repr(transparent) over [u8; 2] with alignment 1.
        Ok(unsafe { &*(data.as_ptr() as *const GuardZc) })
    }

    fn from_bytes_mut(data: &mut [u8]) -> Result<&mut GuardZc, ZeroPodError> {
        Self::validate(data)?;
        // SAFETY: as in from_bytes.
        Ok(unsafe { &mut *(data.as_mut_ptr() as *mut GuardZc) })
    }

    fn validate(data: &[u8]) -> Result<(), ZeroPodError> {
        if data.len() < Self::SIZE {
            return Err(ZeroPodError::BufferTooSmall);
        }
        if data[0] > 1 {
            return Err(ZeroPodError::InvalidBool);
        }
        Ok(())
    }
}

/// Guard schema behind a 3-byte prefix: layout range is `3..5`.
struct GuardedLayout;
impl AccountLayout for GuardedLayout {
    type Schema = Guard;
    const DATA_OFFSET: usize = 3;
}
impl DataLen for GuardedLayout {}
impl ZeroPodCheck for GuardedLayout {}

/// Multi-byte discriminator so per-byte comparison bounds are observable.
struct MultiDisc;
impl traits::Discriminator for MultiDisc {
    const DISCRIMINATOR: &'static [u8] = &[0xAB, 0xCD, 0xEF];
}
impl DiscriminatorCheck for MultiDisc {}

// --- checks::Address -----------------------------------------------------

#[test]
fn address_accepts_exact_match() {
    let mut acc = AccountBuffer::build(PROGRAM_ID, OWNER_ID, &[], false, false, false);
    assert_eq!(TheProgram::check(&acc.view()), Ok(()));
}

#[test]
fn address_rejects_single_byte_mismatch_in_every_word() {
    for pos in CORRUPT_POSITIONS {
        let mut address = PROGRAM_ID;
        address[pos] ^= 0x01;
        let mut acc = AccountBuffer::build(address, OWNER_ID, &[], false, false, false);
        assert_eq!(
            TheProgram::check(&acc.view()),
            Err(ProgramError::IncorrectProgramId),
            "corrupted byte {pos} must be detected"
        );
    }
}

// --- checks::Owner -------------------------------------------------------

#[test]
fn owner_accepts_exact_match() {
    let mut acc = AccountBuffer::build(PROGRAM_ID, OWNER_ID, &[], false, false, false);
    assert_eq!(Owned::check(&acc.view()), Ok(()));
}

#[test]
fn owner_rejects_single_byte_mismatch_in_every_word() {
    for pos in CORRUPT_POSITIONS {
        let mut owner = OWNER_ID;
        owner[pos] ^= 0x01;
        let mut acc = AccountBuffer::build(PROGRAM_ID, owner, &[], false, false, false);
        assert_eq!(
            Owned::check(&acc.view()),
            Err(ProgramError::IllegalOwner),
            "corrupted byte {pos} must be detected"
        );
    }
}

// --- checks::Signer / Mutable / Executable -------------------------------

#[test]
fn signer_accepts_signer_and_rejects_non_signer() {
    let mut signer = header_account(true, false, false);
    assert_eq!(<Header as SignerCheck>::check(&signer.view()), Ok(()));

    let mut non_signer = header_account(false, true, true);
    assert_eq!(
        <Header as SignerCheck>::check(&non_signer.view()),
        Err(ProgramError::MissingRequiredSignature)
    );
}

#[test]
fn mutable_accepts_writable_and_rejects_readonly() {
    let mut writable = header_account(false, true, false);
    assert_eq!(<Header as Mutable>::check(&writable.view()), Ok(()));

    let mut readonly = header_account(true, false, true);
    assert_eq!(
        <Header as Mutable>::check(&readonly.view()),
        Err(ProgramError::Immutable)
    );
}

#[test]
fn executable_accepts_program_and_rejects_data_account() {
    let mut program = header_account(false, false, true);
    assert_eq!(<Header as Executable>::check(&program.view()), Ok(()));

    let mut data_account = header_account(true, true, false);
    assert_eq!(
        <Header as Executable>::check(&data_account.view()),
        Err(ProgramError::InvalidAccountData)
    );
}

// --- checks::DataLen ------------------------------------------------------

#[test]
fn data_len_boundary_is_exact() {
    // Layout range is DATA_OFFSET(3) + DATA_SIZE(2) = 5 bytes.
    let mut short = AccountBuffer::build(PROGRAM_ID, OWNER_ID, &[0; 4], false, false, false);
    assert_eq!(
        <GuardedLayout as DataLen>::check(&short.view()),
        Err(ProgramError::AccountDataTooSmall)
    );

    let mut exact = AccountBuffer::build(PROGRAM_ID, OWNER_ID, &[0; 5], false, false, false);
    assert_eq!(<GuardedLayout as DataLen>::check(&exact.view()), Ok(()));

    let mut longer = AccountBuffer::build(PROGRAM_ID, OWNER_ID, &[0; 6], false, false, false);
    assert_eq!(<GuardedLayout as DataLen>::check(&longer.view()), Ok(()));
}

// --- checks::Discriminator ------------------------------------------------

#[test]
fn discriminator_accepts_exact_and_prefixed_data() {
    assert_eq!(MultiDisc::check_data(&[0xAB, 0xCD, 0xEF]), Ok(()));
    assert_eq!(
        MultiDisc::check_data(&[0xAB, 0xCD, 0xEF, 0x00, 0x42]),
        Ok(())
    );
}

#[test]
fn discriminator_rejects_short_data() {
    assert_eq!(
        MultiDisc::check_data(&[]),
        Err(ProgramError::AccountDataTooSmall)
    );
    assert_eq!(
        MultiDisc::check_data(&[0xAB, 0xCD]),
        Err(ProgramError::AccountDataTooSmall)
    );
}

#[test]
fn discriminator_rejects_mismatch_at_every_byte() {
    // First, middle, and last byte each individually corrupted: a comparison
    // loop that stops early or skips the tail must fail here.
    assert_eq!(
        MultiDisc::check_data(&[0xAA, 0xCD, 0xEF]),
        Err(ProgramError::InvalidAccountData)
    );
    assert_eq!(
        MultiDisc::check_data(&[0xAB, 0xCC, 0xEF]),
        Err(ProgramError::InvalidAccountData)
    );
    assert_eq!(
        MultiDisc::check_data(&[0xAB, 0xCD, 0xEE]),
        Err(ProgramError::InvalidAccountData)
    );
}

#[test]
fn discriminator_view_paths_match_check_data() {
    let mut ok = AccountBuffer::build(
        PROGRAM_ID,
        OWNER_ID,
        &[0xAB, 0xCD, 0xEF, 0x00],
        false,
        false,
        false,
    );
    assert_eq!(MultiDisc::check(&ok.view()), Ok(()));
    assert_eq!(MultiDisc::check_checked(&ok.view()), Ok(()));

    let mut bad = AccountBuffer::build(
        PROGRAM_ID,
        OWNER_ID,
        &[0xAB, 0xCD, 0xEE, 0x00],
        false,
        false,
        false,
    );
    assert_eq!(
        MultiDisc::check(&bad.view()),
        Err(ProgramError::InvalidAccountData)
    );
    assert_eq!(
        MultiDisc::check_checked(&bad.view()),
        Err(ProgramError::InvalidAccountData)
    );
}

// --- checks::ZeroPod ------------------------------------------------------

#[test]
fn zeropod_rejects_data_shorter_than_layout_range() {
    assert_eq!(
        GuardedLayout::check_data(&[0; 4]),
        Err(ProgramError::AccountDataTooSmall)
    );
}

#[test]
fn zeropod_validates_exactly_the_layout_window() {
    // Bytes before DATA_OFFSET are not schema bytes: junk there must be
    // ignored, junk inside the window must reject. Kills offset-arithmetic
    // mutants (offset -> 0, offset + size -> offset - size, ...).
    assert_eq!(GuardedLayout::check_data(&[9, 9, 9, 1, 7]), Ok(()));
    assert_eq!(
        GuardedLayout::check_data(&[0, 0, 0, 2, 0]),
        Err(ProgramError::InvalidAccountData)
    );
}

#[test]
fn zeropod_view_paths_match_check_data() {
    let mut ok = AccountBuffer::build(PROGRAM_ID, OWNER_ID, &[9, 9, 9, 0, 0], false, false, false);
    assert_eq!(<GuardedLayout as ZeroPodCheck>::check(&ok.view()), Ok(()));
    assert_eq!(
        <GuardedLayout as ZeroPodCheck>::check_checked(&ok.view()),
        Ok(())
    );

    let mut bad = AccountBuffer::build(PROGRAM_ID, OWNER_ID, &[0, 0, 0, 9, 0], false, false, false);
    assert_eq!(
        <GuardedLayout as ZeroPodCheck>::check(&bad.view()),
        Err(ProgramError::InvalidAccountData)
    );
    assert_eq!(
        <GuardedLayout as ZeroPodCheck>::check_checked(&bad.view()),
        Err(ProgramError::InvalidAccountData)
    );
}
