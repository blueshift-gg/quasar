use quasar_lang::{
    Address,
    account::{InterfaceAccount, InterfaceKind},
    zeropod::{self, Layout},
};

/// The stable 65-byte prefix. Name, URI, creators and newer fields follow it.
#[derive(zeropod::ZeroPod)]
#[zeropod(crate = quasar_lang::zeropod)]
pub struct Metadata {
    pub key: u8,
    pub update_authority: Address,
    pub mint: Address,
}

impl InterfaceKind for Metadata {
    type Owners = crate::MetadataProgram;

    fn validate(data: &[u8]) -> bool {
        Self::view(data).is_ok_and(|view| view.key() == 4)
    }
}

/// MasterEditionV2's Borsh prefix. `None` takes ten bytes, `Some` eighteen;
/// allocated accounts may contain trailing padding.
#[derive(zeropod::ZeroPod)]
#[zeropod(crate = quasar_lang::zeropod)]
pub struct MasterEdition {
    pub key: u8,
    pub supply: u64,
    pub max_supply: Option<u64>,
}

impl InterfaceKind for MasterEdition {
    type Owners = crate::MetadataProgram;

    fn validate(data: &[u8]) -> bool {
        Self::view(data).is_ok_and(|view| view.key() == 6)
    }
}

/// These wrappers check owner, prefix length, discriminant and encoding.
pub type MetadataAccount = InterfaceAccount<Metadata>;
pub type MasterEditionAccount = InterfaceAccount<MasterEdition>;

#[cfg(kani)]
mod proofs {
    use super::*;

    #[kani::proof]
    fn metadata_key_is_required() {
        let mut data = [0; 65];
        data[0] = kani::any();
        assert_eq!(Metadata::validate(&data), data[0] == 4);
    }

    #[kani::proof]
    fn edition_option_tag_is_checked() {
        let mut data = [0; 18];
        data[0] = kani::any();
        data[9] = kani::any();
        assert_eq!(MasterEdition::validate(&data), data[0] == 6 && data[9] <= 1);
    }
}
