use quasar_idl::types::{check_spec, spec_is_supported, Idl, CURRENT_SPEC};

fn probe(spec: &str) -> String {
    format!(r#"{{"spec":"{spec}"}}"#)
}

#[test]
fn accepts_stable_v1_semver_and_build_metadata() {
    for spec in [
        CURRENT_SPEC,
        "quasar-idl/1.4.2",
        "quasar-idl/1.4.2+vendor.7",
    ] {
        assert!(spec_is_supported(spec), "expected `{spec}` to be supported");
        assert_eq!(check_spec(&probe(spec)).unwrap(), spec);
    }
}

#[test]
fn rejects_malformed_semver() {
    for spec in [
        "quasar-idl/1",
        "quasar-idl/1.0",
        "quasar-idl/1.0.0.0",
        "quasar-idl/01.0.0",
        "quasar-idl/1.01.0",
        "quasar-idl/1.0.01",
        "quasar-idl/1.0.0 trailing",
    ] {
        assert!(!spec_is_supported(spec), "expected `{spec}` to be rejected");
        let error = check_spec(&probe(spec)).unwrap_err();
        assert!(error.contains("malformed IDL spec version"), "{error}");
    }
}

#[test]
fn rejects_prereleases_other_majors_and_schemes() {
    for spec in ["quasar-idl/1.0.0-alpha", "quasar-idl/1.1.0-rc.1"] {
        let error = check_spec(&probe(spec)).unwrap_err();
        assert!(error.contains("prerelease"), "{error}");
    }

    let error = check_spec(&probe("quasar-idl/2.0.0")).unwrap_err();
    assert!(error.contains("unsupported IDL spec"), "{error}");
    let error = check_spec(&probe("anchor/0.30.0")).unwrap_err();
    assert!(error.contains("invalid IDL spec scheme"), "{error}");
}

#[test]
fn compatible_v1_extensions_round_trip_without_loss() {
    let json = r#"{
        "spec": "quasar-idl/1.1.0+vendor.7",
        "name": "demo",
        "version": "0.1.0",
        "address": "11111111111111111111111111111111",
        "extensions": {
            "vendor.example": { "enabled": true, "limit": 7 }
        }
    }"#;

    assert_eq!(check_spec(json).unwrap(), "quasar-idl/1.1.0+vendor.7");
    let idl: Idl = serde_json::from_str(json).expect("compatible v1 extension document");
    assert_eq!(
        idl.extensions,
        Some(serde_json::json!({
            "vendor.example": { "enabled": true, "limit": 7 }
        }))
    );

    let serialized = serde_json::to_string(&idl).unwrap();
    let reparsed: Idl = serde_json::from_str(&serialized).unwrap();
    assert_eq!(reparsed.extensions, idl.extensions);
}

#[test]
fn additive_data_outside_extensions_is_rejected() {
    let json = r#"{
        "spec": "quasar-idl/1.1.0",
        "name": "demo",
        "version": "0.1.0",
        "address": "11111111111111111111111111111111",
        "futureTopLevelField": { "anything": true }
    }"#;

    check_spec(json).expect("the version itself is compatible");
    let error = serde_json::from_str::<Idl>(json)
        .expect_err("additive data outside extensions must be rejected");
    assert!(error.to_string().contains("futureTopLevelField"), "{error}");
}

#[test]
fn missing_spec_has_a_version_gate_diagnostic() {
    let error = check_spec(r#"{"name":"demo"}"#).unwrap_err();
    assert!(error.contains("`spec`"), "{error}");
}
