//! Library constraints: `metadata(mint = mint, update_authority = authority)`
//! and `master_edition(mint = mint)`. Metaplex initialization uses the explicit
//! helpers: its PDAs are created by Metaplex, not by the calling program.
use quasar_lang::{Result, input::Entry, wrapper::Wrapper};

pub mod metadata {
    use super::*;

    #[derive(Default)]
    pub struct Check<'a> {
        pub program: Option<&'a Entry>,
        pub mint: Option<&'a Entry>,
        pub update_authority: Option<&'a Entry>,
    }

    impl Check<'_> {
        pub fn check(&self, account: &crate::MetadataAccount) -> Result {
            if let Some(program) = self.program {
                crate::validate::validate_metadata_program(program)?;
            }
            let mint = self.mint.map(Entry::key).unwrap_or_else(|| account.mint());
            crate::validate::validate_metadata_account(
                account.entry(),
                mint,
                self.update_authority.map(Entry::key),
            )?;
            crate::pda::verify_metadata_address(account.address(), mint)
        }
    }
}

pub mod master_edition {
    use super::*;

    #[derive(Default)]
    pub struct Check<'a> {
        pub program: Option<&'a Entry>,
        pub mint: Option<&'a Entry>,
        pub metadata: Option<&'a Entry>,
    }

    impl Check<'_> {
        pub fn check(&self, account: &crate::MasterEditionAccount) -> Result {
            if let Some(program) = self.program {
                crate::validate::validate_metadata_program(program)?;
            }
            let mint = self.mint.ok_or(quasar_lang::ProgramError::InvalidArgument)?.key();
            crate::validate::validate_master_edition_account(account.entry(), Some(mint))?;
            if let Some(metadata) = self.metadata {
                crate::validate::validate_metadata_account(metadata, mint, None)?;
                crate::pda::verify_metadata_address(metadata.key(), mint)?;
            }
            Ok(())
        }
    }
}
