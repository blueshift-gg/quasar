use quasar_idl::types::{
    canonical_json, canonical_json_pretty, check_spec, compute_abi_hash, compute_idl_hash, Idl,
    IdlHashes,
};

const DOCUMENT_A: &str = r#"{
    "extensions": {
        "z": { "beta": 2, "alpha": 1 },
        "a": [{ "y": true, "x": null }, 3]
    },
    "address": "11111111111111111111111111111111",
    "version": "0.1.0",
    "name": "golden",
    "spec": "quasar-idl/1.1.0",
    "docs": ["first", "second"],
    "metadata": {
        "extra": {
            "zeta": { "right": 2, "left": 1 },
            "alpha": "value"
        }
    }
}"#;

const DOCUMENT_B: &str = r#"{
    "metadata": {
        "extra": {
            "alpha": "value",
            "zeta": { "left": 1, "right": 2 }
        }
    },
    "docs": ["first", "second"],
    "spec": "quasar-idl/1.1.0",
    "name": "golden",
    "version": "0.1.0",
    "address": "11111111111111111111111111111111",
    "extensions": {
        "a": [{ "x": null, "y": true }, 3],
        "z": { "alpha": 1, "beta": 2 }
    }
}"#;

const CANONICAL_JSON: &str = concat!(
    r#"{"address":"11111111111111111111111111111111","docs":["first","second"],"#,
    r#""extensions":{"a":[{"x":null,"y":true},3],"z":{"alpha":1,"beta":2}},"#,
    r#""metadata":{"extra":{"alpha":"value","zeta":{"left":1,"right":2}}},"#,
    r#""name":"golden","spec":"quasar-idl/1.1.0","version":"0.1.0"}"#,
);

// Golden SHA-256 of CANONICAL_JSON. Updating this value is a hash-contract
// change and must be reviewed together with the canonical bytes above.
const CANONICAL_HASH: &str = "e0d05b5a3dfb66e0509bb151a822fbb27090ed3c3318073d0b837de218705ac2";

fn parse(json: &str) -> Idl {
    check_spec(json).expect("compatible spec");
    serde_json::from_str(json).expect("valid IDL")
}

#[test]
fn full_hash_has_a_key_order_independent_golden_vector() {
    let left = parse(DOCUMENT_A);
    let right = parse(DOCUMENT_B);

    assert_eq!(canonical_json(&left).unwrap(), CANONICAL_JSON.as_bytes());
    assert_eq!(canonical_json(&right).unwrap(), CANONICAL_JSON.as_bytes());
    assert_eq!(compute_idl_hash(&left), CANONICAL_HASH);
    assert_eq!(compute_idl_hash(&right), CANONICAL_HASH);
}

#[test]
fn extension_bearing_document_round_trips_with_its_integrity_hash() {
    let mut idl = parse(DOCUMENT_A);
    let extensions = idl.extensions.clone();
    let idl_hash = compute_idl_hash(&idl);
    idl.hashes = Some(IdlHashes {
        idl: idl_hash.clone(),
        abi: compute_abi_hash(&idl),
    });

    let serialized = canonical_json_pretty(&idl).unwrap();
    let reparsed: Idl = serde_json::from_slice(&serialized).unwrap();

    assert_eq!(reparsed.extensions, extensions);
    assert_eq!(compute_idl_hash(&reparsed), idl_hash);
    assert_eq!(reparsed.hashes.as_ref().unwrap().idl, idl_hash);
}

#[test]
fn full_hash_covers_abi_documentation_and_opaque_extension_data() {
    let base = parse(DOCUMENT_A);
    let base_hash = compute_idl_hash(&base);

    let mut abi_mutation = base.clone();
    abi_mutation.address.push('2');

    let mut documentation_mutation = base.clone();
    documentation_mutation.docs.swap(0, 1);

    let mut metadata_mutation = base.clone();
    metadata_mutation
        .metadata
        .extra
        .insert("alpha".to_owned(), serde_json::json!("changed"));

    let mut extension_mutation = base.clone();
    extension_mutation.extensions.as_mut().unwrap()["z"]["beta"] = serde_json::json!(3);

    for (name, mutation) in [
        ("ABI", abi_mutation),
        ("documentation array order", documentation_mutation),
        ("metadata", metadata_mutation),
        ("opaque extension", extension_mutation),
    ] {
        assert_ne!(
            compute_idl_hash(&mutation),
            base_hash,
            "{name} is inside the full-IDL hash scope"
        );
    }

    let mut stored_hash_mutation = base;
    stored_hash_mutation.hashes = Some(IdlHashes {
        idl: "not-part-of-the-input".to_owned(),
        abi: "also-excluded".to_owned(),
    });
    assert_eq!(compute_idl_hash(&stored_hash_mutation), base_hash);
}
