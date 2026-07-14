//! Quasar compiler validation-plan data stored below the IDL `extensions`
//! field.

use {
    serde::{Deserialize, Serialize},
    std::collections::BTreeMap,
};

/// Root key used inside [`crate::Idl::extensions`].
pub const VALIDATION_EXTENSION_KEY: &str = "quasar:validationPlan";
/// Current validation-plan extension format.
pub const VALIDATION_EXTENSION_VERSION: u32 = 1;

/// Resolved validation plans keyed by instruction name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdlValidationPlan {
    /// Extension format version, independent from the wire IDL spec.
    pub version: u32,
    /// Compiler plan for every instruction that uses an accounts struct.
    pub instructions: BTreeMap<String, IdlAccountsValidation>,
}

/// Resolved plan for one accounts struct as used by an instruction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdlAccountsValidation {
    /// How rent is sourced for initialization and reallocation.
    pub rent: String,
    /// Account fields in parse order.
    pub accounts: Vec<IdlAccountValidation>,
}

/// Resolved validation and lifecycle phases for one account field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdlAccountValidation {
    /// Client-facing account name.
    pub name: String,
    /// Source-level effective wrapper type.
    pub account_type: String,
    /// Compiler wrapper classification.
    pub wrapper: String,
    /// Whether the account meta must be writable.
    pub writable: bool,
    /// Whether the account meta must sign.
    pub signer: bool,
    /// Whether the account can be omitted.
    pub optional: bool,
    /// Whether duplicate account aliases are accepted.
    pub allow_duplicate: bool,
    /// Resolved account loading strategy.
    pub load: String,
    /// Phase-ordered operations before account loading.
    pub pre_load: Vec<String>,
    /// Phase-ordered checks and mutations after loading.
    pub post_load: Vec<String>,
    /// Phase-ordered exit and close operations.
    pub epilogue: Vec<String>,
}
