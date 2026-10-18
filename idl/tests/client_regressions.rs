//! Targeted assertions retained from the pre-rewrite client smoke tests.
//! Compact layouts and PDA resolvers remain public-schema features; the runtime
//! adapter uses Borsh and explicit account inputs instead.
use quasar_idl::{
    codegen::{c, golang, python, rust, typescript},
    types::{Idl, IdlPdaSeed, IdlResolver, IdlType},
};

fn fixture() -> Idl {
    quasar_idl::read(include_str!("fixtures/client-regressions.json")).unwrap()
}

fn contains(source: &str, expected: &[&str]) {
    for text in expected {
        assert!(source.contains(text), "missing generated fragment: {text}");
    }
}

#[test]
fn typescript_fixed_byte_arrays_use_sized_codecs() {
    let idl = fixture();
    for source in [typescript::generate_ts_client(&idl), typescript::generate_ts_client_kit(&idl)] {
        contains(&source, &["fixCodecSize(getBytesCodec(), 32)"]);
        assert!(!source.contains("/* unknown: bytes */"));
    }
}

#[test]
fn typescript_numeric_vectors_use_builtin_types_and_codecs() {
    // PodU64 became u64 in the rewrite; the public IDL still describes u64.
    let idl = fixture();
    for source in [typescript::generate_ts_client(&idl), typescript::generate_ts_client_kit(&idl)] {
        contains(
            &source,
            &["nums: Array<bigint>;", "getArrayCodec(getU64Codec(), { size: input.nums.length })"],
        );
        assert!(!source.contains("PodU64Codec"));
        assert!(!source.contains("Array<PodU64>"));
    }
}

#[test]
fn kit_plugin_exposes_only_supported_accounts_and_instructions() {
    let kit = typescript::generate_ts_client_kit(&fixture());
    contains(
        &kit,
        &[
            "from \"@solana/kit/program-client-core\"",
            "export function clientRegressionsProgram()",
            "staticAccount: addSelfFetchFunctions(client, StaticAccountCodec),",
            "simple: (input: SimpleInstructionInput) => addSelfPlanAndSendFunctions(client, __client.createSimpleInstruction(input)),",
            "createResolverHeavyInstruction(",
            "resolver: AccountDataResolver",
        ],
    );
    assert!(!kit.contains("dynamicAccount: addSelfFetchFunctions"));
    assert!(!kit.contains("resolverHeavy:"));
}

#[test]
fn account_field_pda_seeds_preserve_source_type_and_require_resolution() {
    let idl = fixture();
    let ix = idl.instructions.iter().find(|ix| ix.name == "resolverHeavy").unwrap();
    let IdlResolver::Pda { seeds, .. } = &ix.accounts[1].resolver else {
        panic!("PDA resolver lost")
    };
    assert!(matches!(&seeds[0], IdlPdaSeed::AccountField { path, account, field }
        if path == "config" && account == "NamespaceConfig" && field == "namespace"));
    let namespace = idl.types.iter().find(|ty| ty.name == "NamespaceConfig").unwrap();
    assert_eq!(namespace.fields[0].ty, IdlType::Primitive("u32".into()));
    for source in [typescript::generate_ts_client(&idl), typescript::generate_ts_client_kit(&idl)] {
        contains(&source, &["scopedItem", "NamespaceConfigCodec", "getU32Codec().encode("]);
        assert!(!source.contains("scopedItem: Address;"));
        assert!(!source.contains(" :: seeds("));
    }
    contains(&c::generate_c_client(&idl), &["config_namespace_seed"]);
}

#[test]
fn optional_dynamic_args_keep_tags_and_payloads_in_all_clients() {
    let idl = fixture();
    let files = rust::generate_client(&idl);
    let source = &files.iter().find(|(path, _)| path == "instructions/submit.rs").unwrap().1;
    contains(
        source,
        &[
            "pub maybe_name: Option<DynString<u8>>",
            "pub maybe_addrs: Option<DynVec<Address, u16>>",
            "data.push(u8::from(ix.maybe_name.is_some()))",
            "data.push(u8::from(ix.maybe_addrs.is_some()))",
        ],
    );
    for source in [typescript::generate_ts_client(&idl), typescript::generate_ts_client_kit(&idl)] {
        contains(
            &source,
            &[
                "getU8Codec().encode(input.maybe_name === null ? 0 : 1)",
                "getU8Codec().encode(input.maybe_addrs === null ? 0 : 1)",
                "...maybe_nameTag",
                "...maybe_addrsTag",
                "...maybe_nameBytes",
                "...maybe_addrsBytes",
            ],
        );
    }
    contains(
        &python::generate_python_client(&idl),
        &[
            "maybe_name: Optional[str]",
            "maybe_addrs: Optional[list[Pubkey]]",
            "data.append(0 if input.maybe_name is None else 1)",
            "data.append(0 if input.maybe_addrs is None else 1)",
            "if input.maybe_name is not None:",
            "if input.maybe_addrs is not None:",
        ],
    );
    contains(
        &golang::generate_go_client(&idl),
        &[
            "MaybeName *string",
            "MaybeAddrs *[]solana.PublicKey",
            "if input.MaybeName == nil",
            "if input.MaybeAddrs == nil",
            "_MaybeNameBytes := []byte(*input.MaybeName)",
            "for _, item := range *input.MaybeAddrs",
        ],
    );
    contains(
        &c::generate_c_client(&idl),
        &[
            "bool maybe_name_present;",
            "const uint8_t *maybe_name;",
            "bool maybe_addrs_present;",
            "const Pubkey *maybe_addrs;",
            "data_buf[off++] = args->maybe_name_present ? 1 : 0;",
            "if (args->maybe_name_present)",
            "data_buf[off++] = args->maybe_addrs_present ? 1 : 0;",
            "if (args->maybe_addrs_present)",
        ],
    );
}
