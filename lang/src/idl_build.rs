//! IDL fragment collection (feature-gated behind `idl-build`).
//!
//! Each derive macro (`#[account]`, `#[event]`, `#[error_code]`,
//! `#[derive(QuasarSerialize)]`) emits an inventory submission that registers
//! a fragment. The `#[program]` macro emits a collection point that assembles
//! all fragments into a complete `Idl`.

extern crate alloc;
#[allow(unused_imports)]
pub use alloc::vec;
pub use alloc::{boxed::Box, string::String, vec::Vec};

/// Convert `&str` to [`String`] in generated IDL code.
#[inline]
pub fn s(v: &str) -> String {
    String::from(v)
}

/// Convert a Solana address to base58 string.
pub fn address_to_base58(addr: &solana_address::Address) -> String {
    bs58::encode(addr.as_array()).into_string()
}

/// Re-exports for generated code (proc macros reference these via
/// `::quasar_lang::idl_build::__reexport::*`).
pub mod __reexport {
    pub use {quasar_idl_schema::*, serde_json};
}

use quasar_idl_schema::*;

/// Fragment submitted by `#[account]`; uses a fn pointer to avoid static
/// alloc.
pub struct AccountFragment {
    pub build: fn() -> (IdlAccountDef, IdlTypeDef),
}

/// Fragment submitted by `#[derive(QuasarSerialize)]` for instruction arg
/// types.
pub struct TypeFragment {
    pub build: fn() -> IdlTypeDef,
}

/// Fragment submitted by `#[event]`.
pub struct EventFragment {
    pub build: fn() -> (IdlEventDef, IdlTypeDef),
}

/// Fragment submitted by `#[error_code]`.
pub struct ErrorFragment {
    pub build: fn() -> Vec<IdlErrorDef>,
}

/// Fragment submitted by `#[program]` for each `#[instruction]`.
pub struct InstructionFragment {
    pub build: fn() -> IdlInstruction,
    /// Name of the accounts struct used by this instruction (for lookup).
    pub accounts_struct_name: &'static str,
    /// Whether the discriminator was pinned in source or assigned by
    /// `#[program]`.
    pub discriminator_source: InstructionDiscriminatorSource,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InstructionDiscriminatorSource {
    Auto,
    Explicit,
}

/// Fragment submitted by `#[derive(Accounts)]`; carries account metadata for
/// IDL.
pub struct AccountsMetaFragment(pub fn() -> (String, Vec<IdlAccountNode>));

inventory::collect!(AccountFragment);
inventory::collect!(TypeFragment);
inventory::collect!(EventFragment);
inventory::collect!(ErrorFragment);
inventory::collect!(InstructionFragment);
inventory::collect!(AccountsMetaFragment);

/// Assemble all registered fragments into a complete IDL.
///
/// `crate_name` is the Cargo package name of the program crate (threaded from
/// `env!("CARGO_PKG_NAME")` at the call site); it is distinct from `name`, the
/// `#[program]` module name.
pub fn build_idl(address: &str, name: &str, crate_name: &str, version: &str) -> Idl {
    let mut accounts = Vec::new();
    let mut types = Vec::new();
    let mut events = Vec::new();
    let mut errors = Vec::new();
    let mut instructions = Vec::new();
    let mut auto_discriminator_sources = serde_json::Map::new();

    // Collect accounts meta fragments into a lookup table.
    let accounts_meta: Vec<(String, Vec<IdlAccountNode>)> = inventory::iter::<AccountsMetaFragment>
        .into_iter()
        .map(|frag| (frag.0)())
        .collect();

    for frag in inventory::iter::<AccountFragment> {
        let (account_def, type_def) = (frag.build)();
        accounts.push(account_def);
        types.push(type_def);
    }
    for frag in inventory::iter::<TypeFragment> {
        types.push((frag.build)());
    }
    for frag in inventory::iter::<EventFragment> {
        let (event_def, type_def) = (frag.build)();
        events.push(event_def);
        types.push(type_def);
    }
    for frag in inventory::iter::<ErrorFragment> {
        errors.extend((frag.build)());
    }
    for frag in inventory::iter::<InstructionFragment> {
        let mut ix = (frag.build)();
        if frag.discriminator_source == InstructionDiscriminatorSource::Auto {
            auto_discriminator_sources.insert(
                ix.name.clone(),
                serde_json::Value::String(String::from("auto")),
            );
        }
        // Look up the matching AccountsMetaFragment by struct name. A missing
        // fragment is a hard error: the instruction names an accounts struct
        // whose metadata never registered (e.g. a fragment-name mismatch),
        // which would otherwise silently emit an instruction with no accounts.
        if ix.accounts.is_empty() && !frag.accounts_struct_name.is_empty() {
            let (_, nodes) = accounts_meta
                .iter()
                .find(|(struct_name, _)| struct_name == frag.accounts_struct_name)
                .unwrap_or_else(|| {
                    panic!(
                        "idl-build: instruction `{}` references accounts struct `{}` but no \
                         AccountsMetaFragment with that name was registered",
                        ix.name, frag.accounts_struct_name
                    )
                });
            ix.accounts = nodes.clone();
        }
        instructions.push(ix);
    }

    // Deterministic assembly: `inventory` yields fragments in unspecified,
    // link-order-dependent order, but the assembled IDL is hashed, so the
    // output must not depend on registration order. Sort every collection by a
    // stable key: instructions by discriminator (tie-break on name), everything
    // else by name.
    instructions.sort_by(|a, b| {
        a.discriminator
            .cmp(&b.discriminator)
            .then_with(|| a.name.cmp(&b.name))
    });
    accounts.sort_by(|a, b| a.name.cmp(&b.name));
    types.sort_by(|a, b| a.name.cmp(&b.name));
    events.sort_by(|a, b| a.name.cmp(&b.name));
    errors.sort_by(|a, b| a.name.cmp(&b.name));

    let mut idl = Idl {
        spec: String::from(CURRENT_SPEC),
        name: String::from(name),
        version: String::from(version),
        address: String::from(address),
        metadata: IdlMetadata {
            crate_name: Some(String::from(crate_name)),
            generator_version: Some(String::from(env!("CARGO_PKG_VERSION"))),
            schema_version: Some(String::from("1.0.0")),
            ..IdlMetadata::default()
        },
        docs: Vec::new(),
        instructions,
        accounts,
        types,
        events,
        errors,
        constants: Vec::new(),
        wrappers: None,
        extensions: None,
        hashes: None,
    };

    if !auto_discriminator_sources.is_empty() {
        idl.metadata.extra.insert(
            String::from("quasar:instructionDiscriminatorSource"),
            serde_json::Value::Object(auto_discriminator_sources),
        );
    }

    let idl_hash = compute_idl_hash(&idl);
    let abi_hash = compute_abi_hash(&idl);
    idl.hashes = Some(IdlHashes {
        idl: idl_hash,
        abi: abi_hash,
    });

    idl
}
