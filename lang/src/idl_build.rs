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

/// camelCase form of a snake_case behavior-arg name.
fn snake_to_camel(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut upper_next = false;
    for c in name.chars() {
        if c == '_' {
            upper_next = true;
        } else if upper_next {
            out.extend(c.to_uppercase());
            upper_next = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// Resolve one protocol behavior's address metadata against the account-field
/// arguments used by a concrete `#[account(...)]` declaration.
pub fn behavior_resolver(
    resolver: Option<crate::account_behavior::BehaviorIdlResolver>,
    account_args: &[(&str, &str)],
    fields: &[&str],
) -> Option<IdlResolver> {
    // An unmapped behavior arg resolves to the same-named account field
    // (mirroring the runtime init inference); `fields` carries camelCase
    // names, so compare the argument camelized.
    let account = |argument: &str| {
        account_args
            .iter()
            .find_map(|(key, field)| (*key == argument).then(|| String::from(*field)))
            .or_else(|| {
                let camel = snake_to_camel(argument);
                fields
                    .iter()
                    .find(|field| **field == camel)
                    .map(|field| String::from(*field))
            })
    };

    match resolver? {
        crate::account_behavior::BehaviorIdlResolver::AssociatedToken {
            mint,
            owner,
            token_program,
        } => Some(IdlResolver::AssociatedToken {
            mint: account(mint)?,
            owner: account(owner)?,
            token_program: match token_program {
                Some(argument) => Some(account(argument)?),
                None => None,
            },
        }),
    }
}

/// Select the only behavior-defined address recipe for an account field.
///
/// Multiple recipes are ambiguous and indicate conflicting behavior metadata,
/// so IDL generation fails instead of silently picking one.
pub fn one_behavior_resolver<const N: usize>(
    field: &str,
    resolvers: [Option<IdlResolver>; N],
) -> Option<IdlResolver> {
    let mut selected = None;
    for resolver in resolvers.into_iter().flatten() {
        assert!(
            selected.is_none(),
            "idl-build: account field `{field}` has multiple behavior-defined address resolvers"
        );
        selected = Some(resolver);
    }
    selected
}

/// Fragment submitted by `#[account]`; uses a fn pointer to avoid static
/// alloc.
pub struct AccountFragment {
    /// Builds the account and corresponding type definitions.
    pub build: fn() -> (IdlAccountDef, IdlTypeDef),
}

/// Fragment submitted by `#[derive(QuasarSerialize)]` for instruction arg
/// types.
pub struct TypeFragment {
    /// Builds the registered type definition.
    pub build: fn() -> IdlTypeDef,
}

/// Fragment submitted by `#[event]`.
pub struct EventFragment {
    /// Builds the event and corresponding type definitions.
    pub build: fn() -> (IdlEventDef, IdlTypeDef),
}

/// Fragment submitted by `#[error_code]`.
pub struct ErrorFragment {
    /// Builds the registered error definitions.
    pub build: fn() -> Vec<IdlErrorDef>,
}

/// Fragment submitted by `#[program]` for each `#[instruction]`.
pub struct InstructionFragment {
    /// Builds the registered instruction definition.
    pub build: fn() -> IdlInstruction,
    /// Name of the accounts struct used by this instruction (for lookup).
    pub accounts_struct_name: &'static str,
    /// Whether the discriminator was pinned in source or assigned by
    /// `#[program]`.
    pub discriminator_source: InstructionDiscriminatorSource,
}

#[derive(Clone, Copy, PartialEq, Eq)]
/// Records whether an instruction discriminator was generated or source-pinned.
pub enum InstructionDiscriminatorSource {
    /// The program macro assigned the discriminator.
    Auto,
    /// Source code explicitly supplied the discriminator.
    Explicit,
}

/// Fragment submitted by `#[derive(Accounts)]`; carries account metadata for
/// IDL.
pub struct AccountsMetaFragment(pub fn() -> (String, Vec<AccountsMetaEntry>));

/// One entry of an accounts struct's IDL metadata.
///
/// A composite field flattens to `Inner::COUNT` accounts on the wire, but the
/// derive only sees the field's type, not the inner struct's plan. It records
/// the reference and [`build_idl`] splices the inner struct's own entries in.
/// The assembled IDL stays flat, so the wire format is unchanged.
pub enum AccountsMetaEntry {
    /// One account, already fully described.
    Node(IdlAccountNode),
    /// A composite field, resolved against the inner struct's fragment.
    Group {
        /// The composite field's camelCase name, used to prefix inner names.
        field: &'static str,
        /// The inner accounts struct, keyed as its `AccountsMetaFragment`.
        accounts_struct: &'static str,
        /// How many back-to-back copies (`AccountsArray<T, N>` gives `N`).
        repeat: usize,
    },
}

/// Nesting cap for composite flattening. Composites are finite on-chain
/// (`COUNT` is a const), so exceeding this means a fragment referenced itself.
const MAX_GROUP_DEPTH: usize = 16;

fn prefixed_name(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        return String::from(name);
    }
    let mut out = String::from(prefix);
    let mut chars = name.chars();
    if let Some(first) = chars.next() {
        out.extend(first.to_uppercase());
        out.push_str(chars.as_str());
    }
    out
}

/// Re-root every account-relative path in a resolver under `prefix`.
///
/// Instruction-argument paths (`IdlResolver::Arg`, `IdlPdaSeed::Arg`) name args
/// of the enclosing instruction, not accounts, so they are left alone.
fn prefix_resolver_paths(resolver: &mut IdlResolver, prefix: &str) {
    match resolver {
        IdlResolver::Pda { program, seeds } => {
            if let __reexport::IdlPdaProgram::Account { path } = program {
                *path = prefixed_name(prefix, path);
            }
            for seed in seeds {
                match seed {
                    __reexport::IdlPdaSeed::Account { path }
                    | __reexport::IdlPdaSeed::AccountField { path, .. } => {
                        *path = prefixed_name(prefix, path);
                    }
                    __reexport::IdlPdaSeed::Const { .. } | __reexport::IdlPdaSeed::Arg { .. } => {}
                }
            }
        }
        IdlResolver::AssociatedToken {
            mint,
            owner,
            token_program,
        } => {
            *mint = prefixed_name(prefix, mint);
            *owner = prefixed_name(prefix, owner);
            if let Some(token_program) = token_program {
                *token_program = prefixed_name(prefix, token_program);
            }
        }
        IdlResolver::AccountField { account, .. } => {
            *account = prefixed_name(prefix, account);
        }
        IdlResolver::Optional { resolver } => prefix_resolver_paths(resolver, prefix),
        IdlResolver::Input {}
        | IdlResolver::Const { .. }
        | IdlResolver::KnownProgram { .. }
        | IdlResolver::Arg { .. }
        | IdlResolver::Remaining { .. } => {}
    }
}

fn flatten_accounts_meta(
    owner: &str,
    entries: &[AccountsMetaEntry],
    registry: &[(String, Vec<AccountsMetaEntry>)],
    prefix: &str,
    depth: usize,
    out: &mut Vec<IdlAccountNode>,
) {
    assert!(
        depth <= MAX_GROUP_DEPTH,
        "idl-build: accounts struct `{owner}` nests composite groups more than \
         {MAX_GROUP_DEPTH} deep; this is a cycle in the registered fragments"
    );

    for entry in entries {
        match entry {
            AccountsMetaEntry::Node(node) => {
                let mut node = node.clone();
                node.name = prefixed_name(prefix, &node.name);
                prefix_resolver_paths(&mut node.resolver, prefix);
                out.push(node);
            }
            AccountsMetaEntry::Group {
                field,
                accounts_struct,
                repeat,
            } => {
                let (_, inner) = registry
                    .iter()
                    .find(|(name, _)| name == accounts_struct)
                    .unwrap_or_else(|| {
                        panic!(
                            "idl-build: accounts struct `{owner}` has composite field `{field}` \
                             of type `{accounts_struct}` but no AccountsMetaFragment with that \
                             name was registered"
                        )
                    });
                for index in 0..*repeat {
                    let field_prefix = if *repeat == 1 {
                        prefixed_name(prefix, field)
                    } else {
                        prefixed_name(prefix, &alloc::format!("{field}{index}"))
                    };
                    flatten_accounts_meta(
                        accounts_struct,
                        inner,
                        registry,
                        &field_prefix,
                        depth + 1,
                        out,
                    );
                }
            }
        }
    }
}

/// Fragment submitted by `#[derive(Accounts)]`; carries the compiler's
/// resolved validation and lifecycle plan for audit tooling.
pub struct AccountsValidationFragment(pub fn() -> (String, IdlAccountsValidation));

inventory::collect!(AccountFragment);
inventory::collect!(TypeFragment);
inventory::collect!(EventFragment);
inventory::collect!(ErrorFragment);
inventory::collect!(InstructionFragment);
inventory::collect!(AccountsMetaFragment);
inventory::collect!(AccountsValidationFragment);

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
    let mut validation_instructions = alloc::collections::BTreeMap::new();

    // Collect accounts meta fragments into a lookup table.
    let accounts_meta: Vec<(String, Vec<AccountsMetaEntry>)> =
        inventory::iter::<AccountsMetaFragment>
            .into_iter()
            .map(|frag| (frag.0)())
            .collect();
    let accounts_validation: Vec<(String, IdlAccountsValidation)> =
        inventory::iter::<AccountsValidationFragment>
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
            let (_, entries) = accounts_meta
                .iter()
                .find(|(struct_name, _)| struct_name == frag.accounts_struct_name)
                .unwrap_or_else(|| {
                    panic!(
                        "idl-build: instruction `{}` references accounts struct `{}` but no \
                         AccountsMetaFragment with that name was registered",
                        ix.name, frag.accounts_struct_name
                    )
                });
            let mut nodes = Vec::new();
            flatten_accounts_meta(
                frag.accounts_struct_name,
                entries,
                &accounts_meta,
                "",
                0,
                &mut nodes,
            );
            ix.accounts = nodes;
        }
        if !frag.accounts_struct_name.is_empty() {
            let (_, validation) = accounts_validation
                .iter()
                .find(|(struct_name, _)| struct_name == frag.accounts_struct_name)
                .unwrap_or_else(|| {
                    panic!(
                        "idl-build: instruction `{}` references accounts struct `{}` but no \
                         AccountsValidationFragment with that name was registered",
                        ix.name, frag.accounts_struct_name
                    )
                });
            validation_instructions.insert(ix.name.clone(), validation.clone());
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
        extensions: if validation_instructions.is_empty() {
            None
        } else {
            let validation = IdlValidationPlan {
                version: VALIDATION_EXTENSION_VERSION,
                instructions: validation_instructions,
            };
            let mut extensions = serde_json::Map::new();
            extensions.insert(
                String::from(VALIDATION_EXTENSION_KEY),
                serde_json::to_value(validation)
                    .expect("validation plan should serialize into the IDL extension"),
            );
            Some(serde_json::Value::Object(extensions))
        },
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
/// (`checks/discriminator.rs`) and every `#[account]` shares `OWNER =
/// crate::ID`, so a discriminator that is a prefix of another (which includes
/// exact equality) is silent type confusion at runtime. This is the hard-error
/// half of lint rule P009; an empty discriminator (`unsafe_no_disc`) is an
/// explicit opt-out and is skipped. Panics in the
/// missing-fragment/missing-codec style.
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

    #[test]
    fn behavior_resolver_requires_every_declared_recipe_argument() {
        let recipe = Some(
            crate::account_behavior::BehaviorIdlResolver::AssociatedToken {
                mint: "mint",
                owner: "owner",
                token_program: Some("token_program"),
            },
        );
        let incomplete = [("mint", "mint"), ("owner", "wallet")];
        assert!(behavior_resolver(recipe, &incomplete, &[]).is_none());

        // An unmapped declared argument resolves to a same-named field.
        assert!(matches!(
            behavior_resolver(recipe, &incomplete, &["tokenProgram"]),
            Some(IdlResolver::AssociatedToken { .. })
        ));

        let complete = [
            ("mint", "mint"),
            ("owner", "wallet"),
            ("token_program", "tokenProgram"),
        ];
        assert!(matches!(
            behavior_resolver(recipe, &complete, &[]),
            Some(IdlResolver::AssociatedToken {
                mint,
                owner,
                token_program: Some(token_program),
            }) if mint == "mint" && owner == "wallet" && token_program == "tokenProgram"
        ));
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

#[cfg(test)]
mod composite_tests {
    use super::*;

    fn node(name: &str, resolver: IdlResolver) -> AccountsMetaEntry {
        AccountsMetaEntry::Node(IdlAccountNode {
            name: String::from(name),
            optional: false,
            writable: __reexport::AccountFlag::Fixed(false),
            signer: __reexport::AccountFlag::Fixed(false),
            resolver,
            docs: Vec::new(),
        })
    }

    fn flatten(owner: &str, registry: &[(String, Vec<AccountsMetaEntry>)]) -> Vec<IdlAccountNode> {
        let (_, entries) = registry.iter().find(|(name, _)| name == owner).unwrap();
        let mut out = Vec::new();
        flatten_accounts_meta(owner, entries, registry, "", 0, &mut out);
        out
    }

    fn names(nodes: &[IdlAccountNode]) -> Vec<&str> {
        nodes.iter().map(|node| node.name.as_str()).collect()
    }

    #[test]
    fn nested_group_flattens_under_the_field_name() {
        let registry = vec![
            (
                String::from("Inner"),
                vec![
                    node("first", IdlResolver::Input {}),
                    node("second", IdlResolver::Input {}),
                ],
            ),
            (
                String::from("Outer"),
                vec![
                    node("payer", IdlResolver::Input {}),
                    AccountsMetaEntry::Group {
                        field: "pair",
                        accounts_struct: "Inner",
                        repeat: 1,
                    },
                ],
            ),
        ];

        assert_eq!(
            names(&flatten("Outer", &registry)),
            ["payer", "pairFirst", "pairSecond"]
        );
    }

    #[test]
    fn repeated_group_indexes_each_copy() {
        let registry = vec![
            (
                String::from("Inner"),
                vec![node("a", IdlResolver::Input {})],
            ),
            (
                String::from("Outer"),
                vec![AccountsMetaEntry::Group {
                    field: "pairs",
                    accounts_struct: "Inner",
                    repeat: 3,
                }],
            ),
        ];

        assert_eq!(
            names(&flatten("Outer", &registry)),
            ["pairs0A", "pairs1A", "pairs2A"]
        );
    }

    #[test]
    fn inner_account_paths_are_rerooted_but_arg_paths_are_not() {
        let registry = vec![
            (
                String::from("Inner"),
                vec![
                    node("authority", IdlResolver::Input {}),
                    node(
                        "vault",
                        IdlResolver::Pda {
                            program: __reexport::IdlPdaProgram::ProgramId {},
                            seeds: vec![
                                __reexport::IdlPdaSeed::Account {
                                    path: String::from("authority"),
                                },
                                __reexport::IdlPdaSeed::Arg {
                                    path: String::from("seed"),
                                    ty: IdlType::Primitive(String::from("u64")),
                                },
                            ],
                        },
                    ),
                ],
            ),
            (
                String::from("Outer"),
                vec![AccountsMetaEntry::Group {
                    field: "group",
                    accounts_struct: "Inner",
                    repeat: 1,
                }],
            ),
        ];

        let nodes = flatten("Outer", &registry);
        assert_eq!(names(&nodes), ["groupAuthority", "groupVault"]);

        let IdlResolver::Pda { seeds, .. } = &nodes[1].resolver else {
            panic!("expected a pda resolver");
        };
        match &seeds[0] {
            __reexport::IdlPdaSeed::Account { path } => assert_eq!(path, "groupAuthority"),
            other => panic!("expected an account seed, got {other:?}"),
        }
        match &seeds[1] {
            // Instruction args belong to the enclosing instruction, not the group.
            __reexport::IdlPdaSeed::Arg { path, .. } => assert_eq!(path, "seed"),
            other => panic!("expected an arg seed, got {other:?}"),
        }
    }

    #[test]
    #[should_panic(expected = "no AccountsMetaFragment with that name")]
    fn missing_inner_fragment_is_a_hard_error() {
        let registry = vec![(
            String::from("Outer"),
            vec![AccountsMetaEntry::Group {
                field: "pair",
                accounts_struct: "Missing",
                repeat: 1,
            }],
        )];
        flatten("Outer", &registry);
    }
}
