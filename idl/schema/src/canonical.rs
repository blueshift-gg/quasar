use {
    crate::root::Idl,
    sha2::{Digest, Sha256},
};

/// Serialize an IDL to canonical JSON bytes (deterministic output).
///
/// Canonical JSON rules:
/// - Struct fields serialize in declaration order (serde default).
/// - BTreeMap keys serialize in sorted order (BTreeMap guarantee).
/// - No trailing whitespace, compact format.
pub fn canonical_json(idl: &Idl) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec(idl)
}

/// Serialize an IDL to canonical pretty-printed JSON (for human-readable
/// output).
pub fn canonical_json_pretty(idl: &Idl) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec_pretty(idl)
}

/// Compute the full IDL hash (SHA-256 of canonical JSON, excluding `hashes`
/// field).
pub fn compute_idl_hash(idl: &Idl) -> String {
    let mut idl_for_hash = idl.clone();
    idl_for_hash.hashes = None;
    let bytes = canonical_json(&idl_for_hash).expect("IDL serialization should not fail");
    hex_sha256(&bytes)
}

/// Compute the ABI hash (SHA-256 of ABI-affecting subset only).
///
/// ABI hash includes: address, discriminators, instruction args/codecs/layouts,
/// account data types/codecs/layouts, event types, account meta ordering,
/// resolver requirements.
///
/// ABI hash excludes: docs, source spans, metadata, non-ABI extension data.
pub fn compute_abi_hash(idl: &Idl) -> String {
    let abi_subset = extract_abi_subset(idl);
    let bytes = serde_json::to_vec(&abi_subset).expect("ABI subset serialization should not fail");
    hex_sha256(&bytes)
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

/// Extract the ABI-affecting subset for hashing.
/// This is a simplified representation that captures only ABI-relevant fields.
#[derive(serde::Serialize)]
struct AbiSubset {
    address: String,
    instructions: Vec<AbiInstruction>,
    accounts: Vec<AbiAccount>,
    types: Vec<AbiType>,
    events: Vec<AbiEvent>,
}

#[derive(serde::Serialize)]
struct AbiInstruction {
    name: String,
    discriminator: Vec<u8>,
    args: Vec<crate::instruction::IdlArg>,
    accounts: Vec<AbiAccountMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    remaining_accounts: Option<crate::account::IdlRemainingAccounts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    layout: Option<crate::layout::IdlLayout>,
}

#[derive(serde::Serialize)]
struct AbiAccountMeta {
    name: String,
    optional: bool,
    writable: crate::account::AccountFlag,
    signer: crate::account::AccountFlag,
    resolver: crate::account::IdlResolver,
}

/// Serde fields of [`crate::account::IdlAccountNode`] that are deliberately
/// excluded from the ABI hash (they carry no wire/ABI meaning). The
/// completeness test asserts every serde field is either mirrored in
/// [`AbiAccountMeta`] or listed here, so a newly added field cannot silently
/// skip the hash.
#[cfg(test)]
const ABI_WAIVED: &[&str] = &["docs"];

#[derive(serde::Serialize)]
struct AbiAccount {
    name: String,
    discriminator: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    space: Option<crate::space::IdlSpace>,
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
    space: Option<crate::space::IdlSpace>,
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

fn extract_abi_subset(idl: &Idl) -> AbiSubset {
    AbiSubset {
        address: idl.address.clone(),
        instructions: idl
            .instructions
            .iter()
            .map(|ix| AbiInstruction {
                name: ix.name.clone(),
                discriminator: ix.discriminator.clone(),
                args: ix.args.clone(),
                accounts: ix
                    .accounts
                    .iter()
                    .map(|a| AbiAccountMeta {
                        name: a.name.clone(),
                        optional: a.optional,
                        writable: a.writable.clone(),
                        signer: a.signer.clone(),
                        resolver: a.resolver.clone(),
                    })
                    .collect(),
                remaining_accounts: ix.remaining_accounts.clone(),
                layout: ix.layout.clone(),
            })
            .collect(),
        accounts: idl
            .accounts
            .iter()
            .map(|a| AbiAccount {
                name: a.name.clone(),
                discriminator: a.discriminator.clone(),
                space: a.space.clone(),
            })
            .collect(),
        types: idl
            .types
            .iter()
            .map(|t| AbiType {
                name: t.name.clone(),
                kind: t.kind,
                fields: abi_fields(&t.fields),
                variants: t
                    .variants
                    .iter()
                    .map(|v| AbiEnumVariant {
                        name: v.name.clone(),
                        value: v.value,
                        fields: abi_fields(&v.fields),
                        layout: v.layout.clone(),
                    })
                    .collect(),
                repr: t.repr.clone(),
                alias: t.alias.clone(),
                fallback: t.fallback.clone(),
                codec: t.codec.clone(),
                layout: t.layout.clone(),
                space: t.space.clone(),
            })
            .collect(),
        events: idl
            .events
            .iter()
            .map(|e| AbiEvent {
                name: e.name.clone(),
                discriminator: e.discriminator.clone(),
                ty: e.ty.clone(),
            })
            .collect(),
    }
}

fn abi_fields(fields: &[crate::types::IdlFieldDef]) -> Vec<AbiField> {
    fields
        .iter()
        .map(|f| AbiField {
            name: f.name.clone(),
            ty: f.ty.clone(),
            codec: f.codec.clone(),
        })
        .collect()
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

    /// Guard: every serde-serialized field of `IdlAccountNode` must be mirrored
    /// in the ABI subset (`AbiAccountMeta`) or explicitly waived in
    /// `ABI_WAIVED`. Without this, a newly added account-meta field would
    /// silently fall out of the ABI hash and break compatibility detection.
    #[test]
    fn abi_account_meta_covers_all_idl_account_node_fields() {
        // Fully populate every optional/skippable field so none are omitted
        // from the serialized key set.
        let node = IdlAccountNode {
            name: "authority".to_owned(),
            optional: true,
            writable: AccountFlag::Fixed(true),
            signer: AccountFlag::Fixed(true),
            resolver: IdlResolver::Input {},
            docs: vec!["doc".to_owned()],
        };
        let node_value = serde_json::to_value(&node).expect("node serializes");
        let node_keys = node_value
            .as_object()
            .expect("node is a JSON object")
            .keys()
            .cloned()
            .collect::<Vec<_>>();

        let meta = AbiAccountMeta {
            name: "authority".to_owned(),
            optional: true,
            writable: AccountFlag::Fixed(true),
            signer: AccountFlag::Fixed(true),
            resolver: IdlResolver::Input {},
        };
        let meta_value = serde_json::to_value(&meta).expect("meta serializes");
        let meta_keys = meta_value
            .as_object()
            .expect("meta is a JSON object")
            .keys()
            .cloned()
            .collect::<std::collections::HashSet<_>>();

        for key in node_keys {
            let covered = meta_keys.contains(&key) || ABI_WAIVED.contains(&key.as_str());
            assert!(
                covered,
                "IdlAccountNode field `{key}` is neither in the ABI subset (AbiAccountMeta) nor \
                 in ABI_WAIVED; the ABI hash would silently ignore it. Add it to AbiAccountMeta \
                 or ABI_WAIVED.",
            );
        }
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
