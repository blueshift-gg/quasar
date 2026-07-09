use {
    crate::{codec::IdlCodec, layout::IdlLayout, space::IdlSpace},
    serde::{Deserialize, Serialize},
};

/// Semantic type reference. Describes the logical value shape without encoding
/// details.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IdlType {
    /// Primitive scalar or well-known type name.
    /// One of: bool, u8, u16, u32, u64, u128, i8, i16, i32, i64, i128,
    /// f32, f64, pubkey, bytes, string.
    Primitive(String),
    /// Optional value.
    Option { option: Box<IdlType> },
    /// Variable-length sequence.
    Vec { vec: Box<IdlType> },
    /// Fixed-length array.
    Array { array: (Box<IdlType>, usize) },
    /// Reference to a named type definition.
    Defined { defined: IdlDefinedRef },
    /// Generic type parameter (in generic definitions).
    Generic { generic: String },
}

/// Reference to a named type definition with optional generic arguments.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdlDefinedRef {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generics: Vec<IdlGenericArg>,
}

/// A generic argument: either a type or a const value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum IdlGenericArg {
    #[serde(rename = "type")]
    Type { r#type: IdlType },
    #[serde(rename = "const")]
    Const { value: String },
}

/// A type definition in the root `types` array.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdlTypeDef {
    pub name: String,
    pub kind: IdlTypeDefKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generics: Vec<IdlGenericParam>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<IdlFieldDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<IdlEnumVariant>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<IdlType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codec: Option<IdlCodec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<IdlLayout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space: Option<IdlSpace>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics: Option<serde_json::Value>,
}

/// Kind of type definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IdlTypeDefKind {
    Struct,
    TupleStruct,
    Alias,
    Enum,
    Opaque,
}

/// A generic parameter declaration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdlGenericParam {
    pub kind: IdlGenericParamKind,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub ty: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IdlGenericParamKind {
    Type,
    Const,
}

/// A field in a struct or enum variant.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdlFieldDef {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: IdlType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codec: Option<IdlCodec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
}

/// An enum variant.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdlEnumVariant {
    pub name: String,
    pub value: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<IdlFieldDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<IdlLayout>,
}
