use {
    crate::{
        account::IdlAccountDef, error::IdlErrorDef, event::IdlEventDef,
        instruction::IdlInstruction, types::IdlTypeDef,
    },
    semver::Version,
    serde::{Deserialize, Serialize},
    std::collections::BTreeMap,
};

/// The root IDL structure. Represents the complete program interface.
///
/// Schema version: `quasar-idl/1.0.0`
///
/// The root is closed like every leaf schema. Later 1.x specs can add data only
/// below `extensions`, which older readers preserve as opaque JSON.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Idl {
    /// Schema version string (e.g., "quasar-idl/1.0.0").
    pub spec: String,
    /// Program name (display name).
    pub name: String,
    /// Program version (semver).
    pub version: String,
    /// Program address (base58-encoded pubkey).
    pub address: String,
    /// Build and package metadata.
    #[serde(default, skip_serializing_if = "IdlMetadata::is_empty")]
    pub metadata: IdlMetadata,
    /// Program-level documentation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    /// Instruction definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instructions: Vec<IdlInstruction>,
    /// Account data definitions (state types stored on-chain).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accounts: Vec<IdlAccountDef>,
    /// Type definitions (shared types used by instructions, accounts, events).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<IdlTypeDef>,
    /// Event definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<IdlEventDef>,
    /// Error definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<IdlErrorDef>,
    /// Opaque additive data for later compatible 1.x specs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Value>,
    /// Integrity hashes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hashes: Option<IdlHashes>,
}

/// Build and package metadata.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdlMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "crateName")]
    pub crate_name: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "packageName"
    )]
    pub package_name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "generatorVersion"
    )]
    pub generator_version: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "schemaVersion"
    )]
    pub schema_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// Arbitrary extra metadata (BTreeMap for deterministic serialization).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl IdlMetadata {
    pub fn is_empty(&self) -> bool {
        self.crate_name.is_none()
            && self.package_name.is_none()
            && self.features.is_empty()
            && self.generator_version.is_none()
            && self.schema_version.is_none()
            && self.profile.is_none()
            && self.extra.is_empty()
    }

    /// Get the client-facing name (prefers crate_name, falls back to program
    /// name).
    pub fn client_name(&self, program_name: &str) -> String {
        self.crate_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(program_name)
            .to_owned()
    }
}

/// Integrity hashes for the IDL.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdlHashes {
    /// SHA-256 hash of the full canonical IDL (excluding the `hashes` field).
    pub idl: String,
    /// SHA-256 hash of the ABI-affecting subset only.
    pub abi: String,
}

/// Scheme prefix shared by every Quasar IDL spec string.
pub const SPEC_SCHEME: &str = "quasar-idl/";
/// The full spec string this build produces.
pub const CURRENT_SPEC: &str = "quasar-idl/1.0.0";
/// The only schema major this build can read.
pub const SUPPORTED_SPEC_MAJOR: u64 = 1;

/// Read just the `spec` field of an IDL JSON document, so callers can gate on
/// the schema version before committing to a full parse.
pub fn parse_spec(json: &str) -> Result<String, serde_json::Error> {
    #[derive(Deserialize)]
    struct SpecProbe {
        spec: String,
    }
    serde_json::from_str::<SpecProbe>(json).map(|probe| probe.spec)
}

fn validate_spec(spec: &str) -> Result<(), String> {
    let version = spec.strip_prefix(SPEC_SCHEME).ok_or_else(|| {
        format!("invalid IDL spec scheme in `{spec}`; expected `{SPEC_SCHEME}<semver>`")
    })?;
    let version = Version::parse(version)
        .map_err(|error| format!("malformed IDL spec version in `{spec}`: {error}"))?;
    if !version.pre.is_empty() {
        return Err(format!(
            "unsupported prerelease IDL spec `{spec}`; use a stable `{SPEC_SCHEME}1.x` version"
        ));
    }
    if version.major != SUPPORTED_SPEC_MAJOR {
        return Err(format!(
            "unsupported IDL spec `{spec}`; this build reads stable `{SPEC_SCHEME}1.x` versions \
             (e.g. `{CURRENT_SPEC}`)"
        ));
    }
    Ok(())
}

/// Whether a `spec` string is one this build can read.
///
/// The suffix must be strict SemVer with major 1. Stable versions and build
/// metadata are accepted; prerelease versions are rejected. Compatible newer
/// 1.x producers must put additive data below the preserved `extensions` field.
pub fn spec_is_supported(spec: &str) -> bool {
    validate_spec(spec).is_ok()
}

/// Gate an IDL JSON document on its spec version. Returns the spec string on
/// success, or a human-readable diagnostic describing the mismatch. Every IDL
/// parse site should call this first so an incompatible spec fails with a clear
/// message instead of a confusing field-level deserialization error.
pub fn check_spec(json: &str) -> Result<String, String> {
    let spec = parse_spec(json).map_err(|e| {
        format!("IDL is missing a top-level `spec` field or is not valid JSON: {e}")
    })?;
    validate_spec(&spec)?;
    Ok(spec)
}

#[cfg(test)]
mod spec_tests {
    use super::{check_spec, spec_is_supported, CURRENT_SPEC};

    #[test]
    fn accepts_current_and_stable_additive_minor_specs() {
        assert!(spec_is_supported(CURRENT_SPEC));
        assert!(spec_is_supported("quasar-idl/1.4.2+vendor.7"));
        assert!(check_spec(r#"{"spec":"quasar-idl/1.9.0","name":"x"}"#).is_ok());
    }

    #[test]
    fn rejects_other_majors_and_schemes() {
        assert!(!spec_is_supported("quasar-idl/2.0.0"));
        assert!(!spec_is_supported("anchor/0.30.0"));
        let err = check_spec(r#"{"spec":"quasar-idl/2.0.0","name":"x"}"#).unwrap_err();
        assert!(err.contains("unsupported IDL spec"), "{err}");
        let err = check_spec(r#"{"spec":"anchor/0.30.0","name":"x"}"#).unwrap_err();
        assert!(err.contains("invalid IDL spec scheme"), "{err}");
    }

    #[test]
    fn reports_missing_spec_field() {
        let err = check_spec(r#"{"name":"x"}"#).unwrap_err();
        assert!(err.contains("`spec`"), "{err}");
    }
}

#[cfg(test)]
mod deny_unknown_tests {
    use super::Idl;

    const MINIMAL: &str = r#"{
        "spec": "quasar-idl/1.0.0",
        "name": "demo",
        "version": "0.1.0",
        "address": "11111111111111111111111111111111"
    }"#;

    #[test]
    fn root_rejects_unknown_top_level_fields() {
        let json = MINIMAL.replace(
            "\"address\": \"11111111111111111111111111111111\"",
            "\"address\": \"11111111111111111111111111111111\", \"futureTopLevelField\": { \
             \"anything\": true }",
        );
        let error = serde_json::from_str::<Idl>(&json)
            .expect_err("additive data outside extensions must be rejected");
        assert!(error.to_string().contains("futureTopLevelField"), "{error}");
    }

    #[test]
    fn leaf_rejects_unknown_fields() {
        // `hashes` is a leaf type: stray keys are contract errors.
        let json = MINIMAL.replace(
            "\"address\": \"11111111111111111111111111111111\"",
            "\"address\": \"11111111111111111111111111111111\", \"hashes\": { \"idl\": \"a\", \
             \"abi\": \"b\", \"bogus\": \"c\" }",
        );
        let err =
            serde_json::from_str::<Idl>(&json).expect_err("unknown leaf field must be rejected");
        assert!(
            err.to_string().contains("bogus") || err.to_string().contains("unknown field"),
            "unexpected error: {err}"
        );
    }
}
