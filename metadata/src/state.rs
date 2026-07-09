// The `#[derive(quasar_lang::ZeroPod)]` expansion emits unqualified `zeropod::`
// paths and has no crate-path override, so alias the framework's re-export as
// `zeropod` to resolve them. Everything else uses the stable
// `quasar_lang::{ZeroPod, pod, ...}` paths.
use {
    crate::constants::METADATA_PROGRAM_ID,
    quasar_lang::{__zeropod as zeropod, prelude::*},
    solana_address::Address,
};

/// Metaplex Key enum discriminant for MetadataV1 accounts.
pub(crate) const KEY_METADATA_V1: u8 = 4;
/// Metaplex Key enum discriminant for MasterEditionV2 accounts.
pub(crate) const KEY_MASTER_EDITION_V2: u8 = 6;

/// Zero-copy layout for the fixed-size prefix of Metaplex Metadata accounts.
///
/// The first 65 bytes of a Metadata account have a stable layout:
/// - `key` (1 byte): Metaplex account type discriminant (`Key::MetadataV1 = 4`)
/// - `update_authority` (32 bytes): pubkey authorized to update this metadata
/// - `mint` (32 bytes): the SPL Token mint this metadata describes
///
/// Fields after the prefix (name, symbol, uri, creators, etc.) are
/// variable-length Borsh-serialized data and require offset walking to access.
#[derive(quasar_lang::ZeroPod)]
pub struct MetadataPrefix {
    pub key: u8,
    pub update_authority: Address,
    pub mint: Address,
}

const _: () = assert!(core::mem::size_of::<MetadataPrefixZc>() == 65);
const _: () = assert!(core::mem::align_of::<MetadataPrefixZc>() == 1);
const _: () = assert!(core::mem::offset_of!(MetadataPrefixZc, key) == 0);
const _: () = assert!(core::mem::offset_of!(MetadataPrefixZc, update_authority) == 1);
const _: () = assert!(core::mem::offset_of!(MetadataPrefixZc, mint) == 33);
const _: () = assert!(<MetadataPrefix as quasar_lang::ZeroPodFixed>::SIZE == 65);

/// Zero-copy layout for the fixed-size prefix of Metaplex MasterEdition
/// accounts.
///
/// - `key` (1 byte): Metaplex account type discriminant (`Key::MasterEditionV2
///   = 6`)
/// - `supply` (8 bytes, u64 LE): number of editions printed
/// - `max_supply` (9 bytes): Borsh `Option<u64>` tag + value
#[derive(quasar_lang::ZeroPod)]
pub struct MasterEditionPrefix {
    pub key: u8,
    pub supply: u64,
    pub max_supply: zeropod::pod::PodOption<zeropod::pod::PodU64, 1>,
}

const _: () = assert!(core::mem::size_of::<MasterEditionPrefixZc>() == 18);
const _: () = assert!(core::mem::align_of::<MasterEditionPrefixZc>() == 1);
const _: () = assert!(core::mem::offset_of!(MasterEditionPrefixZc, key) == 0);
const _: () = assert!(core::mem::offset_of!(MasterEditionPrefixZc, supply) == 1);
const _: () = assert!(core::mem::offset_of!(MasterEditionPrefixZc, max_supply) == 9);
const _: () = assert!(<MasterEditionPrefix as quasar_lang::ZeroPodFixed>::SIZE == 18);

/// Semantic accessors for MasterEditionPrefixZc.
impl MasterEditionPrefixZc {
    #[inline(always)]
    pub fn max_supply_tag_valid(&self) -> bool {
        self.max_supply.raw_tag() <= 1
    }

    #[inline(always)]
    pub fn supply_value(&self) -> u64 {
        self.supply.get()
    }

    #[inline(always)]
    pub fn max_supply_value(&self) -> Option<u64> {
        self.max_supply.get_ref().map(|v| v.get())
    }
}

quasar_lang::define_account!(
    /// Metaplex Metadata account; validates owner is Metadata program.
    ///
    /// Derefs to [`MetadataPrefixZc`] for zero-copy access to the fixed-size
    /// header (update_authority, mint). Variable-length fields (name, symbol,
    /// uri, creators) require Borsh deserialization and are not exposed here.
    ///
    /// Checks: owner == Metadata program, data_len >= 65, key byte == 4,
    /// ZeroPod validation.
    pub struct MetadataAccount => [checks::Owner, checks::Discriminator, checks::DataLen, checks::ZeroPod]: MetadataPrefix
);

impl Owner for MetadataAccount {
    const OWNER: Address = METADATA_PROGRAM_ID;
}

impl quasar_lang::traits::Discriminator for MetadataAccount {
    const DISCRIMINATOR: &'static [u8] = &[KEY_METADATA_V1];
}

quasar_lang::define_account!(
    /// Metaplex MasterEdition account; validates owner is Metadata program.
    ///
    /// Derefs to [`MasterEditionPrefixZc`] for zero-copy access to supply and
    /// max_supply fields.
    ///
    /// Checks: owner == Metadata program, data_len >= 18, key byte == 6,
    /// ZeroPod validation.
    pub struct MasterEditionAccount => [checks::Owner, checks::Discriminator, checks::DataLen, checks::ZeroPod]: MasterEditionPrefix
);

impl Owner for MasterEditionAccount {
    const OWNER: Address = METADATA_PROGRAM_ID;
}

impl quasar_lang::traits::Discriminator for MasterEditionAccount {
    const DISCRIMINATOR: &'static [u8] = &[KEY_MASTER_EDITION_V2];
}

#[cfg(kani)]
#[path = "../kani/state.rs"]
mod kani_proofs;
