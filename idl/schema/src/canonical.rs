use {
    crate::root::Idl,
    sha2::{Digest, Sha256},
};

/// Serialize an IDL to canonical JSON bytes (deterministic output).
///
/// This is the byte contract used by [`compute_idl_hash`]:
/// - serialize the typed [`Idl`] using its Serde field names and omission
///   rules;
/// - sort every JSON object's keys recursively by Rust [`String`] order;
/// - preserve array order;
/// - emit compact UTF-8 JSON with no insignificant whitespace.
///
/// This deliberately defines a Quasar-producer integrity format, not a
/// language-neutral JSON canonicalization standard such as RFC 8785. External
/// tools should use this crate or `quasar idl verify` when checking
/// `hashes.idl`.
pub fn canonical_json(idl: &Idl) -> serde_json::Result<Vec<u8>> {
    let mut value = serde_json::to_value(idl)?;
    sort_json_objects(&mut value);
    serde_json::to_vec(&value)
}

/// Serialize an IDL in Quasar's stable, human-readable presentation order.
///
/// This output is deterministic for a given typed [`Idl`], but its whitespace
/// and field order are not the byte input to [`compute_idl_hash`].
pub fn canonical_json_pretty(idl: &Idl) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec_pretty(idl)
}

/// Compute the Quasar-producer integrity hash for the complete admitted IDL.
///
/// The hash is lowercase SHA-256 over [`canonical_json`] after omitting the
/// top-level `hashes` field. Every other field admitted by the typed schema,
/// including opaque `extensions`, is in scope. Object insertion order and
/// presentation whitespace are not in scope; array order is.
pub fn compute_idl_hash(idl: &Idl) -> String {
    let mut idl_for_hash = idl.clone();
    idl_for_hash.hashes = None;
    let bytes = canonical_json(&idl_for_hash).expect("IDL serialization should not fail");
    hex_sha256(&bytes)
}

fn sort_json_objects(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                sort_json_objects(value);
            }
        }
        serde_json::Value::Object(object) => {
            let mut entries = std::mem::take(object).into_iter().collect::<Vec<_>>();
            for (_, value) in &mut entries {
                sort_json_objects(value);
            }
            entries.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
            object.extend(entries);
        }
        _ => {}
    }
}

/// Compute the ABI hash (SHA-256 of ABI-affecting subset only).
///
/// ABI hash includes: program name/address, discriminators, instruction
/// args/codecs/layouts, account data types/codecs/layouts, event types, account
/// meta ordering, resolver requirements, and error codes/names.
///
/// ABI hash excludes: schema/package versions, docs and error messages,
/// human-readable space formulae, metadata, opaque type semantics, extension
/// data, and stored hashes.
pub fn compute_abi_hash(idl: &Idl) -> String {
    let bytes = canonical_abi_json(idl).expect("ABI subset serialization should not fail");
    hex_sha256(&bytes)
}

/// Serialize the ABI and generated-client compatibility projection.
///
/// The compact bytes are the exact input to [`compute_abi_hash`]. The
/// projection includes the fields documented there and omits documentation,
/// build metadata, messages, formulas, extensions, and stored hashes.
pub fn canonical_abi_json(idl: &Idl) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec(&extract_abi_subset(idl))
}

/// Serialize the ABI projection in a stable, reviewable presentation format.
///
/// This contains the same typed projection as [`canonical_abi_json`], with
/// insignificant whitespace added for compatibility-baseline diffs.
pub fn canonical_abi_json_pretty(idl: &Idl) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec_pretty(&extract_abi_subset(idl))
}

/// SHA-256 hash as lowercase hex string.
fn hex_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    hex::encode(result)
}

/// Hex encoding without external dep (inline implementation).
mod hex {
    use std::fmt::Write;

    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        let bytes = bytes.as_ref();
        let mut out = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            let _ = write!(out, "{b:02x}");
        }
        out
    }
}

/// Extract the ABI and generated-client compatibility subset for hashing.
///
/// Every projection below destructures its source schema type without `..`.
/// Adding a schema field therefore fails compilation until the field is either
/// mirrored into the ABI shape or explicitly ignored in that destructure.
#[derive(serde::Serialize)]
struct AbiSubset {
    name: String,
    address: String,
    instructions: Vec<AbiInstruction>,
    accounts: Vec<AbiAccount>,
    types: Vec<AbiType>,
    events: Vec<AbiEvent>,
    errors: Vec<AbiError>,
}

#[derive(serde::Serialize)]
struct AbiInstruction {
    name: String,
    discriminator: Vec<u8>,
    args: Vec<AbiArg>,
    accounts: Vec<AbiAccountMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    remaining_accounts: Option<crate::account::IdlRemainingAccounts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    layout: Option<crate::layout::IdlLayout>,
}

#[derive(serde::Serialize)]
struct AbiArg {
    name: String,
    #[serde(rename = "type")]
    ty: crate::types::IdlType,
    #[serde(skip_serializing_if = "Option::is_none")]
    codec: Option<crate::codec::IdlCodec>,
}

#[derive(serde::Serialize)]
struct AbiAccountMeta {
    name: String,
    optional: bool,
    writable: crate::account::AccountFlag,
    signer: crate::account::AccountFlag,
    resolver: crate::account::IdlResolver,
}

#[derive(serde::Serialize)]
struct AbiAccount {
    name: String,
    discriminator: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    space: Option<AbiSpace>,
}

#[derive(serde::Serialize)]
struct AbiType {
    name: String,
    kind: crate::types::IdlTypeDefKind,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    fields: Vec<AbiField>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    variants: Vec<AbiEnumVariant>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    alias: Option<crate::types::IdlType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fallback: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    codec: Option<crate::codec::IdlCodec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    layout: Option<crate::layout::IdlLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    space: Option<AbiSpace>,
}

#[derive(serde::Serialize)]
struct AbiField {
    name: String,
    #[serde(rename = "type")]
    ty: crate::types::IdlType,
    #[serde(skip_serializing_if = "Option::is_none")]
    codec: Option<crate::codec::IdlCodec>,
}

#[derive(serde::Serialize)]
struct AbiEnumVariant {
    name: String,
    value: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    fields: Vec<AbiField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    layout: Option<crate::layout::IdlLayout>,
}

#[derive(serde::Serialize)]
struct AbiEvent {
    name: String,
    discriminator: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ty: Option<crate::types::IdlType>,
}

#[derive(serde::Serialize)]
struct AbiError {
    code: u32,
    name: String,
}

#[derive(serde::Serialize)]
struct AbiSpace {
    #[serde(skip_serializing_if = "Option::is_none")]
    discriminator: Option<usize>,
    min: u64,
    max: Option<u64>,
}

fn extract_abi_subset(idl: &Idl) -> AbiSubset {
    let Idl {
        spec: _,
        name,
        version: _,
        address,
        metadata: _,
        docs: _,
        instructions,
        accounts,
        types,
        events,
        errors,
        extensions: _,
        hashes: _,
    } = idl;

    AbiSubset {
        name: name.clone(),
        address: address.clone(),
        instructions: instructions.iter().map(abi_instruction).collect(),
        accounts: accounts.iter().map(abi_account).collect(),
        types: types.iter().map(abi_type).collect(),
        events: events.iter().map(abi_event).collect(),
        errors: errors.iter().map(abi_error).collect(),
    }
}

fn abi_instruction(instruction: &crate::instruction::IdlInstruction) -> AbiInstruction {
    let crate::instruction::IdlInstruction {
        name,
        discriminator,
        docs: _,
        accounts,
        args,
        layout,
        remaining_accounts,
    } = instruction;

    AbiInstruction {
        name: name.clone(),
        discriminator: discriminator.clone(),
        args: args.iter().map(abi_arg).collect(),
        accounts: accounts.iter().map(abi_account_meta).collect(),
        remaining_accounts: remaining_accounts.clone(),
        layout: layout.clone(),
    }
}

fn abi_arg(arg: &crate::instruction::IdlArg) -> AbiArg {
    let crate::instruction::IdlArg {
        name,
        ty,
        codec,
        docs: _,
    } = arg;

    AbiArg {
        name: name.clone(),
        ty: ty.clone(),
        codec: codec.clone(),
    }
}

fn abi_account_meta(account: &crate::account::IdlAccountNode) -> AbiAccountMeta {
    let crate::account::IdlAccountNode {
        name,
        optional,
        writable,
        signer,
        resolver,
        docs: _,
    } = account;

    AbiAccountMeta {
        name: name.clone(),
        optional: *optional,
        writable: writable.clone(),
        signer: signer.clone(),
        resolver: resolver.clone(),
    }
}

fn abi_account(account: &crate::account::IdlAccountDef) -> AbiAccount {
    let crate::account::IdlAccountDef {
        name,
        discriminator,
        docs: _,
        space,
    } = account;

    AbiAccount {
        name: name.clone(),
        discriminator: discriminator.clone(),
        space: space.as_ref().map(abi_space),
    }
}

fn abi_type(type_def: &crate::types::IdlTypeDef) -> AbiType {
    let crate::types::IdlTypeDef {
        name,
        kind,
        docs: _,
        fields,
        variants,
        repr,
        alias,
        fallback,
        codec,
        layout,
        space,
        semantics: _,
    } = type_def;

    AbiType {
        name: name.clone(),
        kind: *kind,
        fields: abi_fields(fields),
        variants: variants.iter().map(abi_enum_variant).collect(),
        repr: repr.clone(),
        alias: alias.clone(),
        fallback: fallback.clone(),
        codec: codec.clone(),
        layout: layout.clone(),
        space: space.as_ref().map(abi_space),
    }
}

fn abi_fields(fields: &[crate::types::IdlFieldDef]) -> Vec<AbiField> {
    fields
        .iter()
        .map(|field| {
            let crate::types::IdlFieldDef {
                name,
                ty,
                codec,
                docs: _,
            } = field;
            AbiField {
                name: name.clone(),
                ty: ty.clone(),
                codec: codec.clone(),
            }
        })
        .collect()
}

fn abi_enum_variant(variant: &crate::types::IdlEnumVariant) -> AbiEnumVariant {
    let crate::types::IdlEnumVariant {
        name,
        value,
        fields,
        layout,
    } = variant;

    AbiEnumVariant {
        name: name.clone(),
        value: *value,
        fields: abi_fields(fields),
        layout: layout.clone(),
    }
}

fn abi_event(event: &crate::event::IdlEventDef) -> AbiEvent {
    let crate::event::IdlEventDef {
        name,
        discriminator,
        docs: _,
        ty,
    } = event;

    AbiEvent {
        name: name.clone(),
        discriminator: discriminator.clone(),
        ty: ty.clone(),
    }
}

fn abi_error(error: &crate::error::IdlErrorDef) -> AbiError {
    let crate::error::IdlErrorDef { code, name, msg: _ } = error;
    AbiError {
        code: *code,
        name: name.clone(),
    }
}

fn abi_space(space: &crate::space::IdlSpace) -> AbiSpace {
    let crate::space::IdlSpace {
        discriminator,
        min,
        max,
        formula: _,
    } = space;
    AbiSpace {
        discriminator: *discriminator,
        min: *min,
        max: *max,
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            account::{
                AccountFlag, IdlAccountNode, IdlRemainingAccounts, IdlResolver,
                RemainingAccountItem, RemainingAccountPolicy, RemainingAccountsKind,
                RemainingOrder, RemainingPosition,
            },
            codec::{Endian, IdlCodec, ScalarRepr, Storage},
            instruction::IdlInstruction,
            root::{IdlHashes, IdlMetadata},
            types::{IdlFieldDef, IdlType, IdlTypeDef, IdlTypeDefKind},
        },
    };

    fn minimal_idl(
        resolver: IdlResolver,
        remaining_accounts: Option<IdlRemainingAccounts>,
        types: Vec<IdlTypeDef>,
    ) -> Idl {
        Idl {
            spec: "quasar-idl/1.0.0".to_owned(),
            name: "hash_test".to_owned(),
            version: "0.1.0".to_owned(),
            address: "11111111111111111111111111111111".to_owned(),
            metadata: IdlMetadata::default(),
            docs: vec![],
            instructions: vec![IdlInstruction {
                name: "check".to_owned(),
                discriminator: vec![1],
                docs: vec![],
                accounts: vec![IdlAccountNode {
                    name: "authority".to_owned(),
                    optional: false,
                    writable: AccountFlag::Fixed(false),
                    signer: AccountFlag::Fixed(false),
                    resolver,
                    docs: vec![],
                }],
                args: vec![],
                layout: None,
                remaining_accounts,
            }],
            accounts: vec![],
            types,
            events: vec![],
            errors: vec![],
            extensions: None,
            hashes: None,
        }
    }

    fn minimal_idl_with_resolver(resolver: IdlResolver) -> Idl {
        minimal_idl(resolver, None, vec![])
    }

    #[test]
    fn canonical_json_round_trips_byte_for_byte() {
        // serialize -> parse -> serialize must be byte-identical (idempotent),
        // which is what makes committed golden IDLs and hashes stable.
        let idl = minimal_idl_with_resolver(IdlResolver::Input {});
        let first = canonical_json(&idl).expect("serialize");
        let parsed: Idl = serde_json::from_slice(&first).expect("parse");
        let second = canonical_json(&parsed).expect("reserialize");
        assert_eq!(
            first, second,
            "canonical_json must be idempotent across parse"
        );
    }

    #[test]
    fn recomputed_hashes_match_stored_hashes() {
        // Models `quasar idl verify`: hashes are recomputed on the parsed IDL
        // (which carries a populated `hashes` field) and must match the stored
        // values, since the hash excludes the `hashes` field itself.
        let mut idl = minimal_idl_with_resolver(IdlResolver::Input {});
        let idl_hash = compute_idl_hash(&idl);
        let abi_hash = compute_abi_hash(&idl);
        idl.hashes = Some(IdlHashes {
            idl: idl_hash.clone(),
            abi: abi_hash.clone(),
        });
        assert_eq!(compute_idl_hash(&idl), idl_hash);
        assert_eq!(compute_abi_hash(&idl), abi_hash);
    }

    #[test]
    fn abi_hash_changes_when_account_resolver_changes() {
        let input_hash = compute_abi_hash(&minimal_idl_with_resolver(IdlResolver::Input {}));
        let const_hash = compute_abi_hash(&minimal_idl_with_resolver(IdlResolver::Const {
            address: "SysvarRent111111111111111111111111111111111".to_owned(),
        }));

        assert_ne!(input_hash, const_hash);
    }

    #[test]
    fn abi_hash_changes_when_remaining_accounts_change() {
        let base_hash = compute_abi_hash(&minimal_idl_with_resolver(IdlResolver::Input {}));
        let remaining_hash = compute_abi_hash(&minimal_idl(
            IdlResolver::Input {},
            Some(IdlRemainingAccounts {
                kind: RemainingAccountsKind::Append,
                name: "remainingAccounts".to_owned(),
                min: 1,
                max: Some(4),
                item: RemainingAccountItem {
                    client_type: "accountMeta".to_owned(),
                    signer: AccountFlag::Dynamic(crate::account::AccountFlagDynamic::Input),
                    writable: AccountFlag::Fixed(false),
                },
                policy: RemainingAccountPolicy {
                    position: RemainingPosition::AfterDeclaredAccounts,
                    order: RemainingOrder::PreserveInput,
                },
            }),
            vec![],
        ));

        assert_ne!(base_hash, remaining_hash);
    }

    #[test]
    fn abi_hash_changes_when_account_optional_changes() {
        let mut idl = minimal_idl_with_resolver(IdlResolver::Input {});
        let base_hash = compute_abi_hash(&idl);

        idl.instructions[0].accounts[0].optional = true;
        let optional_hash = compute_abi_hash(&idl);

        assert_ne!(
            base_hash, optional_hash,
            "flipping IdlAccountNode.optional must change the ABI hash"
        );
    }

    #[test]
    fn abi_hash_changes_when_type_layout_changes() {
        let fixed_type = type_with_label_codec(Storage::Inline);
        let tail_type = type_with_label_codec(Storage::Tail);

        let fixed_hash =
            compute_abi_hash(&minimal_idl(IdlResolver::Input {}, None, vec![fixed_type]));
        let tail_hash =
            compute_abi_hash(&minimal_idl(IdlResolver::Input {}, None, vec![tail_type]));

        assert_ne!(fixed_hash, tail_hash);
    }

    fn type_with_label_codec(storage: Storage) -> IdlTypeDef {
        IdlTypeDef {
            name: "Config".to_owned(),
            kind: IdlTypeDefKind::Struct,
            docs: vec![],
            fields: vec![IdlFieldDef {
                name: "label".to_owned(),
                ty: IdlType::Primitive("string".to_owned()),
                codec: Some(IdlCodec::SizePrefixed {
                    prefix: ScalarRepr {
                        ty: "u8".to_owned(),
                        endian: Endian::Le,
                    },
                    storage,
                    max_bytes: Some(32),
                    max_items: None,
                    encoding: Some("utf8".to_owned()),
                    item: None,
                }),
                docs: vec![],
            }],
            variants: vec![],
            repr: None,
            alias: None,
            fallback: None,
            codec: None,
            layout: None,
            space: None,
            semantics: None,
        }
    }
}
