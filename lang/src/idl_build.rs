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
pub fn build_idl(address: &str, name: &str, version: &str) -> Idl {
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
        // Look up the matching AccountsMetaFragment by struct name.
        if ix.accounts.is_empty() && !frag.accounts_struct_name.is_empty() {
            if let Some((_, nodes)) = accounts_meta
                .iter()
                .find(|(struct_name, _)| struct_name == frag.accounts_struct_name)
            {
                ix.accounts = nodes.clone();
            }
        }
        instructions.push(ix);
    }

    let mut idl = Idl {
        spec: String::from("quasar-idl/1.0.0"),
        name: String::from(name),
        version: String::from(version),
        address: String::from(address),
        metadata: IdlMetadata {
            crate_name: Some(String::from(name)),
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
