//! Miri UB tests for quasar-metadata unsafe code paths.
//!
//! ## Run
//!
//! ```sh
//! MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
//!   cargo +nightly miri test -p quasar-metadata --test miri
//! ```
use {
    quasar_lang::{
        __internal::{AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED},
        account_load::AccountLoad,
        accounts::Account,
    },
    quasar_metadata::{
        validate::validate_master_edition_account, MasterEditionAccount, MasterEditionPrefixZc,
        MetadataAccount, MetadataPrefixZc,
    },
    solana_address::Address,
    solana_program_error::ProgramError,
    std::mem::size_of,
};

const METADATA_OWNER: [u8; 32] = [
    11, 112, 101, 177, 227, 209, 124, 69, 56, 157, 82, 127, 107, 4, 195, 205, 88, 184, 108, 115,
    26, 160, 253, 181, 73, 182, 209, 188, 3, 248, 41, 70,
];

struct AccountBuffer {
    inner: Vec<u64>,
}

impl AccountBuffer {
    fn new(data_len: usize) -> Self {
        let byte_len =
            size_of::<RuntimeAccount>() + data_len + MAX_PERMITTED_DATA_INCREASE + size_of::<u64>();
        let u64_count = byte_len.div_ceil(8);
        Self {
            inner: vec![0; u64_count],
        }
    }

    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.inner.as_mut_ptr() as *mut u8
    }

    fn raw(&mut self) -> *mut RuntimeAccount {
        self.inner.as_mut_ptr() as *mut RuntimeAccount
    }

    fn init(
        &mut self,
        address: [u8; 32],
        owner: [u8; 32],
        lamports: u64,
        data_len: u64,
        is_signer: bool,
        is_writable: bool,
    ) {
        let raw = self.raw();
        unsafe {
            (*raw).borrow_state = NOT_BORROWED;
            (*raw).is_signer = is_signer as u8;
            (*raw).is_writable = is_writable as u8;
            (*raw).executable = 0;
            (*raw).padding = [0u8; 4];
            (*raw).address = Address::new_from_array(address);
            (*raw).owner = Address::new_from_array(owner);
            (*raw).lamports = lamports;
            (*raw).data_len = data_len;
        }
    }

    unsafe fn view(&mut self) -> AccountView {
        AccountView::new_unchecked(self.raw())
    }

    fn write_data(&mut self, data: &[u8]) {
        let data_start = size_of::<RuntimeAccount>();
        let dst = unsafe {
            std::slice::from_raw_parts_mut(self.as_mut_ptr().add(data_start), data.len())
        };
        dst.copy_from_slice(data);
    }
}

/// Build a 65+ byte metadata account data buffer.
fn build_metadata_data(key: u8, update_authority: [u8; 32], mint: [u8; 32]) -> Vec<u8> {
    let mut data = vec![0u8; 128]; // larger than prefix to simulate real account
    data[0] = key;
    data[1..33].copy_from_slice(&update_authority);
    data[33..65].copy_from_slice(&mint);
    data
}

/// Build an 18+ byte master edition account data buffer.
fn build_master_edition_data(key: u8, supply: u64, max_supply: Option<u64>) -> Vec<u8> {
    let mut data = vec![0u8; 32]; // larger than prefix
    data[0] = key;
    data[1..9].copy_from_slice(&supply.to_le_bytes());
    match max_supply {
        Some(v) => {
            data[9] = 1;
            data[10..18].copy_from_slice(&v.to_le_bytes());
        }
        None => {
            data[9] = 0;
            data[10..18].copy_from_slice(&0u64.to_le_bytes());
        }
    }
    data
}

const FAKE_ADDR: [u8; 32] = [1u8; 32];
const FAKE_MINT: [u8; 32] = [2u8; 32];
const FAKE_UA: [u8; 32] = [3u8; 32];

#[test]
fn metadata_prefix_zc_size_65() {
    assert_eq!(size_of::<MetadataPrefixZc>(), 65);
    assert_eq!(std::mem::align_of::<MetadataPrefixZc>(), 1);
}

#[test]
fn master_edition_prefix_zc_size_18() {
    assert_eq!(size_of::<MasterEditionPrefixZc>(), 18);
    assert_eq!(std::mem::align_of::<MasterEditionPrefixZc>(), 1);
}

#[test]
fn metadata_deref_reads_correct_fields() {
    let data = build_metadata_data(4, FAKE_UA, FAKE_MINT);
    let mut buf = AccountBuffer::new(data.len());
    buf.init(
        FAKE_ADDR,
        METADATA_OWNER,
        1_000_000,
        data.len() as u64,
        false,
        false,
    );
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let result = <MetadataAccount as quasar_lang::account_load::AccountLoad>::check(&view);
    assert!(result.is_ok());

    let account = unsafe { MetadataAccount::from_account_view_unchecked(&view) };
    assert_eq!(account.key, 4);
    assert_eq!(account.update_authority.as_ref(), &FAKE_UA);
    assert_eq!(account.mint.as_ref(), &FAKE_MINT);
}

#[test]
fn metadata_wrong_key_byte_rejected() {
    let data = build_metadata_data(6, FAKE_UA, FAKE_MINT); // key=6, not 4
    let mut buf = AccountBuffer::new(data.len());
    buf.init(
        FAKE_ADDR,
        METADATA_OWNER,
        1_000_000,
        data.len() as u64,
        false,
        false,
    );
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let result = <MetadataAccount as quasar_lang::account_load::AccountLoad>::check(&view);
    assert!(result.is_err());
}

#[test]
fn metadata_wrong_owner_rejected() {
    let data = build_metadata_data(4, FAKE_UA, FAKE_MINT);
    let mut buf = AccountBuffer::new(data.len());
    buf.init(
        FAKE_ADDR,
        [0u8; 32],
        1_000_000,
        data.len() as u64,
        false,
        false,
    ); // wrong owner
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let result = <MetadataAccount as quasar_lang::account_load::AccountLoad>::check(&view);
    assert!(matches!(result, Err(ProgramError::IllegalOwner)));
}

#[test]
fn metadata_data_too_small_rejected() {
    let data = vec![4u8; 10]; // only 10 bytes, need 65
    let mut buf = AccountBuffer::new(data.len());
    buf.init(
        FAKE_ADDR,
        METADATA_OWNER,
        1_000_000,
        data.len() as u64,
        false,
        false,
    );
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let result = <MetadataAccount as quasar_lang::account_load::AccountLoad>::check(&view);
    assert!(result.is_err());
}

#[test]
fn master_edition_deref_reads_correct_fields() {
    let data = build_master_edition_data(6, 42, Some(100));
    let mut buf = AccountBuffer::new(data.len());
    buf.init(
        FAKE_ADDR,
        METADATA_OWNER,
        1_000_000,
        data.len() as u64,
        false,
        false,
    );
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let result = <MasterEditionAccount as quasar_lang::account_load::AccountLoad>::check(&view);
    assert!(result.is_ok());

    let account = unsafe { MasterEditionAccount::from_account_view_unchecked(&view) };
    assert_eq!(account.key, 6);
    assert_eq!(account.supply_value(), 42);
    assert_eq!(account.max_supply_value(), Some(100));
}

#[test]
fn master_edition_unlimited_supply() {
    let data = build_master_edition_data(6, 0, None);
    let mut buf = AccountBuffer::new(data.len());
    buf.init(
        FAKE_ADDR,
        METADATA_OWNER,
        1_000_000,
        data.len() as u64,
        false,
        false,
    );
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let result = <MasterEditionAccount as quasar_lang::account_load::AccountLoad>::check(&view);
    assert!(result.is_ok());

    let account = unsafe { MasterEditionAccount::from_account_view_unchecked(&view) };
    assert_eq!(account.supply_value(), 0);
    assert_eq!(account.max_supply_value(), None);
}

#[test]
fn master_edition_wrong_key_byte_rejected() {
    let data = build_master_edition_data(4, 0, None); // key=4, not 6
    let mut buf = AccountBuffer::new(data.len());
    buf.init(
        FAKE_ADDR,
        METADATA_OWNER,
        1_000_000,
        data.len() as u64,
        false,
        false,
    );
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let result = <MasterEditionAccount as quasar_lang::account_load::AccountLoad>::check(&view);
    assert!(result.is_err());
}

#[test]
fn master_edition_data_too_small_rejected() {
    let data = vec![6u8; 10]; // only 10 bytes, need 18
    let mut buf = AccountBuffer::new(data.len());
    buf.init(
        FAKE_ADDR,
        METADATA_OWNER,
        1_000_000,
        data.len() as u64,
        false,
        false,
    );
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let result = <MasterEditionAccount as quasar_lang::account_load::AccountLoad>::check(&view);
    assert!(result.is_err());
}

#[test]
fn master_edition_behavior_validation_rejects_invalid_max_supply_tag_after_fast_load() {
    let mut data = build_master_edition_data(6, 42, None);
    data[9] = 2; // invalid Borsh Option tag for max_supply
    let mut buf = AccountBuffer::new(data.len());
    buf.init(
        FAKE_ADDR,
        METADATA_OWNER,
        1_000_000,
        data.len() as u64,
        false,
        false,
    );
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    assert!(
        unsafe { <Account<MasterEditionAccount> as AccountLoad>::load_intrinsic(&view) }.is_ok()
    );
    assert_eq!(
        validate_master_edition_account(&view, None),
        Err(ProgramError::InvalidAccountData)
    );
}

#[test]
fn metadata_all_zeros_rejected() {
    let data = vec![0u8; 128];
    let mut buf = AccountBuffer::new(data.len());
    buf.init(
        FAKE_ADDR,
        METADATA_OWNER,
        1_000_000,
        data.len() as u64,
        false,
        false,
    );
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    // key=0, should be rejected (not 4)
    let result = <MetadataAccount as quasar_lang::account_load::AccountLoad>::check(&view);
    assert!(result.is_err());
}

#[test]
fn metadata_all_ff_rejected() {
    let data = vec![0xFFu8; 128];
    let mut buf = AccountBuffer::new(data.len());
    buf.init(
        FAKE_ADDR,
        METADATA_OWNER,
        1_000_000,
        data.len() as u64,
        false,
        false,
    );
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    // key=0xFF, should be rejected (not 4)
    let result = <MetadataAccount as quasar_lang::account_load::AccountLoad>::check(&view);
    assert!(result.is_err());
}

// PDA derivation and verification tests belong in SVM integration tests
// because `try_find_program_address` and `find_bump_for_address`
// use Solana syscalls that are only available in the runtime.
