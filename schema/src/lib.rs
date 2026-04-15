//! Shared schema types consumed by multiple Quasar crates.
//!
//! Keep this crate narrow: only types that are true cross-pipeline contracts
//! belong here.

use serde::{Deserialize, Serialize};

pub fn known_address_for_type(base: &str, inner: Option<&str>) -> Option<&'static str> {
    match (base, inner) {
        ("SystemProgram", _) | ("Program", Some("System")) => {
            Some("11111111111111111111111111111111")
        }
        ("Program", Some("Token")) => Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
        ("Program", Some("Token2022")) => Some("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"),
        ("Program", Some("AssociatedTokenProgram")) => {
            Some("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")
        }
        ("Sysvar", Some("Rent")) => Some("SysvarRent111111111111111111111111111111111"),
        ("Sysvar", Some("Clock")) => Some("SysvarC1ock11111111111111111111111111111111"),
        _ => None,
    }
}

#[derive(Serialize, Deserialize)]
pub struct Idl {
    pub address: String,
    #[serde(default)]
    pub metadata: IdlMetadata,
    #[serde(default)]
    pub instructions: Vec<IdlInstruction>,
    #[serde(default)]
    pub accounts: Vec<IdlAccountDef>,
    #[serde(default)]
    pub events: Vec<IdlEventDef>,
    #[serde(default)]
    pub types: Vec<IdlTypeDef>,
    #[serde(default)]
    pub errors: Vec<IdlError>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct IdlMetadata {
    pub name: String,
    #[serde(skip)]
    pub crate_name: String,
    pub version: String,
    pub spec: String,
}

#[derive(Serialize, Deserialize)]
pub struct IdlInstruction {
    pub name: String,
    pub discriminator: Vec<u8>,
    pub accounts: Vec<IdlAccountItem>,
    pub args: Vec<IdlField>,
    #[serde(rename = "hasRemaining", default, skip_serializing_if = "is_false")]
    pub has_remaining: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct IdlAccountItem {
    pub name: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub writable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub signer: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pda: Option<IdlPda>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

fn is_false(b: &bool) -> bool {
    !b
}

#[derive(Clone, Serialize, Deserialize)]
pub struct IdlPda {
    pub seeds: Vec<IdlSeed>,
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum IdlSeed {
    #[serde(rename = "const")]
    Const { value: Vec<u8> },
    #[serde(rename = "account")]
    Account { path: String },
    #[serde(rename = "arg")]
    Arg { path: String },
}

#[derive(Clone, Serialize, Deserialize)]
pub struct IdlField {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: IdlType,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct IdlDynString {
    #[serde(rename = "maxLength")]
    pub max_length: usize,
    /// Byte width of the length prefix: 1 (u8, default), 2 (u16), 4 (u32), or 8
    /// (u64).
    #[serde(rename = "prefixBytes")]
    pub prefix_bytes: usize,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct IdlDynVec {
    pub items: Box<IdlType>,
    #[serde(rename = "maxLength")]
    pub max_length: usize,
    /// Byte width of the count prefix: 1 (u8), 2 (u16, default), 4 (u32), or 8
    /// (u64).
    #[serde(rename = "prefixBytes")]
    pub prefix_bytes: usize,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IdlType {
    Primitive(String),
    Defined { defined: String },
    DynString { string: IdlDynString },
    DynVec { vec: IdlDynVec },
}

#[derive(Serialize, Deserialize)]
pub struct IdlAccountDef {
    pub name: String,
    pub discriminator: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct IdlEventDef {
    pub name: String,
    pub discriminator: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct IdlTypeDef {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: IdlTypeDefType,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct IdlTypeDefType {
    pub kind: String,
    pub fields: Vec<IdlField>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct IdlError {
    pub code: u32,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg: Option<String>,
}
