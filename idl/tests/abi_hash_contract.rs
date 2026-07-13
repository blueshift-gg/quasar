use quasar_idl::types::{
    canonical_abi_json, compute_abi_hash, AccountFlag, Idl, IdlHashes, IdlResolver, IdlType,
    IdlTypeDefKind,
};

const COMPLETE_IDL: &str = r#"{
    "spec": "quasar-idl/1.1.0",
    "name": "abi_fixture",
    "version": "0.1.0",
    "address": "11111111111111111111111111111111",
    "metadata": { "crateName": "abi-fixture" },
    "docs": ["program docs"],
    "instructions": [{
        "name": "set_value",
        "discriminator": [1],
        "docs": ["instruction docs"],
        "accounts": [{
            "name": "authority",
            "optional": true,
            "writable": true,
            "signer": "input",
            "resolver": {
                "kind": "const",
                "address": "SysvarRent111111111111111111111111111111111"
            },
            "docs": ["account meta docs"]
        }],
        "args": [{
            "name": "value",
            "type": "u16",
            "codec": { "kind": "scalar", "format": "u16", "endian": "le", "size": 2 },
            "docs": ["argument docs"]
        }],
        "layout": { "kind": "fixed", "fields": ["value"] },
        "remainingAccounts": {
            "kind": "append",
            "name": "remainingAccounts",
            "min": 1,
            "max": 4,
            "item": { "clientType": "accountMeta", "signer": "input", "writable": true },
            "policy": { "position": "afterDeclaredAccounts", "order": "preserveInput" }
        }
    }],
    "accounts": [{
        "name": "Config",
        "discriminator": [2],
        "docs": ["account docs"],
        "space": { "discriminator": 1, "min": 2, "max": 64, "formula": "1 + payload" }
    }],
    "types": [{
        "name": "Value",
        "kind": "struct",
        "docs": ["type docs"],
        "fields": [{
            "name": "value",
            "type": "u16",
            "codec": { "kind": "scalar", "format": "u16", "endian": "le", "size": 2 },
            "docs": ["field docs"]
        }],
        "variants": [{
            "name": "Only",
            "value": 1,
            "fields": [{
                "name": "inner",
                "type": "u8",
                "codec": { "kind": "scalar", "format": "u8", "endian": "le", "size": 1 },
                "docs": ["variant field docs"]
            }],
            "layout": { "kind": "fixed", "fields": ["inner"] }
        }],
        "repr": "u8",
        "alias": "u8",
        "fallback": "opaque",
        "codec": { "kind": "scalar", "format": "u16", "endian": "le", "size": 2 },
        "layout": { "kind": "fixed", "fields": ["value"] },
        "space": { "discriminator": 0, "min": 2, "max": 2, "formula": "two bytes" },
        "semantics": { "vendor": { "mode": "test" } }
    }],
    "events": [{
        "name": "ValueChanged",
        "discriminator": [3],
        "docs": ["event docs"],
        "type": "u16"
    }],
    "errors": [{ "code": 6000, "name": "InvalidValue", "msg": "invalid value" }],
    "extensions": { "vendor": { "enabled": true } }
}"#;

const COMPLETE_ABI_HASH: &str = "ea5a3ae9d4cb2b392f979a1c97e16bcfb47f2165db68ea436359c099bc25de87";

fn fixture() -> Idl {
    serde_json::from_str(COMPLETE_IDL).expect("complete ABI fixture must parse")
}

fn assert_abi_changes(label: &str, base: &Idl, mutate: impl FnOnce(&mut Idl)) {
    let base_hash = compute_abi_hash(base);
    let base_projection = canonical_abi_json(base).unwrap();
    let mut changed = base.clone();
    mutate(&mut changed);
    assert_ne!(
        compute_abi_hash(&changed),
        base_hash,
        "`{label}` is part of the ABI/client interface"
    );
    assert_ne!(
        canonical_abi_json(&changed).unwrap(),
        base_projection,
        "`{label}` must appear in the reviewable ABI projection"
    );
}

fn assert_abi_unchanged(label: &str, base: &Idl, mutate: impl FnOnce(&mut Idl)) {
    let base_hash = compute_abi_hash(base);
    let base_projection = canonical_abi_json(base).unwrap();
    let mut changed = base.clone();
    mutate(&mut changed);
    assert_eq!(
        compute_abi_hash(&changed),
        base_hash,
        "`{label}` is outside the ABI/client interface"
    );
    assert_eq!(
        canonical_abi_json(&changed).unwrap(),
        base_projection,
        "`{label}` must stay outside the reviewable ABI projection"
    );
}

#[test]
fn complete_abi_shape_has_a_golden_hash() {
    assert_eq!(compute_abi_hash(&fixture()), COMPLETE_ABI_HASH);
}

#[test]
fn documentation_metadata_and_extensions_do_not_change_the_abi_hash() {
    let base = fixture();

    assert_abi_unchanged("spec", &base, |idl| idl.spec = "quasar-idl/1.2.0".into());
    assert_abi_unchanged("version", &base, |idl| idl.version = "0.2.0".into());
    assert_abi_unchanged("metadata", &base, |idl| {
        idl.metadata.crate_name = Some("changed".into());
    });
    assert_abi_unchanged("program docs", &base, |idl| idl.docs.push("changed".into()));
    assert_abi_unchanged("instruction docs", &base, |idl| {
        idl.instructions[0].docs.push("changed".into());
    });
    assert_abi_unchanged("argument docs", &base, |idl| {
        idl.instructions[0].args[0].docs.push("changed".into());
    });
    assert_abi_unchanged("account meta docs", &base, |idl| {
        idl.instructions[0].accounts[0].docs.push("changed".into());
    });
    assert_abi_unchanged("account docs", &base, |idl| {
        idl.accounts[0].docs.push("changed".into());
    });
    assert_abi_unchanged("account space formula", &base, |idl| {
        idl.accounts[0].space.as_mut().unwrap().formula = Some("changed".into());
    });
    assert_abi_unchanged("type docs", &base, |idl| {
        idl.types[0].docs.push("changed".into());
    });
    assert_abi_unchanged("field docs", &base, |idl| {
        idl.types[0].fields[0].docs.push("changed".into());
    });
    assert_abi_unchanged("variant field docs", &base, |idl| {
        idl.types[0].variants[0].fields[0]
            .docs
            .push("changed".into());
    });
    assert_abi_unchanged("type semantics", &base, |idl| {
        idl.types[0].semantics = Some(serde_json::json!({ "changed": true }));
    });
    assert_abi_unchanged("type space formula", &base, |idl| {
        idl.types[0].space.as_mut().unwrap().formula = Some("changed".into());
    });
    assert_abi_unchanged("event docs", &base, |idl| {
        idl.events[0].docs.push("changed".into());
    });
    assert_abi_unchanged("error message", &base, |idl| {
        idl.errors[0].msg = Some("changed".into());
    });
    assert_abi_unchanged("extensions", &base, |idl| {
        idl.extensions = Some(serde_json::json!({ "changed": true }));
    });
    assert_abi_unchanged("stored hashes", &base, |idl| {
        idl.hashes = Some(IdlHashes {
            idl: "changed".into(),
            abi: "changed".into(),
        });
    });
}

#[test]
fn every_enumerated_wire_and_client_field_changes_the_abi_hash() {
    let base = fixture();

    assert_abi_changes("program name", &base, |idl| idl.name.push_str("_v2"));
    assert_abi_changes("program address", &base, |idl| idl.address.push('2'));
    assert_abi_changes("instruction name", &base, |idl| {
        idl.instructions[0].name.push_str("_v2");
    });
    assert_abi_changes("instruction discriminator", &base, |idl| {
        idl.instructions[0].discriminator[0] = 9;
    });
    assert_abi_changes("argument name", &base, |idl| {
        idl.instructions[0].args[0].name.push_str("_v2");
    });
    assert_abi_changes("argument type", &base, |idl| {
        idl.instructions[0].args[0].ty = IdlType::Primitive("u32".into());
    });
    assert_abi_changes("argument codec", &base, |idl| {
        idl.instructions[0].args[0].codec = None;
    });
    assert_abi_changes("account meta name", &base, |idl| {
        idl.instructions[0].accounts[0].name.push_str("_v2");
    });
    assert_abi_changes("account optional", &base, |idl| {
        idl.instructions[0].accounts[0].optional = false;
    });
    assert_abi_changes("account writable", &base, |idl| {
        idl.instructions[0].accounts[0].writable = AccountFlag::Fixed(false);
    });
    assert_abi_changes("account signer", &base, |idl| {
        idl.instructions[0].accounts[0].signer = AccountFlag::Fixed(false);
    });
    assert_abi_changes("account resolver", &base, |idl| {
        idl.instructions[0].accounts[0].resolver = IdlResolver::Input {};
    });
    assert_abi_changes("instruction layout", &base, |idl| {
        idl.instructions[0].layout = None;
    });
    assert_abi_changes("remaining name", &base, |idl| {
        idl.instructions[0]
            .remaining_accounts
            .as_mut()
            .unwrap()
            .name
            .push_str("_v2");
    });
    assert_abi_changes("remaining min", &base, |idl| {
        idl.instructions[0].remaining_accounts.as_mut().unwrap().min = 2;
    });
    assert_abi_changes("remaining max", &base, |idl| {
        idl.instructions[0].remaining_accounts.as_mut().unwrap().max = Some(5);
    });
    assert_abi_changes("remaining client type", &base, |idl| {
        idl.instructions[0]
            .remaining_accounts
            .as_mut()
            .unwrap()
            .item
            .client_type
            .push_str("V2");
    });
    assert_abi_changes("remaining signer", &base, |idl| {
        idl.instructions[0]
            .remaining_accounts
            .as_mut()
            .unwrap()
            .item
            .signer = AccountFlag::Fixed(false);
    });
    assert_abi_changes("remaining writable", &base, |idl| {
        idl.instructions[0]
            .remaining_accounts
            .as_mut()
            .unwrap()
            .item
            .writable = AccountFlag::Fixed(false);
    });
    assert_abi_changes("account definition name", &base, |idl| {
        idl.accounts[0].name.push_str("V2");
    });
    assert_abi_changes("account discriminator", &base, |idl| {
        idl.accounts[0].discriminator[0] = 9;
    });
    assert_abi_changes("account space discriminator", &base, |idl| {
        idl.accounts[0].space.as_mut().unwrap().discriminator = Some(2);
    });
    assert_abi_changes("account minimum space", &base, |idl| {
        idl.accounts[0].space.as_mut().unwrap().min = 3;
    });
    assert_abi_changes("account maximum space", &base, |idl| {
        idl.accounts[0].space.as_mut().unwrap().max = Some(65);
    });
    assert_abi_changes("type name", &base, |idl| idl.types[0].name.push_str("V2"));
    assert_abi_changes("type kind", &base, |idl| {
        idl.types[0].kind = IdlTypeDefKind::Alias;
    });
    assert_abi_changes("field name", &base, |idl| {
        idl.types[0].fields[0].name.push_str("V2");
    });
    assert_abi_changes("field type", &base, |idl| {
        idl.types[0].fields[0].ty = IdlType::Primitive("u32".into());
    });
    assert_abi_changes("field codec", &base, |idl| {
        idl.types[0].fields[0].codec = None;
    });
    assert_abi_changes("variant name", &base, |idl| {
        idl.types[0].variants[0].name.push_str("V2");
    });
    assert_abi_changes("variant value", &base, |idl| {
        idl.types[0].variants[0].value = 2;
    });
    assert_abi_changes("variant field name", &base, |idl| {
        idl.types[0].variants[0].fields[0].name.push_str("V2");
    });
    assert_abi_changes("variant field type", &base, |idl| {
        idl.types[0].variants[0].fields[0].ty = IdlType::Primitive("u16".into());
    });
    assert_abi_changes("variant field codec", &base, |idl| {
        idl.types[0].variants[0].fields[0].codec = None;
    });
    assert_abi_changes("variant layout", &base, |idl| {
        idl.types[0].variants[0].layout = None;
    });
    assert_abi_changes("type repr", &base, |idl| idl.types[0].repr = None);
    assert_abi_changes("type alias", &base, |idl| idl.types[0].alias = None);
    assert_abi_changes("type fallback", &base, |idl| idl.types[0].fallback = None);
    assert_abi_changes("type codec", &base, |idl| idl.types[0].codec = None);
    assert_abi_changes("type layout", &base, |idl| idl.types[0].layout = None);
    assert_abi_changes("type space discriminator", &base, |idl| {
        idl.types[0].space.as_mut().unwrap().discriminator = Some(1);
    });
    assert_abi_changes("type minimum space", &base, |idl| {
        idl.types[0].space.as_mut().unwrap().min = 3;
    });
    assert_abi_changes("type maximum space", &base, |idl| {
        idl.types[0].space.as_mut().unwrap().max = Some(3);
    });
    assert_abi_changes("event name", &base, |idl| idl.events[0].name.push_str("V2"));
    assert_abi_changes("event discriminator", &base, |idl| {
        idl.events[0].discriminator[0] = 9;
    });
    assert_abi_changes("event type", &base, |idl| {
        idl.events[0].ty = Some(IdlType::Primitive("u32".into()));
    });
}

#[test]
fn error_codes_and_names_are_client_compatibility_but_messages_are_not() {
    let base = fixture();

    assert_abi_changes("error code", &base, |idl| idl.errors[0].code = 6001);
    assert_abi_changes("error name", &base, |idl| {
        idl.errors[0].name.push_str("V2");
    });
    assert_abi_unchanged("error message", &base, |idl| {
        idl.errors[0].msg = Some("new wording".into());
    });
}
