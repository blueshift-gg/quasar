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

// The upstream ZeroPod derive exposes `*Zc` companion items without carrying
// source docs. Confine that generated-code lint exception to this module.
#[allow(missing_docs)]
mod layouts {
    use super::*;

    /// Zero-copy layout for the fixed-size prefix of Metaplex Metadata
    /// accounts.
    ///
    /// The first 65 bytes contain the account key, update authority, and mint.
    /// Variable-length Borsh fields follow this prefix.
    #[derive(quasar_lang::ZeroPod)]
    pub struct MetadataPrefix {
        /// Metaplex account-type discriminant.
        pub key: u8,
        /// Authority permitted to update this metadata.
        pub update_authority: Address,
        /// SPL Token mint described by this metadata.
        pub mint: Address,
    }

    const _: () = assert!(core::mem::size_of::<MetadataPrefixZc>() == 65);
    const _: () = assert!(core::mem::align_of::<MetadataPrefixZc>() == 1);
    const _: () = assert!(core::mem::offset_of!(MetadataPrefixZc, key) == 0);
    const _: () = assert!(core::mem::offset_of!(MetadataPrefixZc, update_authority) == 1);
    const _: () = assert!(core::mem::offset_of!(MetadataPrefixZc, mint) == 33);
    const _: () = assert!(<MetadataPrefix as quasar_lang::ZeroPodFixed>::SIZE == 65);

    /// Zero-copy layout for the fixed-size Metaplex MasterEdition prefix.
    #[derive(quasar_lang::ZeroPod)]
    pub struct MasterEditionPrefix {
        /// Metaplex account-type discriminant.
        pub key: u8,
        /// Number of editions printed.
        pub supply: u64,
        /// Optional maximum printable-edition supply.
        pub max_supply: zeropod::pod::PodOption<zeropod::pod::PodU64, 1>,
    }

    const _: () = assert!(core::mem::size_of::<MasterEditionPrefixZc>() == 18);
    const _: () = assert!(core::mem::align_of::<MasterEditionPrefixZc>() == 1);
    const _: () = assert!(core::mem::offset_of!(MasterEditionPrefixZc, key) == 0);
    const _: () = assert!(core::mem::offset_of!(MasterEditionPrefixZc, supply) == 1);
    const _: () = assert!(core::mem::offset_of!(MasterEditionPrefixZc, max_supply) == 9);
    const _: () = assert!(<MasterEditionPrefix as quasar_lang::ZeroPodFixed>::SIZE == 18);

    impl MasterEditionPrefixZc {
        /// Returns whether the Borsh option tag is zero or one.
        #[inline(always)]
        pub fn max_supply_tag_valid(&self) -> bool {
            self.max_supply.raw_tag() <= 1
        }

        /// Returns the decoded printed-edition supply.
        #[inline(always)]
        pub fn supply_value(&self) -> u64 {
            self.supply.get()
        }

        /// Returns the decoded optional maximum supply.
        #[inline(always)]
        pub fn max_supply_value(&self) -> Option<u64> {
            self.max_supply.get_ref().map(|v| v.get())
        }
    }
}

pub use layouts::{MasterEditionPrefix, MasterEditionPrefixZc, MetadataPrefix, MetadataPrefixZc};

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
