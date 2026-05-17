use {
    crate::types::IdlType,
    serde::{Deserialize, Serialize},
};

/// Describes how a field is encoded on the wire.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum IdlCodec {
    /// Fixed-size scalar (u8..u128, i8..i128).
    #[serde(rename = "scalar")]
    Scalar {
        format: String,
        endian: Endian,
        size: usize,
    },
    /// Boolean with explicit tag representation.
    #[serde(rename = "bool")]
    Bool {
        repr: ScalarRepr,
        r#false: u8,
        r#true: u8,
    },
    /// Fixed byte array.
    #[serde(rename = "fixedBytes")]
    FixedBytes { size: usize },
    /// Fixed-length array repeating item codec.
    #[serde(rename = "array")]
    Array {
        length: usize,
        item: Box<IdlCodecItem>,
        size: usize,
    },
    /// Reference to a defined type's codec.
    #[serde(rename = "defined")]
    Defined { name: String },
    /// Optional value with explicit tag.
    #[serde(rename = "option")]
    Option {
        tag: ScalarRepr,
        none: u8,
        some: u8,
        #[serde(rename = "nonePayload")]
        none_payload: NonePayload,
        payload: Box<IdlCodecItem>,
    },
    /// Enum discriminant with explicit tag.
    #[serde(rename = "enum")]
    Enum {
        tag: ScalarRepr,
        #[serde(rename = "validTags")]
        valid_tags: Vec<u64>,
        payloads: EnumPayloads,
    },
    /// Size-prefixed dynamic data (string or vec).
    #[serde(rename = "sizePrefixed")]
    SizePrefixed {
        prefix: ScalarRepr,
        storage: Storage,
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "maxBytes")]
        max_bytes: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "maxItems")]
        max_items: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        encoding: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        item: Option<Box<IdlCodecItem>>,
    },
    /// Consumes remainder of container (final tail field).
    #[serde(rename = "remainder")]
    Remainder {
        storage: Storage,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        encoding: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        item: Option<Box<IdlCodecItem>>,
    },
}

/// Endianness for scalar encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Endian {
    Le,
    Be,
}

/// Scalar type representation (used in prefixes, tags, etc.).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScalarRepr {
    #[serde(rename = "type")]
    pub ty: String,
    pub endian: Endian,
}

/// Storage location for dynamic data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Storage {
    Inline,
    Tail,
}

/// What happens to the payload slot when an option is None.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NonePayload {
    Omitted,
}

/// How enum payloads are described.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EnumPayloads {
    None,
    VariantLayouts,
}

/// A codec item: a type paired with its codec.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IdlCodecItem {
    #[serde(rename = "type")]
    pub ty: IdlType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codec: Option<IdlCodec>,
}

impl IdlCodec {
    /// Get the prefix byte width for size-prefixed codecs.
    pub fn prefix_bytes(&self) -> usize {
        match self {
            Self::SizePrefixed { prefix, .. } => match prefix.ty.as_str() {
                "u8" => 1,
                "u16" => 2,
                "u32" => 4,
                "u64" => 8,
                _ => 2,
            },
            _ => 0,
        }
    }

    /// Get max_bytes for string codecs.
    pub fn max_bytes(&self) -> Option<usize> {
        match self {
            Self::SizePrefixed { max_bytes, .. } => *max_bytes,
            _ => None,
        }
    }

    /// Get max_items for vec codecs.
    pub fn max_items(&self) -> Option<usize> {
        match self {
            Self::SizePrefixed { max_items, .. } => *max_items,
            _ => None,
        }
    }
}

impl ScalarRepr {
    /// Byte width of this scalar type.
    pub fn byte_width(&self) -> usize {
        match self.ty.as_str() {
            "u8" | "i8" => 1,
            "u16" | "i16" => 2,
            "u32" | "i32" | "f32" => 4,
            "u64" | "i64" | "f64" => 8,
            "u128" | "i128" => 16,
            _ => 1,
        }
    }
}
