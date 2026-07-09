use {
    crate::types::IdlType,
    serde::{Deserialize, Serialize},
};

/// An event definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdlEventDef {
    pub name: String,
    pub discriminator: Vec<u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub ty: Option<IdlType>,
}
