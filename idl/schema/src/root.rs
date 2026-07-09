use {
    crate::{
        account::IdlAccountDef, constant::IdlConstant, error::IdlErrorDef, event::IdlEventDef,
        instruction::IdlInstruction, types::IdlTypeDef, wrapper::IdlWrappers,
    },
    serde::{Deserialize, Serialize},
    std::collections::BTreeMap,
};

/// The root IDL structure. Represents the complete program interface.
///
/// Schema version: `quasar-idl/1.0.0`
///
/// The root deliberately does NOT `deny_unknown_fields`: it is the additive
/// extension point, so a v1.0 reader tolerates unknown top-level fields written
/// by a newer minor spec (compatibility is decided up front by the `spec`
/// version gate). Leaf types keep `deny_unknown_fields` so precise contracts
/// still reject typos and stray keys.
#[derive(Clone, Debug, Serialize, Deserialize)]
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
    /// Constants exposed to clients.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constants: Vec<IdlConstant>,
    /// Wrapper registry (extension points).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrappers: Option<IdlWrappers>,
    /// Extension declarations (reserved for v1.1+).
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
    fn root_tolerates_unknown_top_level_fields() {
        // Additive policy: a newer minor spec may add root-level fields, and a
        // v1.0 reader must not reject them.
        let json = MINIMAL.replace(
            "\"address\": \"11111111111111111111111111111111\"",
            "\"address\": \"11111111111111111111111111111111\", \
             \"futureTopLevelField\": { \"anything\": true }",
        );
        let idl: Idl = serde_json::from_str(&json).expect("unknown root field must be tolerated");
        assert_eq!(idl.name, "demo");
    }

    #[test]
    fn leaf_rejects_unknown_fields() {
        // `hashes` is a leaf type: stray keys are contract errors.
        let json = MINIMAL.replace(
            "\"address\": \"11111111111111111111111111111111\"",
            "\"address\": \"11111111111111111111111111111111\", \
             \"hashes\": { \"idl\": \"a\", \"abi\": \"b\", \"bogus\": \"c\" }",
        );
        let err =
            serde_json::from_str::<Idl>(&json).expect_err("unknown leaf field must be rejected");
        assert!(
            err.to_string().contains("bogus") || err.to_string().contains("unknown field"),
            "unexpected error: {err}"
        );
    }
}
