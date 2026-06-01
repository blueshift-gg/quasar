//! Top-level Quasar-relevant items detected by the scope scan.
//!
//! `ItemHead` carries enough information to answer existence-and-location
//! questions ("is there an `#[account] struct Counter` in this crate, and
//! where?") without committing to a full parse of the item's body. Full parse
//! lives in [`crate::parse::ParsedFile`].

/// Per-file item record with positional info. Used by `parse_file` output.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ItemHead {
    pub name: String,
    pub kind: ItemKind,
    pub range: ByteRange,
    /// Bytes of the `discriminator = ...` directive when this is an
    /// `AccountType`. `None` for `AccountsStruct` items or for `#[account]`
    /// types declared with `unsafe_no_disc`.
    pub discriminator: Option<Vec<u8>>,
    /// Named fields of the struct: data fields for an `AccountType`, account
    /// bindings for an `AccountsStruct`. Empty for a `define_account!` type —
    /// its fields live on a separate data struct resolved workspace-wide via
    /// [`Self::data_type`] (see [`Self::fields_known`]).
    pub fields: Vec<FieldDecl>,
    /// Whether [`Self::fields`] is the authoritative field list. `true` for
    /// structs parsed directly; `false` for `define_account!` types, whose
    /// fields live on a separate data struct resolved workspace-wide via
    /// [`Self::data_type`]. Consumers must not infer a field is *missing* from
    /// an empty list when this is `false`.
    pub fields_known: bool,
    /// For a `define_account!` type, the name of its associated data struct
    /// (the `: Data` clause, e.g. `MintData`). Resolved across the workspace
    /// to recover the type's real fields, since the data struct often lives in
    /// a different module than the macro call. `None` for directly-parsed
    /// structs and for macro forms without a data clause (marker programs).
    pub data_type: Option<String>,
    /// Byte offset just inside the struct's opening brace, used as an
    /// insertion anchor for "add field" code actions. `None` for unit/tuple
    /// structs and `define_account!` types.
    pub body_insert: Option<u32>,
}

/// A named field of a struct (data field or account binding).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldDecl {
    pub name: String,
    pub range: ByteRange,
}

/// Span-free identity of a Quasar-relevant item. Used by `scope_items` so the
/// workspace symbol index is value-stable across edits that don't add or
/// remove items (e.g., whitespace, body-only changes).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Symbol {
    pub name: String,
    pub kind: ItemKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemKind {
    /// Struct annotated with `#[account]`. Defines an on-chain account
    /// layout.
    AccountType,
    /// Struct annotated with `#[derive(Accounts)]`. Defines an instruction's
    /// account binding list.
    AccountsStruct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ByteRange {
    pub start: u32,
    pub end: u32,
}

impl ByteRange {
    pub fn from_span(span: proc_macro2::Span) -> Self {
        let r = span.byte_range();
        Self {
            start: r.start as u32,
            end: r.end as u32,
        }
    }
}
