//! Where a generated client gets each account address.
//!
//! Every backend classifies accounts through [`account_source`] instead of
//! matching [`IdlResolver`] itself. The match here is exhaustive, so a new
//! resolver kind is a compile error in one place rather than an account that
//! silently renders as a missing address in one language and a required input
//! in another.

use {
    super::model::{CodegenError, CodegenResult},
    crate::types::{IdlAccountNode, IdlResolver},
};

/// Well-known program and sysvar addresses, shared by every backend so a
/// literal is never pasted into a generator.
pub const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
pub const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
pub const TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
pub const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
pub const RENT_SYSVAR_ID: &str = "SysvarRent111111111111111111111111111111111";
pub const CLOCK_SYSVAR_ID: &str = "SysvarC1ock11111111111111111111111111111111";

/// Resolve a `knownProgram` name to its address.
pub fn well_known_program_id(name: &str) -> Option<&'static str> {
    match name {
        "system" | "systemProgram" => Some(SYSTEM_PROGRAM_ID),
        "token" | "tokenProgram" => Some(TOKEN_PROGRAM_ID),
        "token2022" | "tokenProgram2022" => Some(TOKEN_2022_PROGRAM_ID),
        "associatedToken" | "associatedTokenProgram" => Some(ASSOCIATED_TOKEN_PROGRAM_ID),
        "rent" => Some(RENT_SYSVAR_ID),
        "clock" => Some(CLOCK_SYSVAR_ID),
        _ => None,
    }
}

/// How a client obtains one account's address.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AccountSource<'a> {
    /// The caller passes the address.
    Input,
    /// A fixed address the client bakes in.
    Constant(&'a str),
    /// The client derives it (PDA or associated token account).
    Derived,
    /// The address is already an instruction argument.
    Arg(&'a str),
}

impl AccountSource<'_> {
    /// Whether the caller must supply this address as a client input.
    pub fn is_input(&self) -> bool {
        matches!(self, Self::Input)
    }

    /// Whether the client computes this address rather than being told it.
    pub fn is_derived(&self) -> bool {
        matches!(self, Self::Derived)
    }
}

/// Classify one account. Optional accounts stay caller-controlled whatever
/// they wrap: omitting one keeps the runtime's program-id sentinel.
///
/// # Errors
///
/// Rejects resolver kinds no backend can lower into an address without a
/// network fetch, so `quasar client` fails on the IDL instead of emitting a
/// client with a hole in it.
pub fn account_source(account: &IdlAccountNode) -> CodegenResult<AccountSource<'_>> {
    if account.optional {
        return Ok(AccountSource::Input);
    }
    source_of(&account.resolver, &account.name)
}

fn source_of<'a>(resolver: &'a IdlResolver, name: &str) -> CodegenResult<AccountSource<'a>> {
    match resolver {
        IdlResolver::Input {} => Ok(AccountSource::Input),
        IdlResolver::Const { address } => Ok(AccountSource::Constant(address)),
        IdlResolver::KnownProgram { name: program } => well_known_program_id(program)
            .map(AccountSource::Constant)
            .ok_or_else(|| {
                CodegenError::new(format!(
                    "account `{name}` names unknown program `{program}`; use a `const` resolver \
                     with its address"
                ))
            }),
        IdlResolver::Pda { .. } | IdlResolver::AssociatedToken { .. } => Ok(AccountSource::Derived),
        IdlResolver::Arg { path } => Ok(AccountSource::Arg(path)),
        // An address stored in another account's data needs that account
        // fetched and decoded. Builders stay offline, so this is not lowerable.
        IdlResolver::AccountField { account, field } => Err(CodegenError::new(format!(
            "account `{name}` resolves to `{account}.{field}`, which a client can only learn by \
             fetching that account; pass the address as an input instead"
        ))),
        // `optional` is already a flag on the node; a nested wrapper would make
        // two sources of truth for the same thing.
        IdlResolver::Optional { .. } => Err(CodegenError::new(format!(
            "account `{name}` wraps an `optional` resolver; set the node's `optional` flag instead"
        ))),
        IdlResolver::Remaining { .. } => Err(CodegenError::new(format!(
            "account `{name}` resolves from remaining accounts; declare it under \
             `remainingAccounts` instead"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::types::{AccountFlag, IdlAccountNode},
    };

    fn node(resolver: IdlResolver) -> IdlAccountNode {
        IdlAccountNode {
            name: "thing".to_owned(),
            optional: false,
            writable: AccountFlag::Fixed(false),
            signer: AccountFlag::Fixed(false),
            resolver,
            docs: vec![],
        }
    }

    #[test]
    fn known_programs_lower_to_constants() {
        let account = node(IdlResolver::KnownProgram {
            name: "token".to_owned(),
        });
        let source = account_source(&account).expect("known program");
        assert_eq!(source, AccountSource::Constant(TOKEN_PROGRAM_ID));
    }

    #[test]
    fn unknown_program_names_are_rejected() {
        let error = account_source(&node(IdlResolver::KnownProgram {
            name: "wormhole".to_owned(),
        }))
        .expect_err("unknown program");
        assert!(error.to_string().contains("unknown program `wormhole`"));
    }

    #[test]
    fn arg_addresses_come_from_the_argument() {
        let account = node(IdlResolver::Arg {
            path: "target".to_owned(),
        });
        let source = account_source(&account).expect("arg resolver");
        assert_eq!(source, AccountSource::Arg("target"));
    }

    #[test]
    fn unfetchable_and_duplicated_resolvers_are_rejected() {
        for resolver in [
            IdlResolver::AccountField {
                account: "Vault".to_owned(),
                field: "authority".to_owned(),
            },
            IdlResolver::Optional {
                resolver: Box::new(IdlResolver::Input {}),
            },
            IdlResolver::Remaining { index: None },
        ] {
            assert!(
                account_source(&node(resolver)).is_err(),
                "resolver must be rejected rather than silently skipped"
            );
        }
    }

    #[test]
    fn optional_accounts_stay_caller_controlled() {
        let mut account = node(IdlResolver::Pda {
            program: crate::types::IdlPdaProgram::ProgramId {},
            seeds: vec![],
        });
        account.optional = true;
        assert_eq!(
            account_source(&account).expect("optional account"),
            AccountSource::Input
        );
    }
}
