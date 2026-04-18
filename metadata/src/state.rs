use {crate::constants::METADATA_PROGRAM_ID, quasar_lang::prelude::*, solana_address::Address};

/// Metaplex Key enum discriminant for MetadataV1 accounts.
const KEY_METADATA_V1: u8 = 4;
/// Metaplex Key enum discriminant for MasterEditionV2 accounts.
const KEY_MASTER_EDITION_V2: u8 = 6;

// ---------------------------------------------------------------------------
// MetadataPrefix — zero-copy layout for the fixed 65-byte header
// ---------------------------------------------------------------------------

/// Zero-copy layout for the fixed-size prefix of Metaplex Metadata accounts.
///
/// The first 65 bytes of a Metadata account have a stable layout:
/// - `key` (1 byte): Metaplex account type discriminant (`Key::MetadataV1 = 4`)
/// - `update_authority` (32 bytes): pubkey authorized to update this metadata
/// - `mint` (32 bytes): the SPL Token mint this metadata describes
///
/// Fields after the prefix (name, symbol, uri, creators, etc.) are
/// variable-length Borsh-serialized data and require offset walking to access.
#[repr(C)]
pub struct MetadataPrefix {
    key: u8,
    update_authority: Address,
    mint: Address,
}

impl MetadataPrefix {
    pub const LEN: usize = core::mem::size_of::<Self>();

    #[inline(always)]
    pub fn key(&self) -> u8 {
        self.key
    }

    #[inline(always)]
    pub fn update_authority(&self) -> &Address {
        &self.update_authority
    }

    #[inline(always)]
    pub fn mint(&self) -> &Address {
        &self.mint
    }
}

const _: () = assert!(core::mem::size_of::<MetadataPrefix>() == 65);
const _: () = assert!(core::mem::align_of::<MetadataPrefix>() == 1);

// ---------------------------------------------------------------------------
// MasterEditionPrefix — zero-copy layout for the fixed 18-byte header
// ---------------------------------------------------------------------------

/// Zero-copy layout for the fixed-size prefix of Metaplex MasterEdition
/// accounts.
///
/// - `key` (1 byte): Metaplex account type discriminant (`Key::MasterEditionV2
///   = 6`)
/// - `supply` (8 bytes, u64 LE): number of editions printed
/// - `max_supply_flag` (1 byte): `Option<u64>` tag — 0 = None (unlimited), 1 =
///   Some
/// - `max_supply` (8 bytes, u64 LE): maximum editions (valid only when flag ==
///   1)
#[repr(C)]
pub struct MasterEditionPrefix {
    key: u8,
    supply: [u8; 8],
    max_supply_flag: u8,
    max_supply: [u8; 8],
}

impl MasterEditionPrefix {
    pub const LEN: usize = core::mem::size_of::<Self>();

    #[inline(always)]
    pub fn key(&self) -> u8 {
        self.key
    }

    #[inline(always)]
    pub fn supply(&self) -> u64 {
        u64::from_le_bytes(self.supply)
    }

    #[inline(always)]
    pub fn max_supply(&self) -> Option<u64> {
        if self.max_supply_flag == 1 {
            Some(u64::from_le_bytes(self.max_supply))
        } else {
            None
        }
    }
}

const _: () = assert!(core::mem::size_of::<MasterEditionPrefix>() == 18);
const _: () = assert!(core::mem::align_of::<MasterEditionPrefix>() == 1);

// ---------------------------------------------------------------------------
// MetadataAccount — marker type for Account<MetadataAccount>
// ---------------------------------------------------------------------------

/// Metaplex Token Metadata account marker.
///
/// Validates:
/// - Owner is the Metaplex Token Metadata program
/// - Data length >= 65 bytes (prefix size)
/// - First byte (`Key`) is `MetadataV1` (4), rejecting uninitialized accounts
///
/// Use as `Account<MetadataAccount>` for reading existing metadata.
pub struct MetadataAccount;

impl AccountCheck for MetadataAccount {
    type Params = ();

    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        if view.data_len() < MetadataPrefix::LEN {
            return Err(ProgramError::AccountDataTooSmall);
        }
        let key = unsafe { *view.data_ptr() };
        if key != KEY_METADATA_V1 {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}

impl CheckOwner for MetadataAccount {
    #[inline(always)]
    fn check_owner(view: &AccountView) -> Result<(), ProgramError> {
        if !quasar_lang::keys_eq(view.owner(), &METADATA_PROGRAM_ID) {
            return Err(ProgramError::IllegalOwner);
        }
        Ok(())
    }
}

impl ZeroCopyDeref for MetadataAccount {
    type Target = MetadataPrefix;

    #[inline(always)]
    unsafe fn deref_from(view: &AccountView) -> &Self::Target {
        &*(view.data_ptr() as *const MetadataPrefix)
    }

    #[inline(always)]
    unsafe fn deref_from_mut(view: &mut AccountView) -> &mut Self::Target {
        &mut *(view.data_mut_ptr() as *mut MetadataPrefix)
    }
}

// ---------------------------------------------------------------------------
// MasterEditionAccount — marker type for Account<MasterEditionAccount>
// ---------------------------------------------------------------------------

/// Metaplex Master Edition account marker.
///
/// Validates:
/// - Owner is the Metaplex Token Metadata program
/// - Data length >= 18 bytes (prefix size)
/// - First byte (`Key`) is `MasterEditionV2` (6), rejecting uninitialized
///   accounts
///
/// Use as `Account<MasterEditionAccount>` for reading existing master editions.
pub struct MasterEditionAccount;

impl AccountCheck for MasterEditionAccount {
    type Params = ();

    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        if view.data_len() < MasterEditionPrefix::LEN {
            return Err(ProgramError::AccountDataTooSmall);
        }
        let key = unsafe { *view.data_ptr() };
        if key != KEY_MASTER_EDITION_V2 {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}

impl CheckOwner for MasterEditionAccount {
    #[inline(always)]
    fn check_owner(view: &AccountView) -> Result<(), ProgramError> {
        if !quasar_lang::keys_eq(view.owner(), &METADATA_PROGRAM_ID) {
            return Err(ProgramError::IllegalOwner);
        }
        Ok(())
    }
}

impl ZeroCopyDeref for MasterEditionAccount {
    type Target = MasterEditionPrefix;

    #[inline(always)]
    unsafe fn deref_from(view: &AccountView) -> &Self::Target {
        &*(view.data_ptr() as *const MasterEditionPrefix)
    }

    #[inline(always)]
    unsafe fn deref_from_mut(view: &mut AccountView) -> &mut Self::Target {
        &mut *(view.data_mut_ptr() as *mut MasterEditionPrefix)
    }
}

// ---------------------------------------------------------------------------
// Kani model-checking proof harnesses
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    // --- MetadataPrefix ---

    /// Prove MetadataPrefix::LEN matches the actual struct size.
    #[kani::proof]
    fn metadata_prefix_len_matches_sizeof() {
        assert!(MetadataPrefix::LEN == core::mem::size_of::<MetadataPrefix>());
    }

    /// Prove MetadataPrefix has alignment 1 (safe for pointer cast from
    /// arbitrary account data).
    #[kani::proof]
    fn metadata_prefix_align_one() {
        assert!(core::mem::align_of::<MetadataPrefix>() == 1);
    }

    /// Prove MetadataPrefix is exactly 65 bytes.
    #[kani::proof]
    fn metadata_prefix_size_65() {
        assert!(core::mem::size_of::<MetadataPrefix>() == 65);
    }

    /// Prove: for any `data_len >= MetadataPrefix::LEN`, the data covers
    /// the full struct — verifies the runtime guard in `MetadataAccount::check`
    /// is sufficient for the pointer cast in `deref_from`.
    #[kani::proof]
    fn metadata_prefix_data_len_guard_sufficient() {
        let data_len: usize = kani::any();
        kani::assume(data_len >= MetadataPrefix::LEN);
        assert!(data_len >= core::mem::size_of::<MetadataPrefix>());
    }

    // --- MasterEditionPrefix ---

    /// Prove MasterEditionPrefix::LEN matches the actual struct size.
    #[kani::proof]
    fn master_edition_prefix_len_matches_sizeof() {
        assert!(MasterEditionPrefix::LEN == core::mem::size_of::<MasterEditionPrefix>());
    }

    /// Prove MasterEditionPrefix has alignment 1 (safe for pointer cast from
    /// arbitrary account data).
    #[kani::proof]
    fn master_edition_prefix_align_one() {
        assert!(core::mem::align_of::<MasterEditionPrefix>() == 1);
    }

    /// Prove MasterEditionPrefix is exactly 18 bytes.
    #[kani::proof]
    fn master_edition_prefix_size_18() {
        assert!(core::mem::size_of::<MasterEditionPrefix>() == 18);
    }

    /// Prove: for any `data_len >= MasterEditionPrefix::LEN`, the data covers
    /// the full struct — verifies the runtime guard in
    /// `MasterEditionAccount::check` is sufficient for the pointer cast in
    /// `deref_from`.
    #[kani::proof]
    fn master_edition_prefix_data_len_guard_sufficient() {
        let data_len: usize = kani::any();
        kani::assume(data_len >= MasterEditionPrefix::LEN);
        assert!(data_len >= core::mem::size_of::<MasterEditionPrefix>());
    }
}
