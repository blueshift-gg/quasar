//! Wire-format constants shared by encoder and decoder.
//!
//! See `SPEC.md` for the normative specification. Any value defined here is
//! part of the binary contract — changing a byte here is a breaking change.

/// Three-byte magic prefix. ASCII-friendly to make hex dumps recognisable.
pub const MAGIC: [u8; 3] = [0xC1, 0xDE, 0x1C];

/// Current wire-format version. Incremented on any breaking change.
pub const VERSION: u8 = 0x01;

/// Fixed header size: magic (3) + version (1) + body_len (4) + digest (32).
pub const HEADER_LEN: usize = 3 + 1 + 4 + 32;

// ---------------------------------------------------------------------------
// IdlType variant tags (see SPEC.md §3.1)
// ---------------------------------------------------------------------------

pub const TAG_TYPE_PRIMITIVE: u8 = 0x01;
pub const TAG_TYPE_OPTION: u8 = 0x02;
pub const TAG_TYPE_DEFINED: u8 = 0x03;
pub const TAG_TYPE_DYN_STRING: u8 = 0x04;
pub const TAG_TYPE_DYN_VEC: u8 = 0x05;

// ---------------------------------------------------------------------------
// IdlSeed variant tags (see SPEC.md §3.2)
// ---------------------------------------------------------------------------

pub const TAG_SEED_CONST: u8 = 0x01;
pub const TAG_SEED_ACCOUNT: u8 = 0x02;
pub const TAG_SEED_ARG: u8 = 0x03;

// ---------------------------------------------------------------------------
// IdlTypeDefKind tags (see SPEC.md §3.3)
// ---------------------------------------------------------------------------

pub const TAG_TYPEDEF_STRUCT: u8 = 0x01;
