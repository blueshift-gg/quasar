use quasar_core::prelude::*;
use solana_address::Address;

use crate::metadata::constants::METADATA_PROGRAM_ID;

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
/// Fields after the prefix (name, symbol, uri, creators, etc.) are variable-length
/// Borsh-serialized data and require offset walking to access.
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

/// Zero-copy layout for the fixed-size prefix of Metaplex MasterEdition accounts.
///
/// - `key` (1 byte): Metaplex account type discriminant (`Key::MasterEditionV2 = 6`)
/// - `supply` (8 bytes, u64 LE): number of editions printed
/// - `max_supply_flag` (1 byte): `Option<u64>` tag — 0 = None (unlimited), 1 = Some
/// - `max_supply` (8 bytes, u64 LE): maximum editions (valid only when flag == 1)
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
        if !quasar_core::keys_eq(unsafe { view.owner() }, &METADATA_PROGRAM_ID) {
            return Err(ProgramError::IllegalOwner);
        }
        Ok(())
    }
}

impl ZeroCopyDeref for MetadataAccount {
    type Target = MetadataPrefix;

    #[inline(always)]
    fn deref_from(view: &AccountView) -> &Self::Target {
        unsafe { &*(view.data_ptr() as *const MetadataPrefix) }
    }

    #[inline(always)]
    fn deref_from_mut(view: &AccountView) -> &mut Self::Target {
        unsafe { &mut *(view.data_ptr() as *mut MetadataPrefix) }
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
/// - First byte (`Key`) is `MasterEditionV2` (6), rejecting uninitialized accounts
///
/// Use as `Account<MasterEditionAccount>` for reading existing master editions.
pub struct MasterEditionAccount;

impl AccountCheck for MasterEditionAccount {
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
        if !quasar_core::keys_eq(unsafe { view.owner() }, &METADATA_PROGRAM_ID) {
            return Err(ProgramError::IllegalOwner);
        }
        Ok(())
    }
}

impl ZeroCopyDeref for MasterEditionAccount {
    type Target = MasterEditionPrefix;

    #[inline(always)]
    fn deref_from(view: &AccountView) -> &Self::Target {
        unsafe { &*(view.data_ptr() as *const MasterEditionPrefix) }
    }

    #[inline(always)]
    fn deref_from_mut(view: &AccountView) -> &mut Self::Target {
        unsafe { &mut *(view.data_ptr() as *mut MasterEditionPrefix) }
    }
}
