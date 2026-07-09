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
        extensions: None,
        hashes: None,
    };

    if !auto_discriminator_sources.is_empty() {
        idl.metadata.extra.insert(
            String::from("quasar:instructionDiscriminatorSource"),
            serde_json::Value::Object(auto_discriminator_sources),
        );
    }

    assert_dynamic_fields_have_codecs(&idl);
    assert_account_discriminators_distinct(&idl);

    let idl_hash = compute_idl_hash(&idl);
    let abi_hash = compute_abi_hash(&idl);
    idl.hashes = Some(IdlHashes {
        idl: idl_hash,
        abi: abi_hash,
    });

    idl
}

/// Whether an IDL type is dynamically sized and therefore requires an explicit
/// size-prefix codec. Strings and vecs (including those wrapped in `Option`)
/// carry a length prefix whose width the client must know; without a codec a
/// client defaults to a guessed width (e.g. u32) that diverges from the wire.
fn idl_type_needs_codec(ty: &IdlType) -> bool {
    match ty {
        IdlType::Primitive(name) => name == "string",
        IdlType::Vec { .. } => true,
        IdlType::Option { option } => idl_type_needs_codec(option),
        _ => false,
    }
}

/// Codec is mandatory for dynamic types: every string/vec field and arg must
/// carry an explicit codec so clients cannot silently pick the wrong prefix
/// width. A missing codec is a hard error (a producer bug), matching the
/// panic style used for missing account fragments above.
fn assert_dynamic_fields_have_codecs(idl: &Idl) {
    fn check(ty: &IdlType, codec_present: bool, location: &dyn core::fmt::Display) {
        if idl_type_needs_codec(ty) && !codec_present {
            panic!(
                "idl-build: {location} has a dynamic (string/vec) type but no codec; dynamic \
                 types must declare an explicit size-prefix codec so clients use the right prefix \
                 width"
            );
        }
    }

    for ix in &idl.instructions {
        for arg in &ix.args {
            check(
                &arg.ty,
                arg.codec.is_some(),
                &format_args!("instruction `{}` arg `{}`", ix.name, arg.name),
            );
        }
    }
    for ty in &idl.types {
        for field in &ty.fields {
            check(
                &field.ty,
                field.codec.is_some(),
                &format_args!("type `{}` field `{}`", ty.name, field.name),
            );
        }
        for variant in &ty.variants {
            for field in &variant.fields {
                check(
                    &field.ty,
                    field.codec.is_some(),
                    &format_args!(
                        "type `{}` variant `{}` field `{}`",
                        ty.name, variant.name, field.name
                    ),
                );
            }
        }
    }
}

/// Account discriminators must be pairwise distinct AND non-prefixing: the
/// runtime discriminator check reads only the declared-length prefix
/// (`checks/discriminator.rs`) and every `#[account]` shares `OWNER = crate::ID`,
/// so a discriminator that is a prefix of another (which includes exact
/// equality) is silent type confusion at runtime. This is the hard-error half of
/// lint rule P009; an empty discriminator (`unsafe_no_disc`) is an explicit
/// opt-out and is skipped. Panics in the missing-fragment/missing-codec style.
fn assert_account_discriminators_distinct(idl: &Idl) {
    fn collides(a: &[u8], b: &[u8]) -> bool {
        if a.is_empty() || b.is_empty() {
            return false;
        }
        let shared = a.len().min(b.len());
        a[..shared] == b[..shared]
    }

    for (i, account) in idl.accounts.iter().enumerate() {
        for other in &idl.accounts[i + 1..] {
            if collides(&account.discriminator, &other.discriminator) {
                panic!(
                    "idl-build: accounts `{}` and `{}` have colliding discriminators ({:?} vs \
                     {:?}); the runtime check is a prefix compare, so one account can be decoded \
                     as the other. Give every account a distinct discriminator that is not a \
                     prefix of another.",
                    account.name, other.name, account.discriminator, other.discriminator
                );
            }
        }
    }
}

#[cfg(test)]
mod codec_tests {
    use super::*;

    fn arg(ty: IdlType, codec: Option<IdlCodec>) -> IdlArg {
        IdlArg {
            name: String::from("x"),
            ty,
            codec,
            docs: Vec::new(),
        }
    }

    fn idl_with_arg(a: IdlArg) -> Idl {
        Idl {
            spec: String::from(CURRENT_SPEC),
            name: String::from("t"),
            version: String::from("0"),
            address: String::from("11111111111111111111111111111111"),
            metadata: IdlMetadata::default(),
            docs: Vec::new(),
            instructions: vec![IdlInstruction {
                name: String::from("ix"),
                discriminator: vec![0],
                docs: Vec::new(),
                accounts: Vec::new(),
                args: vec![a],
                layout: None,
                remaining_accounts: None,
            }],
            accounts: Vec::new(),
            types: Vec::new(),
            events: Vec::new(),
            errors: Vec::new(),
            extensions: None,
            hashes: None,
        }
    }

    fn u8_vec() -> IdlType {
        IdlType::Vec {
            vec: Box::new(IdlType::Primitive(String::from("u8"))),
        }
    }

    fn vec_codec() -> IdlCodec {
        IdlCodec::SizePrefixed {
            prefix: ScalarRepr {
                ty: String::from("u16"),
                endian: Endian::Le,
            },
            storage: Storage::Tail,
            max_bytes: None,
            max_items: Some(4),
            encoding: None,
            item: None,
        }
    }

    #[test]
    fn fixed_arg_without_codec_ok() {
        assert_dynamic_fields_have_codecs(&idl_with_arg(arg(
            IdlType::Primitive(String::from("u64")),
            None,
        )));
    }

    #[test]
    fn dynamic_arg_with_codec_ok() {
        assert_dynamic_fields_have_codecs(&idl_with_arg(arg(u8_vec(), Some(vec_codec()))));
    }

    #[test]
    #[should_panic(expected = "no codec")]
    fn dynamic_arg_without_codec_panics() {
        assert_dynamic_fields_have_codecs(&idl_with_arg(arg(u8_vec(), None)));
    }

    fn idl_with_account_discs(discs: &[(&str, Vec<u8>)]) -> Idl {
        let mut idl = idl_with_arg(arg(IdlType::Primitive(String::from("u64")), None));
        idl.accounts = discs
            .iter()
            .map(|(name, disc)| IdlAccountDef {
                name: String::from(*name),
                discriminator: disc.clone(),
                docs: Vec::new(),
                space: None,
            })
            .collect();
        idl
    }

    #[test]
    fn distinct_account_discriminators_ok() {
        assert_account_discriminators_distinct(&idl_with_account_discs(&[
            ("A", vec![1]),
            ("B", vec![2]),
            ("C", vec![3, 4]),
        ]));
    }

    #[test]
    fn no_disc_accounts_are_skipped() {
        assert_account_discriminators_distinct(&idl_with_account_discs(&[
            ("A", Vec::new()),
            ("B", Vec::new()),
        ]));
    }

    #[test]
    #[should_panic(expected = "colliding discriminators")]
    fn equal_account_discriminators_panic() {
        assert_account_discriminators_distinct(&idl_with_account_discs(&[
            ("A", vec![1]),
            ("B", vec![1]),
        ]));
    }

    #[test]
    #[should_panic(expected = "colliding discriminators")]
    fn prefix_account_discriminators_panic() {
        assert_account_discriminators_distinct(&idl_with_account_discs(&[
            ("A", vec![1]),
            ("B", vec![1, 2]),
        ]));
    }
}
