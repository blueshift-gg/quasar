//! Serializable snapshot of a Quasar program's lint-relevant surface.
//!
//! `quasar build` parses source into [`crate::parser::ParsedProgram`].
//! That structure carries `syn::Type` trees for fields and args, which
//! aren't serializable and contain a lot of data the diff rules don't
//! care about. This module flattens it into a [`ProgramSnapshot`] —
//! stringified types, only the fields each diff rule needs — and
//! persists it as `quasar.lock.json` alongside the program crate.
//!
//! ## Invariants
//!
//! - **Field declaration order is preserved.** L013 (reorder), L016 (insert
//!   middle), L017 (append) all depend on it; the snapshot captures fields as
//!   ordered `Vec`s, never sets/maps.
//! - **Type comparison is string-equality on the stringified `syn::Type`.**
//!   Whitespace is normalized via `quote!` round-trip. This is good enough for
//!   catching `u64 → u32`, `Pubkey → [u8; 32]`, etc., and intentionally fragile
//!   for cosmetic differences (an alias rename shows as a retype — usually the
//!   right call).
//! - **Discriminators are byte-compared.** No semantic interpretation.
//!
//! ## Lock-file lifecycle
//!
//! The lock file is committed to source control. `quasar build` updates
//! it (via an explicit flag, never silently). `quasar lint --diff`
//! reads it and compares the current parse to the stored snapshot,
//! producing an [`super::types::LintReport`] with L013–L026 findings.

use {
    crate::parser::{
        accounts::{RawAccountField, RawAccountsStruct, RawPda, RawSeed},
        events::RawEvent,
        program::RawInstruction,
        state::RawStateAccount,
        ParsedProgram,
    },
    quote::ToTokens,
    serde::{Deserialize, Serialize},
    std::{
        io,
        path::{Path, PathBuf},
    },
};

/// Default file name for the persisted snapshot — committed alongside
/// the program crate's `Cargo.toml`.
pub const LOCK_FILE_NAME: &str = "quasar.lock.json";

/// Schema version for `quasar.lock.json`. Bump on any breaking change
/// to the snapshot shape so old lock files surface a clean error
/// rather than silently mis-comparing.
pub const SNAPSHOT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProgramSnapshot {
    pub version: u32,
    pub program_id: String,
    pub program_name: String,
    pub accounts: Vec<AccountSnapshot>,
    pub instructions: Vec<InstructionSnapshot>,
    pub events: Vec<EventSnapshot>,
    pub accounts_structs: Vec<AccountsStructSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountSnapshot {
    pub name: String,
    pub discriminator: Vec<u8>,
    /// Ordered field list. Order is load-bearing for the reorder /
    /// insert / append rules.
    pub fields: Vec<NamedTypeSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstructionSnapshot {
    pub name: String,
    pub discriminator: Vec<u8>,
    /// Argument list in declaration order.
    pub args: Vec<NamedTypeSnapshot>,
    pub accounts_type_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventSnapshot {
    pub name: String,
    pub discriminator: Vec<u8>,
    pub fields: Vec<NamedTypeSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountsStructSnapshot {
    pub name: String,
    pub fields: Vec<AccountSlotSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountSlotSnapshot {
    pub name: String,
    pub writable: bool,
    pub signer: bool,
    pub pda: Option<PdaSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PdaSnapshot {
    pub seeds: Vec<SeedSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SeedSnapshot {
    Const { bytes: Vec<u8> },
    Account { name: String },
    Arg { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NamedTypeSnapshot {
    pub name: String,
    /// Stringified `syn::Type`. Compared by string equality at diff
    /// time — see module docs for the rationale.
    pub ty: String,
}

/// Resolve `quasar.lock.json` for a program crate path. Always pinned
/// to `LOCK_FILE_NAME` at the crate root so devs can find / inspect /
/// review it without flag spelunking.
pub fn lock_path(crate_root: &Path) -> PathBuf {
    crate_root.join(LOCK_FILE_NAME)
}

/// Errors from lock-file IO. Distinguishes "no lock yet" (a benign
/// state for fresh projects — caller should suggest `--update-lock`)
/// from real parse / version-mismatch failures.
#[derive(Debug)]
pub enum SnapshotIoError {
    NotFound(PathBuf),
    Io(io::Error),
    Parse(serde_json::Error),
    VersionMismatch { expected: u32, found: u32 },
}

impl std::fmt::Display for SnapshotIoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(p) => write!(
                f,
                "no lock file at {} — run `quasar lint --update-lock` to create one",
                p.display()
            ),
            Self::Io(e) => write!(f, "reading lock file: {e}"),
            Self::Parse(e) => write!(f, "parsing lock file: {e}"),
            Self::VersionMismatch { expected, found } => write!(
                f,
                "lock file schema version {found} doesn't match expected {expected}; regenerate \
                 with `quasar lint --update-lock`"
            ),
        }
    }
}

impl std::error::Error for SnapshotIoError {}

impl ProgramSnapshot {
    /// Project a freshly-parsed `ParsedProgram` into a serializable
    /// snapshot. Pure function — no I/O.
    pub fn from_parsed(parsed: &ParsedProgram) -> Self {
        Self {
            version: SNAPSHOT_VERSION,
            program_id: parsed.program_id.clone(),
            program_name: parsed.program_name.clone(),
            accounts: parsed
                .state_accounts
                .iter()
                .map(AccountSnapshot::from_raw)
                .collect(),
            instructions: parsed
                .instructions
                .iter()
                .map(InstructionSnapshot::from_raw)
                .collect(),
            events: parsed.events.iter().map(EventSnapshot::from_raw).collect(),
            accounts_structs: parsed
                .accounts_structs
                .iter()
                .map(AccountsStructSnapshot::from_raw)
                .collect(),
        }
    }

    /// Read a snapshot from disk. Validates the schema version up
    /// front — a mismatch produces a clear error rather than running
    /// rules against incompatible bytes.
    pub fn load(path: &Path) -> Result<Self, SnapshotIoError> {
        if !path.exists() {
            return Err(SnapshotIoError::NotFound(path.to_path_buf()));
        }
        let bytes = std::fs::read(path).map_err(SnapshotIoError::Io)?;
        let snap: Self = serde_json::from_slice(&bytes).map_err(SnapshotIoError::Parse)?;
        if snap.version != SNAPSHOT_VERSION {
            return Err(SnapshotIoError::VersionMismatch {
                expected: SNAPSHOT_VERSION,
                found: snap.version,
            });
        }
        Ok(snap)
    }

    /// Write the snapshot to disk as pretty-printed JSON. Pretty
    /// rather than compact so review diffs in source control are
    /// human-readable — the file size is small enough that it doesn't
    /// matter.
    pub fn save(&self, path: &Path) -> Result<(), SnapshotIoError> {
        let body = serde_json::to_string_pretty(self).map_err(SnapshotIoError::Parse)?;
        std::fs::write(path, body).map_err(SnapshotIoError::Io)?;
        Ok(())
    }
}

impl AccountSnapshot {
    fn from_raw(raw: &RawStateAccount) -> Self {
        Self {
            name: raw.name.clone(),
            discriminator: raw.discriminator.clone(),
            fields: raw
                .fields
                .iter()
                .map(NamedTypeSnapshot::from_pair)
                .collect(),
        }
    }
}

impl InstructionSnapshot {
    fn from_raw(raw: &RawInstruction) -> Self {
        Self {
            name: raw.name.clone(),
            discriminator: raw.discriminator.clone(),
            args: raw.args.iter().map(NamedTypeSnapshot::from_pair).collect(),
            accounts_type_name: raw.accounts_type_name.clone(),
        }
    }
}

impl EventSnapshot {
    fn from_raw(raw: &RawEvent) -> Self {
        Self {
            name: raw.name.clone(),
            discriminator: raw.discriminator.clone(),
            fields: raw
                .fields
                .iter()
                .map(NamedTypeSnapshot::from_pair)
                .collect(),
        }
    }
}

impl AccountsStructSnapshot {
    fn from_raw(raw: &RawAccountsStruct) -> Self {
        Self {
            name: raw.name.clone(),
            fields: raw
                .fields
                .iter()
                .map(AccountSlotSnapshot::from_raw)
                .collect(),
        }
    }
}

impl AccountSlotSnapshot {
    fn from_raw(raw: &RawAccountField) -> Self {
        Self {
            name: raw.name.clone(),
            writable: raw.writable,
            signer: raw.signer,
            pda: raw.pda.as_ref().map(PdaSnapshot::from_raw),
        }
    }
}

impl PdaSnapshot {
    fn from_raw(raw: &RawPda) -> Self {
        Self {
            seeds: raw.seeds.iter().map(SeedSnapshot::from_raw).collect(),
        }
    }
}

impl SeedSnapshot {
    fn from_raw(raw: &RawSeed) -> Self {
        match raw {
            RawSeed::ByteString(b) => SeedSnapshot::Const { bytes: b.clone() },
            RawSeed::AccountRef(n) => SeedSnapshot::Account { name: n.clone() },
            RawSeed::ArgRef(n) => SeedSnapshot::Arg { name: n.clone() },
        }
    }
}

impl NamedTypeSnapshot {
    fn from_pair((name, ty): &(String, syn::Type)) -> Self {
        Self {
            name: name.clone(),
            ty: stringify_type(ty),
        }
    }
}

/// Render a `syn::Type` to its canonical string form. Goes via
/// `quote`'s `ToTokens` impl so whitespace is normalized — `Vec<u8>`
/// and `Vec < u8 >` hash to the same string.
fn stringify_type(ty: &syn::Type) -> String {
    ty.to_token_stream().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ty(s: &str) -> syn::Type {
        syn::parse_str(s).unwrap()
    }

    fn make_account(name: &str, fields: &[(&str, &str)], disc: Vec<u8>) -> RawStateAccount {
        RawStateAccount {
            name: name.to_string(),
            discriminator: disc,
            fields: fields.iter().map(|(n, t)| (n.to_string(), ty(t))).collect(),
            seeds: None,
        }
    }

    fn parsed_with(state_accounts: Vec<RawStateAccount>) -> ParsedProgram {
        ParsedProgram {
            program_id: "11111111111111111111111111111111".to_string(),
            program_name: "test".to_string(),
            crate_name: "test".to_string(),
            version: "0.1.0".to_string(),
            instructions: vec![],
            accounts_structs: vec![],
            state_accounts,
            events: vec![],
            errors: vec![],
            data_structs: vec![],
        }
    }

    #[test]
    fn snapshot_preserves_field_order() {
        let p = parsed_with(vec![make_account(
            "Escrow",
            &[("a", "u64"), ("b", "Pubkey"), ("c", "u8")],
            vec![1],
        )]);
        let snap = ProgramSnapshot::from_parsed(&p);
        let names: Vec<&str> = snap.accounts[0]
            .fields
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    #[test]
    fn snapshot_normalizes_whitespace_in_types() {
        // `Vec<u8>` and `Vec < u8 >` both parse and stringify to the
        // same canonical form.
        let p1 = parsed_with(vec![make_account("X", &[("v", "Vec<u8>")], vec![1])]);
        let p2 = parsed_with(vec![make_account("X", &[("v", "Vec < u8 >")], vec![1])]);
        let s1 = ProgramSnapshot::from_parsed(&p1);
        let s2 = ProgramSnapshot::from_parsed(&p2);
        assert_eq!(s1.accounts[0].fields[0].ty, s2.accounts[0].fields[0].ty);
    }

    #[test]
    fn snapshot_round_trips_through_json() {
        let p = parsed_with(vec![make_account("X", &[("v", "u64")], vec![42])]);
        let snap = ProgramSnapshot::from_parsed(&p);
        let json = serde_json::to_string(&snap).unwrap();
        let back: ProgramSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snap, back);
    }
}
