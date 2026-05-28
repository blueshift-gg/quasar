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
