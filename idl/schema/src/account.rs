use {
    crate::{space::IdlSpace, types::IdlType},
    serde::{Deserialize, Serialize},
};

/// An account data definition (state stored on-chain).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdlAccountDef {
    pub name: String,
    pub discriminator: Vec<u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space: Option<IdlSpace>,
}

/// An account node in an instruction's account list (the resolver graph).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdlAccountNode {
    pub name: String,
    #[serde(
        rename = "clientType",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub client_type: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,
    #[serde(default)]
    pub writable: AccountFlag,
    #[serde(default)]
    pub signer: AccountFlag,
    pub resolver: IdlResolver,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Account meta flag: fixed boolean, caller-provided, or runtime-resolved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AccountFlag {
    Fixed(bool),
    Dynamic(AccountFlagDynamic),
}

impl Default for AccountFlag {
    fn default() -> Self {
        Self::Fixed(false)
    }
}

impl AccountFlag {
    /// Returns true if the flag is fixed true.
    pub fn is_true(&self) -> bool {
        matches!(self, Self::Fixed(true))
    }

    /// Returns true if the flag is fixed false.
    pub fn is_false(&self) -> bool {
        matches!(self, Self::Fixed(false))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AccountFlagDynamic {
    /// The flag's value is supplied by the caller at build time.
    Input,
}

/// How an account address is resolved for client construction.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum IdlResolver {
    /// Client must provide the account.
    #[serde(rename = "input")]
    Input {},
    /// Fixed address.
    #[serde(rename = "const")]
    Const { address: String },
    /// Well-known program or sysvar.
    #[serde(rename = "knownProgram")]
    KnownProgram { name: String },
    /// PDA derived from seeds.
    #[serde(rename = "pda")]
    Pda {
        program: IdlPdaProgram,
        seeds: Vec<IdlPdaSeed>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bump: Option<IdlPdaBump>,
    },
    /// Associated token account.
    #[serde(rename = "associatedToken")]
    AssociatedToken {
        mint: String,
        owner: String,
        #[serde(
            rename = "tokenProgram",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        token_program: Option<String>,
    },
    /// Resolved from a field of another account.
    #[serde(rename = "accountField")]
    AccountField { account: String, field: String },
    /// Resolved from an instruction argument.
    #[serde(rename = "arg")]
    Arg { path: String },
    /// Optional wrapper around another resolver.
    #[serde(rename = "optional")]
    Optional { resolver: Box<IdlResolver> },
    /// Account comes from remaining accounts.
    #[serde(rename = "remaining")]
    Remaining { index: Option<usize> },
}

/// Which program to derive a PDA against.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum IdlPdaProgram {
    #[serde(rename = "programId")]
    ProgramId {},
    #[serde(rename = "account")]
    Account { path: String },
}

/// A PDA seed.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum IdlPdaSeed {
    /// Constant byte value.
    #[serde(rename = "const")]
    Const { value: Vec<u8> },
    /// Derived from another account's address.
    #[serde(rename = "account")]
    Account { path: String },
    /// Derived from a decoded field of another account.
    #[serde(rename = "accountField")]
    AccountField {
        path: String,
        account: String,
        field: String,
    },
    /// Derived from an instruction argument.
    #[serde(rename = "arg")]
    Arg {
        path: String,
        #[serde(rename = "type")]
        ty: IdlType,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        encoding: Option<SeedEncoding>,
    },
}

/// How a seed value is encoded to bytes for PDA derivation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SeedEncoding {
    /// Little-endian bytes (for integers).
    Le,
    /// Raw 32 bytes (for pubkeys).
    Raw,
    /// UTF-8 bytes without length prefix (for strings).
    Utf8,
}

/// How the PDA bump is determined.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum IdlPdaBump {
    #[serde(rename = "canonical")]
    Canonical {},
    #[serde(rename = "arg")]
    Arg { path: String },
    #[serde(rename = "account")]
    Account { path: String, field: String },
}

/// Remaining accounts configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdlRemainingAccounts {
    pub kind: RemainingAccountsKind,
    pub name: String,
    pub min: usize,
    pub max: Option<usize>,
    pub item: RemainingAccountItem,
    pub policy: RemainingAccountPolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemainingAccountsKind {
    Append,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemainingAccountItem {
    #[serde(rename = "clientType")]
    pub client_type: String,
    pub signer: AccountFlag,
    pub writable: AccountFlag,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemainingAccountPolicy {
    pub position: RemainingPosition,
    pub order: RemainingOrder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemainingPosition {
    AfterDeclaredAccounts,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemainingOrder {
    PreserveInput,
}
