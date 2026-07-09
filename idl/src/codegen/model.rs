use {
    crate::types::{
        AccountFlag, Idl, IdlArg, IdlCodec, IdlInstruction, IdlLayout, IdlPdaSeed, IdlResolver,
        IdlType,
    },
    quasar_schema::{camel_to_pascal, camel_to_snake, snake_to_pascal},
};

// ---------------------------------------------------------------------------
// Client IR: the lowered wire model
// ---------------------------------------------------------------------------

/// A value's fully-resolved wire encoding. Every `IdlType` + `IdlCodec` pair is
/// lowered here exactly once; backends consume `WireType` and only render their
/// language spelling, never re-deriving scalar widths, size-prefix widths, or
/// option tags. Resolution is total and explicit: a dynamic type without a
/// codec is an error at IR-build time (see [`WireType::resolve`]), never a
/// silently-guessed prefix width — this is what makes the historical
/// "bare string/vec defaults to u32" divergence unrepresentable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WireType {
    /// One-byte boolean (kept distinct from `Scalar { width: 1 }` so backends
    /// can pick a bool codec/type rather than a `u8`).
    Bool,
    /// Fixed-width integer or float. `width` is the byte width; `float` marks
    /// `f32`/`f64`; `signed` marks the signed integers.
    Scalar { width: u8, signed: bool, float: bool },
    /// 32-byte public key / address (distinct from `FixedBytes(32)` so backends
    /// render an address type/codec rather than a raw byte blob).
    Pubkey,
    /// Opaque variable-length bytes (the IDL `bytes` primitive).
    Bytes,
    /// Fixed-length byte array `[u8; N]`.
    FixedBytes(usize),
    /// Fixed-length array `[T; N]` of a non-byte element.
    Array { len: usize, item: Box<WireType> },
    /// Size-prefixed UTF-8 string. `prefix` is the length-prefix byte width.
    Str { prefix: u8 },
    /// Size-prefixed sequence. `prefix` is the length-prefix byte width.
    List { prefix: u8, item: Box<WireType> },
    /// Optional value with an explicit presence-tag byte width.
    Option { tag: u8, inner: Box<WireType> },
    /// Reference to a named defined type (struct or enum).
    Defined(String),
}

impl WireType {
    /// Lower an `IdlType` + optional `IdlCodec` into a `WireType`.
    ///
    /// Errors (rather than guessing) when a dynamic type (`string`/`vec`) lacks
    /// the size-prefix codec that fixes its length-prefix width — the codec is
    /// mandatory for dynamic types at IDL-build time, so a missing one is a
    /// producer bug, not a client default.
    pub fn resolve(ty: &IdlType, codec: &Option<IdlCodec>) -> Result<Self, String> {
        match ty {
            IdlType::Primitive(p) => Self::resolve_primitive(p, codec),
            IdlType::Option { option } => {
                // Optional dynamic inners (`Option<String>`, `Option<Vec<_>>`)
                // carry the inner's size-prefix codec on the option itself; the
                // presence tag is a single byte. Non-dynamic inners resolve
                // without a codec.
                let inner = if type_is_dynamic(option) {
                    Self::resolve(option, codec)?
                } else {
                    Self::resolve(option, &None)?
                };
                Ok(WireType::Option {
                    tag: 1,
                    inner: Box::new(inner),
                })
            }
            IdlType::Vec { vec } => match codec {
                Some(IdlCodec::SizePrefixed { .. }) => Ok(WireType::List {
                    prefix: codec_prefix_width(codec)?,
                    item: Box::new(Self::resolve(vec, &None)?),
                }),
                _ => Err(format!(
                    "vec type `{ty:?}` requires a size-prefix codec; none was declared"
                )),
            },
            IdlType::Array { array: (inner, size) } => {
                if idl_type_is_byte(inner) {
                    Ok(WireType::FixedBytes(*size))
                } else {
                    Ok(WireType::Array {
                        len: *size,
                        item: Box::new(Self::resolve(inner, &None)?),
                    })
                }
            }
            IdlType::Defined { defined } => Ok(WireType::Defined(defined.name.clone())),
            IdlType::Generic { generic } => Ok(WireType::Defined(generic.clone())),
        }
    }

    fn resolve_primitive(p: &str, codec: &Option<IdlCodec>) -> Result<Self, String> {
        Ok(match p {
            "bool" => WireType::Bool,
            "u8" => WireType::Scalar { width: 1, signed: false, float: false },
            "u16" => WireType::Scalar { width: 2, signed: false, float: false },
            "u32" => WireType::Scalar { width: 4, signed: false, float: false },
            "u64" => WireType::Scalar { width: 8, signed: false, float: false },
            "u128" => WireType::Scalar { width: 16, signed: false, float: false },
            "i8" => WireType::Scalar { width: 1, signed: true, float: false },
            "i16" => WireType::Scalar { width: 2, signed: true, float: false },
            "i32" => WireType::Scalar { width: 4, signed: true, float: false },
            "i64" => WireType::Scalar { width: 8, signed: true, float: false },
            "i128" => WireType::Scalar { width: 16, signed: true, float: false },
            "f32" => WireType::Scalar { width: 4, signed: false, float: true },
            "f64" => WireType::Scalar { width: 8, signed: false, float: true },
            "pubkey" => WireType::Pubkey,
            "bytes" => WireType::Bytes,
            "string" => match codec {
                Some(IdlCodec::SizePrefixed { .. }) => WireType::Str {
                    prefix: codec_prefix_width(codec)?,
                },
                _ => {
                    return Err(String::from(
                        "string type requires a size-prefix codec; none was declared",
                    ))
                }
            },
            other => return Err(format!("unknown primitive type `{other}`")),
        })
    }
}

/// The length-prefix byte width of a `SizePrefixed` codec, as a `u8`.
fn codec_prefix_width(codec: &Option<IdlCodec>) -> Result<u8, String> {
    match codec {
        Some(c @ IdlCodec::SizePrefixed { .. }) => Ok(c.prefix_bytes() as u8),
        _ => Err(String::from("expected a size-prefix codec")),
    }
}

/// Whether an `IdlType` is a dynamic (length-prefixed) type at its top level.
fn type_is_dynamic(ty: &IdlType) -> bool {
    matches!(ty, IdlType::Vec { .. }) || matches!(ty, IdlType::Primitive(p) if p == "string")
}

/// Whether an `IdlType` is the `u8` byte primitive (used to fold `[u8; N]` into
/// `FixedBytes`).
fn idl_type_is_byte(ty: &IdlType) -> bool {
    matches!(ty, IdlType::Primitive(p) if p == "u8")
}

/// One instruction argument (or account/type field), lowered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldPlan {
    /// The field name exactly as it appears in the IDL.
    pub name: String,
    /// The resolved wire encoding.
    pub wire: WireType,
}

/// An account slot in an instruction, with its flags and resolver resolved.
#[derive(Clone, Debug)]
pub struct AccountPlan {
    pub name: String,
    pub writable: AccountFlag,
    pub signer: AccountFlag,
    /// Whether the account is optional. Absent slots carry the program id as a
    /// sentinel per the runtime convention (`emit/parse.rs`), so clients expose
    /// this as an optional parameter.
    pub optional: bool,
    pub resolver: IdlResolver,
}

/// A resolved PDA seed (encoding inferred from the seed's declared type).
#[derive(Clone, Debug)]
pub enum ResolvedSeed {
    Const { value: Vec<u8> },
    AccountAddress { path: String },
    AccountField { account: String, field: String, path: String },
    Arg { path: String, wire: WireType },
}

/// A PDA derivation plan: resolved seeds plus the generated helper name.
#[derive(Clone, Debug)]
pub struct PdaPlan {
    pub field_name: String,
    pub helper_name: String,
    pub seeds: Vec<ResolvedSeed>,
}

/// Whether an instruction's argument payload uses the compact wire layout
/// (inline fixed fields, then all tail length-prefixes, then all tail payloads)
/// or a purely-fixed layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireLayout {
    Fixed,
    Compact,
}

/// A fully-lowered instruction: discriminator bytes, argument split into inline
/// (fixed) and tail (dynamic) fields, accounts, and the wire layout.
#[derive(Clone, Debug)]
pub struct InstructionPlan {
    pub name: String,
    pub disc: Vec<u8>,
    pub inline: Vec<FieldPlan>,
    pub tail: Vec<FieldPlan>,
    pub accounts: Vec<AccountPlan>,
    pub layout: WireLayout,
    pub has_remaining: bool,
}

impl InstructionPlan {
    /// Lower one `IdlInstruction`. Returns an error if any argument's wire type
    /// cannot be resolved (e.g. a codec-less dynamic type).
    pub fn from_instruction(ix: &IdlInstruction) -> Result<Self, String> {
        let compact = matches!(ix.layout, Some(IdlLayout::Compact { .. }));
        let mut inline = Vec::new();
        let mut tail = Vec::new();
        for arg in &ix.args {
            let field = FieldPlan {
                name: arg.name.clone(),
                wire: WireType::resolve(&arg.ty, &arg.codec)
                    .map_err(|e| format!("instruction `{}` arg `{}`: {e}", ix.name, arg.name))?,
            };
            if arg_is_tail(arg) {
                tail.push(field);
            } else {
                inline.push(field);
            }
        }
        let accounts = ix
            .accounts
            .iter()
            .map(|a| AccountPlan {
                name: a.name.clone(),
                writable: a.writable.clone(),
                signer: a.signer.clone(),
                optional: a.optional,
                resolver: a.resolver.clone(),
            })
            .collect();
        let layout = if compact && !tail.is_empty() {
            WireLayout::Compact
        } else {
            WireLayout::Fixed
        };
        Ok(InstructionPlan {
            name: ix.name.clone(),
            disc: ix.discriminator.clone(),
            inline,
            tail,
            accounts,
            layout,
            has_remaining: ix.remaining_accounts.is_some(),
        })
    }
}

/// Whether an argument lands in the tail (dynamic) region: a size-prefixed
/// string/vec, or an optional wrapping one.
fn arg_is_tail(arg: &IdlArg) -> bool {
    matches!(arg.codec, Some(IdlCodec::SizePrefixed { .. }))
        && (type_is_dynamic(&arg.ty) || matches!(&arg.ty, IdlType::Option { option } if type_is_dynamic(option)))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedIdentity {
    pub program_name: String,
    pub crate_name: String,
    pub client_name: String,
    pub typescript_dir: String,
    pub typescript_package: String,
    pub python_package: String,
    pub go_package: String,
    pub rust_client_crate: String,
}

impl ResolvedIdentity {
    pub fn from_idl(idl: &Idl) -> Self {
        let program_name = idl.name.clone();
        let crate_name = idl.metadata.crate_name.clone().unwrap_or_default();
        let client_name = if crate_name.is_empty() {
            program_name.clone()
        } else {
            crate_name.clone()
        };
        let go_package = client_name.replace('-', "_");

        Self {
            program_name,
            crate_name,
            typescript_dir: client_name.clone(),
            typescript_package: format!("{client_name}-client"),
            python_package: client_name.clone(),
            go_package: go_package.clone(),
            rust_client_crate: format!("{client_name}-client"),
            client_name,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProgramFeatures {
    pub has_instructions: bool,
    pub has_accounts: bool,
    pub has_events: bool,
    pub has_types: bool,
    pub has_errors: bool,
    pub has_args: bool,
    pub has_pdas: bool,
    pub has_pda_account_seeds: bool,
    pub has_public_key: bool,
    pub has_option: bool,
    pub has_dynamic: bool,
    pub has_float: bool,
    pub needs_codecs: bool,
}

impl ProgramFeatures {
    pub fn from_idl(idl: &Idl) -> Self {
        let mut features = Self {
            has_instructions: !idl.instructions.is_empty(),
            has_accounts: !idl.accounts.is_empty(),
            has_events: !idl.events.is_empty(),
            has_types: !idl.types.is_empty(),
            has_errors: !idl.errors.is_empty(),
            has_args: idl.instructions.iter().any(|ix| !ix.args.is_empty()),
            has_pdas: idl.instructions.iter().any(|ix| {
                ix.accounts
                    .iter()
                    .any(|account| matches!(account.resolver, IdlResolver::Pda { .. }))
            }),
            has_pda_account_seeds: idl.instructions.iter().any(|ix| {
                ix.accounts.iter().any(|account| {
                    if let IdlResolver::Pda { seeds, .. } = &account.resolver {
                        seeds.iter().any(|seed| {
                            matches!(
                                seed,
                                IdlPdaSeed::Account { .. } | IdlPdaSeed::AccountField { .. }
                            )
                        })
                    } else {
                        false
                    }
                })
            }),
            ..Self::default()
        };

        let mut visit = |ty: &IdlType| {
            if type_has_public_key(ty) {
                features.has_public_key = true;
            }
            if type_has_option(ty) {
                features.has_option = true;
            }
            if type_has_float(ty) {
                features.has_float = true;
            }
        };

        for type_def in &idl.types {
            for field in &type_def.fields {
                visit_type(&field.ty, &mut visit);
            }
        }
        for ix in &idl.instructions {
            for arg in &ix.args {
                visit_type(&arg.ty, &mut visit);
                // Check codec for dynamic fields
                if arg.codec.is_some() {
                    features.has_dynamic = true;
                }
            }
        }

        features.needs_codecs = features.has_types || features.has_args;
        features
    }
}

#[derive(Clone)]
pub struct ProgramModel<'a> {
    pub idl: &'a Idl,
    pub identity: ResolvedIdentity,
    pub features: ProgramFeatures,
}

impl<'a> ProgramModel<'a> {
    pub fn new(idl: &'a Idl) -> Self {
        Self {
            idl,
            identity: ResolvedIdentity::from_idl(idl),
            features: ProgramFeatures::from_idl(idl),
        }
    }
}

pub fn visit_type(ty: &IdlType, visit: &mut impl FnMut(&IdlType)) {
    visit(ty);
    match ty {
        IdlType::Option { option } => visit_type(option, visit),
        IdlType::Vec { vec } => visit_type(vec, visit),
        IdlType::Array { array } => visit_type(&array.0, visit),
        _ => {}
    }
}

pub fn type_has_option(ty: &IdlType) -> bool {
    match ty {
        IdlType::Option { .. } => true,
        IdlType::Vec { vec } => type_has_option(vec),
        IdlType::Array { array } => type_has_option(&array.0),
        _ => false,
    }
}

pub fn type_has_float(ty: &IdlType) -> bool {
    match ty {
        IdlType::Primitive(p) => p == "f32" || p == "f64",
        IdlType::Option { option } => type_has_float(option),
        IdlType::Vec { vec } => type_has_float(vec),
        IdlType::Array { array } => type_has_float(&array.0),
        _ => false,
    }
}

pub fn type_has_public_key(ty: &IdlType) -> bool {
    match ty {
        IdlType::Primitive(p) => p == "pubkey",
        IdlType::Option { option } => type_has_public_key(option),
        IdlType::Vec { vec } => type_has_public_key(vec),
        IdlType::Array { array } => type_has_public_key(&array.0),
        _ => false,
    }
}

pub fn python_field_path(path: &str) -> String {
    path.split('.')
        .map(camel_to_snake)
        .collect::<Vec<_>>()
        .join(".")
}

pub fn go_field_path(path: &str) -> String {
    path.split('.')
        .map(|segment| {
            if segment.contains('_') {
                snake_to_pascal(segment)
            } else {
                camel_to_pascal(segment)
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

#[cfg(test)]
mod tests {
    use {super::*, crate::types::IdlMetadata};

    fn idl_with_names(name: &str, crate_name: &str) -> Idl {
        Idl {
            spec: "quasar-idl/1.0.0".to_string(),
            name: name.to_string(),
            version: "0.1.0".to_string(),
            address: "11111111111111111111111111111111".to_string(),
            metadata: IdlMetadata {
                crate_name: if crate_name.is_empty() {
                    None
                } else {
                    Some(crate_name.to_string())
                },
                ..Default::default()
            },
            docs: vec![],
            instructions: vec![],
            accounts: vec![],
            events: vec![],
            types: vec![],
            errors: vec![],
            extensions: None,
            hashes: None,
        }
    }

    #[test]
    fn resolved_identity_prefers_crate_name_when_present() {
        let idl = idl_with_names("multisig", "quasar-multisig");
        let identity = ResolvedIdentity::from_idl(&idl);

        assert_eq!(identity.client_name, "quasar-multisig");
        assert_eq!(identity.typescript_dir, "quasar-multisig");
        assert_eq!(identity.typescript_package, "quasar-multisig-client");
        assert_eq!(identity.python_package, "quasar-multisig");
        assert_eq!(identity.go_package, "quasar_multisig");
        assert_eq!(identity.rust_client_crate, "quasar-multisig-client");
    }

    #[test]
    fn resolved_identity_falls_back_to_program_name_when_crate_name_missing() {
        let idl = idl_with_names("vault", "");
        let identity = ResolvedIdentity::from_idl(&idl);

        assert_eq!(identity.client_name, "vault");
        assert_eq!(identity.typescript_package, "vault-client");
        assert_eq!(identity.go_package, "vault");
    }

    #[test]
    fn path_lowering_matches_generated_field_conventions() {
        assert_eq!(
            python_field_path("walletConfig.approvalThreshold"),
            "wallet_config.approval_threshold"
        );
        assert_eq!(
            go_field_path("walletConfig.approval_threshold"),
            "WalletConfig.ApprovalThreshold"
        );
    }
}

#[cfg(test)]
mod wire_tests {
    use super::*;
    use crate::types::{Endian, IdlArg, IdlDefinedRef, IdlInstruction, ScalarRepr, Storage};

    fn prim(name: &str) -> IdlType {
        IdlType::Primitive(name.to_string())
    }

    fn str_codec(prefix: &str) -> IdlCodec {
        IdlCodec::SizePrefixed {
            prefix: ScalarRepr {
                ty: prefix.to_string(),
                endian: Endian::Le,
            },
            storage: Storage::Tail,
            max_bytes: Some(32),
            max_items: None,
            encoding: Some("utf8".to_string()),
            item: None,
        }
    }

    fn vec_codec(prefix: &str) -> IdlCodec {
        IdlCodec::SizePrefixed {
            prefix: ScalarRepr {
                ty: prefix.to_string(),
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
    fn scalars_resolve_widths_and_signs() {
        assert_eq!(
            WireType::resolve(&prim("u64"), &None).unwrap(),
            WireType::Scalar { width: 8, signed: false, float: false }
        );
        assert_eq!(
            WireType::resolve(&prim("i32"), &None).unwrap(),
            WireType::Scalar { width: 4, signed: true, float: false }
        );
        assert_eq!(
            WireType::resolve(&prim("f64"), &None).unwrap(),
            WireType::Scalar { width: 8, signed: false, float: true }
        );
        assert_eq!(WireType::resolve(&prim("bool"), &None).unwrap(), WireType::Bool);
        assert_eq!(WireType::resolve(&prim("pubkey"), &None).unwrap(), WireType::Pubkey);
    }

    #[test]
    fn dynamic_types_take_prefix_from_codec() {
        assert_eq!(
            WireType::resolve(&prim("string"), &Some(str_codec("u8"))).unwrap(),
            WireType::Str { prefix: 1 }
        );
        let v = IdlType::Vec { vec: Box::new(prim("pubkey")) };
        assert_eq!(
            WireType::resolve(&v, &Some(vec_codec("u16"))).unwrap(),
            WireType::List { prefix: 2, item: Box::new(WireType::Pubkey) }
        );
    }

    #[test]
    fn codec_less_dynamic_is_an_error_not_a_default() {
        assert!(WireType::resolve(&prim("string"), &None).is_err());
        let v = IdlType::Vec { vec: Box::new(prim("u64")) };
        assert!(WireType::resolve(&v, &None).is_err());
    }

    #[test]
    fn optional_dynamic_gets_one_byte_tag_and_inner_prefix() {
        let opt = IdlType::Option { option: Box::new(prim("string")) };
        assert_eq!(
            WireType::resolve(&opt, &Some(str_codec("u8"))).unwrap(),
            WireType::Option { tag: 1, inner: Box::new(WireType::Str { prefix: 1 }) }
        );
    }

    #[test]
    fn optional_scalar_resolves_inner_without_codec() {
        let opt = IdlType::Option { option: Box::new(prim("u64")) };
        assert_eq!(
            WireType::resolve(&opt, &None).unwrap(),
            WireType::Option {
                tag: 1,
                inner: Box::new(WireType::Scalar { width: 8, signed: false, float: false })
            }
        );
    }

    #[test]
    fn byte_arrays_fold_to_fixed_bytes_but_typed_arrays_do_not() {
        let bytes = IdlType::Array { array: (Box::new(prim("u8")), 32) };
        assert_eq!(WireType::resolve(&bytes, &None).unwrap(), WireType::FixedBytes(32));
        let typed = IdlType::Array { array: (Box::new(prim("u64")), 4) };
        assert_eq!(
            WireType::resolve(&typed, &None).unwrap(),
            WireType::Array {
                len: 4,
                item: Box::new(WireType::Scalar { width: 8, signed: false, float: false })
            }
        );
    }

    #[test]
    fn defined_types_carry_their_name() {
        let d = IdlType::Defined { defined: IdlDefinedRef { name: "Foo".to_string(), generics: vec![] } };
        assert_eq!(WireType::resolve(&d, &None).unwrap(), WireType::Defined("Foo".to_string()));
    }

    #[test]
    fn instruction_plan_splits_inline_and_tail_in_compact_layout() {
        use crate::types::{CompactWire, IdlLayout};
        let ix = IdlInstruction {
            name: "submit".to_string(),
            discriminator: vec![7],
            docs: vec![],
            accounts: vec![],
            args: vec![
                IdlArg { name: "tag".to_string(), ty: prim("u64"), codec: None, docs: vec![] },
                IdlArg {
                    name: "label".to_string(),
                    ty: prim("string"),
                    codec: Some(str_codec("u8")),
                    docs: vec![],
                },
            ],
            layout: Some(IdlLayout::Compact {
                inline_fields: vec!["tag".to_string()],
                tail_fields: vec!["label".to_string()],
                wire: CompactWire::InlineFieldsThenTailHeadersThenTailPayloads,
            }),
            remaining_accounts: None,
        };
        let plan = InstructionPlan::from_instruction(&ix).unwrap();
        assert_eq!(plan.layout, WireLayout::Compact);
        assert_eq!(plan.inline.len(), 1);
        assert_eq!(plan.inline[0].name, "tag");
        assert_eq!(plan.tail.len(), 1);
        assert_eq!(plan.tail[0].name, "label");
        assert_eq!(plan.tail[0].wire, WireType::Str { prefix: 1 });
    }
}
