#![cfg(feature = "metadata")]
#![allow(dead_code)]

use std::mem::size_of;

use quasar_core::__internal::{
    AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED,
};
use quasar_core::cpi::CpiCall;
use quasar_spl::metadata::MetadataCpi;
use solana_address::Address;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

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
            (*raw).resize_delta = 0;
            (*raw).address = Address::new_from_array(address);
            (*raw).owner = Address::new_from_array(owner);
            (*raw).lamports = lamports;
            (*raw).data_len = data_len;
        }
    }

    unsafe fn view(&mut self) -> AccountView {
        AccountView::new_unchecked(self.raw())
    }
}

fn make_create_metadata_accounts() -> (
    AccountBuffer,
    AccountBuffer,
    AccountBuffer,
    AccountBuffer,
    AccountBuffer,
    AccountBuffer,
    AccountBuffer,
) {
    let mut program = AccountBuffer::new(0);
    program.init([100u8; 32], [0u8; 32], 0, 0, false, false);
    let mut metadata = AccountBuffer::new(0);
    metadata.init([1u8; 32], [0u8; 32], 0, 0, false, true);
    let mut mint = AccountBuffer::new(0);
    mint.init([2u8; 32], [0u8; 32], 0, 0, false, false);
    let mut mint_authority = AccountBuffer::new(0);
    mint_authority.init([3u8; 32], [0u8; 32], 0, 0, true, false);
    let mut payer = AccountBuffer::new(0);
    payer.init([4u8; 32], [0u8; 32], 1_000_000, 0, true, true);
    let mut update_authority = AccountBuffer::new(0);
    update_authority.init([5u8; 32], [0u8; 32], 0, 0, false, false);
    let mut system_program = AccountBuffer::new(0);
    system_program.init([0u8; 32], [0u8; 32], 0, 0, false, false);
    (
        program,
        metadata,
        mint,
        mint_authority,
        payer,
        update_authority,
        system_program,
    )
}

/// Read a u32 LE from a byte slice at the given offset.
fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}

/// Read a Borsh string (u32 LE length + bytes) from data at offset.
/// Returns (string_bytes, new_offset).
fn read_borsh_string(data: &[u8], offset: usize) -> (&[u8], usize) {
    let len = read_u32_le(data, offset) as usize;
    let start = offset + 4;
    (&data[start..start + len], start + len)
}

/// Read a u16 LE from a byte slice at the given offset.
fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap())
}

// ===========================================================================
// create_metadata_accounts_v3 — byte-level serialization verification
// ===========================================================================

#[test]
fn create_metadata_v3_serializes_correct_borsh_layout() {
    let (mut prog, mut meta, mut mint, mut mint_auth, mut payer, mut upd, mut sys) =
        make_create_metadata_accounts();
    let prog_v = unsafe { prog.view() };
    let meta_v = unsafe { meta.view() };
    let mint_v = unsafe { mint.view() };
    let mint_auth_v = unsafe { mint_auth.view() };
    let payer_v = unsafe { payer.view() };
    let upd_v = unsafe { upd.view() };
    let sys_v = unsafe { sys.view() };

    let cpi = prog_v.create_metadata_accounts_v3(
        &meta_v,
        &mint_v,
        &mint_auth_v,
        &payer_v,
        &upd_v,
        &sys_v,
        b"My NFT",
        b"MNFT",
        b"https://example.com/nft.json",
        500,
        true,
        true,
    );

    let data = cpi.instruction_data();

    // Discriminator: CreateMetadataAccountsV3 = 33
    let mut off = 0;
    assert_eq!(data[off], 33, "wrong discriminator");
    off += 1;

    // DataV2.name: Borsh string
    let (name_bytes, next) = read_borsh_string(data, off);
    assert_eq!(name_bytes, b"My NFT", "name mismatch");
    off = next;

    // DataV2.symbol: Borsh string
    let (symbol_bytes, next) = read_borsh_string(data, off);
    assert_eq!(symbol_bytes, b"MNFT", "symbol mismatch");
    off = next;

    // DataV2.uri: Borsh string
    let (uri_bytes, next) = read_borsh_string(data, off);
    assert_eq!(uri_bytes, b"https://example.com/nft.json", "uri mismatch");
    off = next;

    // DataV2.seller_fee_basis_points: u16 LE
    assert_eq!(read_u16_le(data, off), 500, "seller_fee mismatch");
    off += 2;

    // DataV2.creators: Option<Vec<Creator>> = None
    assert_eq!(data[off], 0, "creators should be None");
    off += 1;

    // DataV2.collection: Option<Collection> = None
    assert_eq!(data[off], 0, "collection should be None");
    off += 1;

    // DataV2.uses: Option<Uses> = None
    assert_eq!(data[off], 0, "uses should be None");
    off += 1;

    // is_mutable: bool
    assert_eq!(data[off], 1, "is_mutable should be true");
    off += 1;

    // update_authority_is_signer: bool
    assert_eq!(data[off], 1, "update_authority_is_signer should be true");
    off += 1;

    // collection_details: Option<CollectionDetails> = None
    assert_eq!(data[off], 0, "collection_details should be None");
    off += 1;

    // data_len should exactly match offset
    assert_eq!(
        cpi.instruction_data_len(),
        off,
        "data_len doesn't match serialized offset"
    );
}

#[test]
fn create_metadata_v3_data_len_matches_content() {
    let (mut prog, mut meta, mut mint, mut mint_auth, mut payer, mut upd, mut sys) =
        make_create_metadata_accounts();
    let prog_v = unsafe { prog.view() };
    let meta_v = unsafe { meta.view() };
    let mint_v = unsafe { mint.view() };
    let mint_auth_v = unsafe { mint_auth.view() };
    let payer_v = unsafe { payer.view() };
    let upd_v = unsafe { upd.view() };
    let sys_v = unsafe { sys.view() };

    // Expected: disc(1) + name(4+6) + symbol(4+3) + uri(4+15) + fee(2) + 3*None(3) + mutable(1) + signer(1) + details(1) = 45
    let cpi = prog_v.create_metadata_accounts_v3(
        &meta_v,
        &mint_v,
        &mint_auth_v,
        &payer_v,
        &upd_v,
        &sys_v,
        b"My NFT",     // 6 bytes
        b"TST",        // 3 bytes
        b"https://x.io/y", // 14 bytes
        0,
        false,
        true,
    );

    let expected_len = 1 + (4 + 6) + (4 + 3) + (4 + 14) + 2 + 3 + 1 + 1 + 1;
    assert_eq!(cpi.instruction_data_len(), expected_len);
}

#[test]
fn create_metadata_v3_empty_strings_produce_zero_length_borsh() {
    let (mut prog, mut meta, mut mint, mut mint_auth, mut payer, mut upd, mut sys) =
        make_create_metadata_accounts();
    let prog_v = unsafe { prog.view() };
    let meta_v = unsafe { meta.view() };
    let mint_v = unsafe { mint.view() };
    let mint_auth_v = unsafe { mint_auth.view() };
    let payer_v = unsafe { payer.view() };
    let upd_v = unsafe { upd.view() };
    let sys_v = unsafe { sys.view() };

    let cpi = prog_v.create_metadata_accounts_v3(
        &meta_v, &mint_v, &mint_auth_v, &payer_v, &upd_v, &sys_v,
        b"", b"", b"", 0, false, false,
    );

    let data = cpi.instruction_data();

    // disc(1) + 3 empty strings (4+0 each = 12) + fee(2) + 3 None(3) + mutable(1) + signer(1) + details(1) = 21
    assert_eq!(cpi.instruction_data_len(), 21);

    // Each string should have length prefix 0
    assert_eq!(read_u32_le(data, 1), 0, "name length should be 0");
    assert_eq!(read_u32_le(data, 5), 0, "symbol length should be 0");
    assert_eq!(read_u32_le(data, 9), 0, "uri length should be 0");
}

#[test]
fn create_metadata_v3_max_lengths_data_len_within_buffer() {
    let (mut prog, mut meta, mut mint, mut mint_auth, mut payer, mut upd, mut sys) =
        make_create_metadata_accounts();
    let prog_v = unsafe { prog.view() };
    let meta_v = unsafe { meta.view() };
    let mint_v = unsafe { mint.view() };
    let mint_auth_v = unsafe { mint_auth.view() };
    let payer_v = unsafe { payer.view() };
    let upd_v = unsafe { upd.view() };
    let sys_v = unsafe { sys.view() };

    let cpi = prog_v.create_metadata_accounts_v3(
        &meta_v, &mint_v, &mint_auth_v, &payer_v, &upd_v, &sys_v,
        &[b'A'; 32], &[b'B'; 10], &[b'C'; 200],
        u16::MAX, true, true,
    );

    // Max: disc(1) + (4+32) + (4+10) + (4+200) + 2 + 3 + 1 + 1 + 1 = 263
    assert_eq!(cpi.instruction_data_len(), 263);
    assert!(cpi.instruction_data_len() <= 512, "exceeds BufCpiCall<512> capacity");

    // Verify seller_fee at max
    let data = cpi.instruction_data();
    // offset after disc + name + symbol + uri = 1 + 36 + 14 + 204 = 255
    assert_eq!(read_u16_le(data, 255), u16::MAX);
}

#[test]
fn create_metadata_v3_is_mutable_false() {
    let (mut prog, mut meta, mut mint, mut mint_auth, mut payer, mut upd, mut sys) =
        make_create_metadata_accounts();
    let prog_v = unsafe { prog.view() };
    let meta_v = unsafe { meta.view() };
    let mint_v = unsafe { mint.view() };
    let mint_auth_v = unsafe { mint_auth.view() };
    let payer_v = unsafe { payer.view() };
    let upd_v = unsafe { upd.view() };
    let sys_v = unsafe { sys.view() };

    let cpi = prog_v.create_metadata_accounts_v3(
        &meta_v, &mint_v, &mint_auth_v, &payer_v, &upd_v, &sys_v,
        b"X", b"Y", b"Z", 0, false, false,
    );

    let data = cpi.instruction_data();
    let len = cpi.instruction_data_len();

    // Last 3 bytes: is_mutable, update_authority_is_signer, collection_details
    assert_eq!(data[len - 3], 0, "is_mutable should be false (0)");
    assert_eq!(data[len - 2], 0, "update_authority_is_signer should be false (0)");
    assert_eq!(data[len - 1], 0, "collection_details should be None (0)");
}

// Bounds check panics

#[test]
#[should_panic(expected = "metadata field lengths exceed Metaplex limits")]
fn create_metadata_v3_name_too_long() {
    let (mut prog, mut meta, mut mint, mut mint_auth, mut payer, mut upd, mut sys) =
        make_create_metadata_accounts();
    let prog_v = unsafe { prog.view() };
    let meta_v = unsafe { meta.view() };
    let mint_v = unsafe { mint.view() };
    let mint_auth_v = unsafe { mint_auth.view() };
    let payer_v = unsafe { payer.view() };
    let upd_v = unsafe { upd.view() };
    let sys_v = unsafe { sys.view() };

    let _ = prog_v.create_metadata_accounts_v3(
        &meta_v, &mint_v, &mint_auth_v, &payer_v, &upd_v, &sys_v,
        &[b'X'; 33], b"OK", b"https://ok.io", 0, false, true,
    );
}

#[test]
#[should_panic(expected = "metadata field lengths exceed Metaplex limits")]
fn create_metadata_v3_symbol_too_long() {
    let (mut prog, mut meta, mut mint, mut mint_auth, mut payer, mut upd, mut sys) =
        make_create_metadata_accounts();
    let prog_v = unsafe { prog.view() };
    let meta_v = unsafe { meta.view() };
    let mint_v = unsafe { mint.view() };
    let mint_auth_v = unsafe { mint_auth.view() };
    let payer_v = unsafe { payer.view() };
    let upd_v = unsafe { upd.view() };
    let sys_v = unsafe { sys.view() };

    let _ = prog_v.create_metadata_accounts_v3(
        &meta_v, &mint_v, &mint_auth_v, &payer_v, &upd_v, &sys_v,
        b"Name", &[b'S'; 11], b"https://ok.io", 0, false, true,
    );
}

#[test]
#[should_panic(expected = "metadata field lengths exceed Metaplex limits")]
fn create_metadata_v3_uri_too_long() {
    let (mut prog, mut meta, mut mint, mut mint_auth, mut payer, mut upd, mut sys) =
        make_create_metadata_accounts();
    let prog_v = unsafe { prog.view() };
    let meta_v = unsafe { meta.view() };
    let mint_v = unsafe { mint.view() };
    let mint_auth_v = unsafe { mint_auth.view() };
    let payer_v = unsafe { payer.view() };
    let upd_v = unsafe { upd.view() };
    let sys_v = unsafe { sys.view() };

    let _ = prog_v.create_metadata_accounts_v3(
        &meta_v, &mint_v, &mint_auth_v, &payer_v, &upd_v, &sys_v,
        b"Name", b"SYM", &[b'U'; 201], 0, false, true,
    );
}

// ===========================================================================
// update_metadata_accounts_v2 — byte-level serialization verification
// ===========================================================================

#[test]
fn update_metadata_v2_with_data_serializes_correct_borsh() {
    let (mut prog, mut meta, _, _, _, mut upd, _) = make_create_metadata_accounts();
    let prog_v = unsafe { prog.view() };
    let meta_v = unsafe { meta.view() };
    let upd_v = unsafe { upd.view() };
    let new_auth = Address::new_from_array([0xBB; 32]);

    let cpi = prog_v.update_metadata_accounts_v2(
        &meta_v,
        &upd_v,
        Some(&new_auth),
        Some(b"Updated"),
        Some(b"UPD"),
        Some(b"https://new.io"),
        Some(750),
        Some(true),
        Some(false),
    );

    let data = cpi.instruction_data();
    let mut off = 0;

    // Discriminator: UpdateMetadataAccountsV2 = 15
    assert_eq!(data[off], 15, "wrong discriminator");
    off += 1;

    // Option<DataV2> = Some
    assert_eq!(data[off], 1, "DataV2 should be Some");
    off += 1;

    // name
    let (name, next) = read_borsh_string(data, off);
    assert_eq!(name, b"Updated");
    off = next;

    // symbol
    let (symbol, next) = read_borsh_string(data, off);
    assert_eq!(symbol, b"UPD");
    off = next;

    // uri
    let (uri, next) = read_borsh_string(data, off);
    assert_eq!(uri, b"https://new.io");
    off = next;

    // seller_fee_basis_points
    assert_eq!(read_u16_le(data, off), 750);
    off += 2;

    // creators=None, collection=None, uses=None
    assert_eq!(data[off], 0); off += 1;
    assert_eq!(data[off], 0); off += 1;
    assert_eq!(data[off], 0); off += 1;

    // new_update_authority: Some(pubkey)
    assert_eq!(data[off], 1, "new_update_authority should be Some");
    off += 1;
    assert_eq!(&data[off..off + 32], &[0xBB; 32], "authority address mismatch");
    off += 32;

    // primary_sale_happened: Some(true)
    assert_eq!(data[off], 1); off += 1;
    assert_eq!(data[off], 1, "primary_sale should be true"); off += 1;

    // is_mutable: Some(false)
    assert_eq!(data[off], 1); off += 1;
    assert_eq!(data[off], 0, "is_mutable should be false"); off += 1;

    assert_eq!(cpi.instruction_data_len(), off);
}

#[test]
fn update_metadata_v2_all_none_serializes_minimal() {
    let (mut prog, mut meta, _, _, _, mut upd, _) = make_create_metadata_accounts();
    let prog_v = unsafe { prog.view() };
    let meta_v = unsafe { meta.view() };
    let upd_v = unsafe { upd.view() };

    let cpi = prog_v.update_metadata_accounts_v2(
        &meta_v, &upd_v, None, None, None, None, None, None, None,
    );

    let data = cpi.instruction_data();

    // disc(1) + DataV2=None(1) + authority=None(1) + primary_sale=None(1) + mutable=None(1) = 5
    assert_eq!(cpi.instruction_data_len(), 5);
    assert_eq!(data[0], 15, "discriminator");
    assert_eq!(data[1], 0, "DataV2 = None");
    assert_eq!(data[2], 0, "new_update_authority = None");
    assert_eq!(data[3], 0, "primary_sale_happened = None");
    assert_eq!(data[4], 0, "is_mutable = None");
}

#[test]
fn update_metadata_v2_partial_data_requires_all_three_strings() {
    // When only name is provided (no symbol/uri), DataV2 should be None
    let (mut prog, mut meta, _, _, _, mut upd, _) = make_create_metadata_accounts();
    let prog_v = unsafe { prog.view() };
    let meta_v = unsafe { meta.view() };
    let upd_v = unsafe { upd.view() };

    let cpi = prog_v.update_metadata_accounts_v2(
        &meta_v, &upd_v, None,
        Some(b"Name"), None, None, // name without symbol/uri
        None, None, None,
    );

    let data = cpi.instruction_data();
    // Should serialize as DataV2=None since not all three strings are provided
    assert_eq!(data[1], 0, "DataV2 should be None when only name is provided");
}

// Bounds check panics

#[test]
#[should_panic(expected = "name length 33 exceeds max 32")]
fn update_metadata_v2_name_too_long() {
    let (mut prog, mut meta, _, _, _, mut upd, _) = make_create_metadata_accounts();
    let prog_v = unsafe { prog.view() };
    let meta_v = unsafe { meta.view() };
    let upd_v = unsafe { upd.view() };

    let _ = prog_v.update_metadata_accounts_v2(
        &meta_v, &upd_v, None,
        Some(&[b'X'; 33]), Some(b"OK"), Some(b"https://ok.io"),
        None, None, None,
    );
}

#[test]
#[should_panic(expected = "symbol length 11 exceeds max 10")]
fn update_metadata_v2_symbol_too_long() {
    let (mut prog, mut meta, _, _, _, mut upd, _) = make_create_metadata_accounts();
    let prog_v = unsafe { prog.view() };
    let meta_v = unsafe { meta.view() };
    let upd_v = unsafe { upd.view() };

    let _ = prog_v.update_metadata_accounts_v2(
        &meta_v, &upd_v, None,
        Some(b"OK"), Some(&[b'S'; 11]), Some(b"https://ok.io"),
        None, None, None,
    );
}

#[test]
#[should_panic(expected = "uri length 201 exceeds max 200")]
fn update_metadata_v2_uri_too_long() {
    let (mut prog, mut meta, _, _, _, mut upd, _) = make_create_metadata_accounts();
    let prog_v = unsafe { prog.view() };
    let meta_v = unsafe { meta.view() };
    let upd_v = unsafe { upd.view() };

    let _ = prog_v.update_metadata_accounts_v2(
        &meta_v, &upd_v, None,
        Some(b"OK"), Some(b"OK"), Some(&[b'U'; 201]),
        None, None, None,
    );
}

// ===========================================================================
// create_master_edition_v3 — byte-level verification
// ===========================================================================

#[test]
fn create_master_edition_v3_some_supply_serializes_correctly() {
    let (mut prog, mut meta, mut mint, _, mut payer, mut upd, mut sys) =
        make_create_metadata_accounts();
    let mut edition = AccountBuffer::new(0);
    edition.init([10u8; 32], [0u8; 32], 0, 0, false, true);
    let mut mint_authority = AccountBuffer::new(0);
    mint_authority.init([11u8; 32], [0u8; 32], 0, 0, true, false);
    let mut token_program = AccountBuffer::new(0);
    token_program.init([12u8; 32], [0u8; 32], 0, 0, false, false);

    let prog_v = unsafe { prog.view() };
    let edition_v = unsafe { edition.view() };
    let mint_v = unsafe { mint.view() };
    let upd_v = unsafe { upd.view() };
    let mint_auth_v = unsafe { mint_authority.view() };
    let payer_v = unsafe { payer.view() };
    let meta_v = unsafe { meta.view() };
    let token_v = unsafe { token_program.view() };
    let sys_v = unsafe { sys.view() };

    let cpi: CpiCall<'_, 9, 10> = prog_v.create_master_edition_v3(
        &edition_v, &mint_v, &upd_v, &mint_auth_v, &payer_v, &meta_v,
        &token_v, &sys_v, Some(42),
    );

    let data = cpi.instruction_data();
    assert_eq!(data.len(), 10, "CpiCall<9, 10> should have exactly 10 bytes");

    // Discriminator: CreateMasterEditionV3 = 17
    assert_eq!(data[0], 17, "wrong discriminator");

    // Option<u64>: Some tag
    assert_eq!(data[1], 1, "max_supply should be Some");

    // Value: 42 as u64 LE
    let value = u64::from_le_bytes(data[2..10].try_into().unwrap());
    assert_eq!(value, 42, "max_supply value mismatch");
}

#[test]
fn create_master_edition_v3_none_supply_serializes_correctly() {
    let (mut prog, mut meta, mut mint, _, mut payer, mut upd, mut sys) =
        make_create_metadata_accounts();
    let mut edition = AccountBuffer::new(0);
    edition.init([10u8; 32], [0u8; 32], 0, 0, false, true);
    let mut mint_authority = AccountBuffer::new(0);
    mint_authority.init([11u8; 32], [0u8; 32], 0, 0, true, false);
    let mut token_program = AccountBuffer::new(0);
    token_program.init([12u8; 32], [0u8; 32], 0, 0, false, false);

    let prog_v = unsafe { prog.view() };
    let edition_v = unsafe { edition.view() };
    let mint_v = unsafe { mint.view() };
    let upd_v = unsafe { upd.view() };
    let mint_auth_v = unsafe { mint_authority.view() };
    let payer_v = unsafe { payer.view() };
    let meta_v = unsafe { meta.view() };
    let token_v = unsafe { token_program.view() };
    let sys_v = unsafe { sys.view() };

    let cpi = prog_v.create_master_edition_v3(
        &edition_v, &mint_v, &upd_v, &mint_auth_v, &payer_v, &meta_v,
        &token_v, &sys_v, None,
    );

    let data = cpi.instruction_data();
    assert_eq!(data[0], 17, "discriminator");
    assert_eq!(data[1], 0, "max_supply should be None");
    // Remaining 8 bytes should be zeroed
    assert_eq!(&data[2..10], &[0u8; 8], "None padding should be zero");
}

#[test]
fn create_master_edition_v3_max_supply_u64_max() {
    let (mut prog, mut meta, mut mint, _, mut payer, mut upd, mut sys) =
        make_create_metadata_accounts();
    let mut edition = AccountBuffer::new(0);
    edition.init([10u8; 32], [0u8; 32], 0, 0, false, true);
    let mut mint_authority = AccountBuffer::new(0);
    mint_authority.init([11u8; 32], [0u8; 32], 0, 0, true, false);
    let mut token_program = AccountBuffer::new(0);
    token_program.init([12u8; 32], [0u8; 32], 0, 0, false, false);

    let prog_v = unsafe { prog.view() };
    let edition_v = unsafe { edition.view() };
    let mint_v = unsafe { mint.view() };
    let upd_v = unsafe { upd.view() };
    let mint_auth_v = unsafe { mint_authority.view() };
    let payer_v = unsafe { payer.view() };
    let meta_v = unsafe { meta.view() };
    let token_v = unsafe { token_program.view() };
    let sys_v = unsafe { sys.view() };

    let cpi = prog_v.create_master_edition_v3(
        &edition_v, &mint_v, &upd_v, &mint_auth_v, &payer_v, &meta_v,
        &token_v, &sys_v, Some(u64::MAX),
    );

    let data = cpi.instruction_data();
    assert_eq!(data[1], 1);
    let value = u64::from_le_bytes(data[2..10].try_into().unwrap());
    assert_eq!(value, u64::MAX);
}

// ===========================================================================
// Fixed-length CPI instructions — discriminator verification
// ===========================================================================

#[test]
fn sign_metadata_has_correct_discriminator() {
    let mut prog = AccountBuffer::new(0);
    prog.init([100u8; 32], [0u8; 32], 0, 0, false, false);
    let mut creator = AccountBuffer::new(0);
    creator.init([1u8; 32], [0u8; 32], 0, 0, true, false);
    let mut meta = AccountBuffer::new(0);
    meta.init([2u8; 32], [0u8; 32], 0, 0, false, true);

    let prog_v = unsafe { prog.view() };
    let creator_v = unsafe { creator.view() };
    let meta_v = unsafe { meta.view() };

    let cpi = prog_v.sign_metadata(&creator_v, &meta_v);
    let data = cpi.instruction_data();
    assert_eq!(data.len(), 1);
    assert_eq!(data[0], 7, "SignMetadata discriminator should be 7");
}

#[test]
fn remove_creator_verification_has_correct_discriminator() {
    let mut prog = AccountBuffer::new(0);
    prog.init([100u8; 32], [0u8; 32], 0, 0, false, false);
    let mut creator = AccountBuffer::new(0);
    creator.init([1u8; 32], [0u8; 32], 0, 0, true, false);
    let mut meta = AccountBuffer::new(0);
    meta.init([2u8; 32], [0u8; 32], 0, 0, false, true);

    let prog_v = unsafe { prog.view() };
    let creator_v = unsafe { creator.view() };
    let meta_v = unsafe { meta.view() };

    let cpi = prog_v.remove_creator_verification(&creator_v, &meta_v);
    assert_eq!(cpi.instruction_data()[0], 28, "RemoveCreatorVerification = 28");
}

#[test]
fn update_primary_sale_has_correct_discriminator() {
    let mut prog = AccountBuffer::new(0);
    prog.init([100u8; 32], [0u8; 32], 0, 0, false, false);
    let mut meta = AccountBuffer::new(0);
    meta.init([1u8; 32], [0u8; 32], 0, 0, false, true);
    let mut owner = AccountBuffer::new(0);
    owner.init([2u8; 32], [0u8; 32], 0, 0, true, false);
    let mut token = AccountBuffer::new(0);
    token.init([3u8; 32], [0u8; 32], 0, 0, false, false);

    let prog_v = unsafe { prog.view() };
    let meta_v = unsafe { meta.view() };
    let owner_v = unsafe { owner.view() };
    let token_v = unsafe { token.view() };

    let cpi = prog_v.update_primary_sale_happened_via_token(&meta_v, &owner_v, &token_v);
    assert_eq!(cpi.instruction_data()[0], 4, "UpdatePrimarySaleHappenedViaToken = 4");
}

#[test]
fn verify_collection_has_correct_discriminator() {
    let mut prog = AccountBuffer::new(0);
    prog.init([100u8; 32], [0u8; 32], 0, 0, false, false);
    let mut bufs: Vec<AccountBuffer> = (0..6).map(|i| {
        let mut b = AccountBuffer::new(0);
        b.init([i as u8 + 1; 32], [0u8; 32], 0, 0, matches!(i, 1 | 2), matches!(i, 0 | 2));
        b
    }).collect();

    let prog_v = unsafe { prog.view() };
    let views: Vec<AccountView> = bufs.iter_mut().map(|b| unsafe { b.view() }).collect();

    let cpi = prog_v.verify_collection(&views[0], &views[1], &views[2], &views[3], &views[4], &views[5]);
    assert_eq!(cpi.instruction_data()[0], 18, "VerifyCollection = 18");
}

#[test]
fn verify_sized_collection_item_has_correct_discriminator() {
    let mut prog = AccountBuffer::new(0);
    prog.init([100u8; 32], [0u8; 32], 0, 0, false, false);
    let mut bufs: Vec<AccountBuffer> = (0..6).map(|i| {
        let mut b = AccountBuffer::new(0);
        b.init([i as u8 + 1; 32], [0u8; 32], 0, 0, false, false);
        b
    }).collect();

    let prog_v = unsafe { prog.view() };
    let views: Vec<AccountView> = bufs.iter_mut().map(|b| unsafe { b.view() }).collect();

    let cpi = prog_v.verify_sized_collection_item(&views[0], &views[1], &views[2], &views[3], &views[4], &views[5]);
    assert_eq!(cpi.instruction_data()[0], 30, "VerifySizedCollectionItem = 30");
}

#[test]
fn unverify_collection_has_correct_discriminator() {
    let mut prog = AccountBuffer::new(0);
    prog.init([100u8; 32], [0u8; 32], 0, 0, false, false);
    let mut bufs: Vec<AccountBuffer> = (0..5).map(|i| {
        let mut b = AccountBuffer::new(0);
        b.init([i as u8 + 1; 32], [0u8; 32], 0, 0, false, false);
        b
    }).collect();

    let prog_v = unsafe { prog.view() };
    let views: Vec<AccountView> = bufs.iter_mut().map(|b| unsafe { b.view() }).collect();

    let cpi = prog_v.unverify_collection(&views[0], &views[1], &views[2], &views[3], &views[4]);
    assert_eq!(cpi.instruction_data()[0], 22, "UnverifyCollection = 22");
}

#[test]
fn mint_new_edition_serializes_edition_number() {
    let mut bufs: Vec<AccountBuffer> = (0..14).map(|i| {
        let mut b = AccountBuffer::new(0);
        b.init([i as u8 + 1; 32], [0u8; 32], 0, 0, false, false);
        b
    }).collect();

    let mut prog = AccountBuffer::new(0);
    prog.init([100u8; 32], [0u8; 32], 0, 0, false, false);

    let prog_v = unsafe { prog.view() };
    let views: Vec<AccountView> = bufs.iter_mut().map(|b| unsafe { b.view() }).collect();

    let cpi = prog_v.mint_new_edition_from_master_edition_via_token(
        &views[0], &views[1], &views[2], &views[3], &views[4], &views[5],
        &views[6], &views[7], &views[8], &views[9], &views[10], &views[11],
        &views[12], 12345,
    );

    let data = cpi.instruction_data();
    assert_eq!(data.len(), 9);
    assert_eq!(data[0], 11, "MintNewEditionFromMasterEditionViaToken = 11");
    let edition = u64::from_le_bytes(data[1..9].try_into().unwrap());
    assert_eq!(edition, 12345, "edition number mismatch");
}

#[test]
fn mint_new_edition_zero_edition() {
    let mut bufs: Vec<AccountBuffer> = (0..14).map(|i| {
        let mut b = AccountBuffer::new(0);
        b.init([i as u8 + 1; 32], [0u8; 32], 0, 0, false, false);
        b
    }).collect();

    let mut prog = AccountBuffer::new(0);
    prog.init([100u8; 32], [0u8; 32], 0, 0, false, false);

    let prog_v = unsafe { prog.view() };
    let views: Vec<AccountView> = bufs.iter_mut().map(|b| unsafe { b.view() }).collect();

    let cpi = prog_v.mint_new_edition_from_master_edition_via_token(
        &views[0], &views[1], &views[2], &views[3], &views[4], &views[5],
        &views[6], &views[7], &views[8], &views[9], &views[10], &views[11],
        &views[12], 0,
    );

    let data = cpi.instruction_data();
    let edition = u64::from_le_bytes(data[1..9].try_into().unwrap());
    assert_eq!(edition, 0);
}
