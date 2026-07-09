/// Layout descriptor for zero-copy account wrappers.
///
/// Maps a wrapper type to its schema (for validation) and target (for Deref).
/// `DATA_OFFSET` is 0 for external accounts (no discriminator) and `disc_len`
/// for program accounts.
pub trait AccountLayout {
    /// The schema type that implements `ZeroPodFixed`.
    /// Used for validation via `ZeroPodFixed::validate()`.
    type Schema: crate::__zeropod::ZeroPodFixed;

    /// Byte offset where account data begins (after discriminator, if any).
    const DATA_OFFSET: usize;

    /// Size of the schema data in bytes.
    const DATA_SIZE: usize = <Self::Schema as crate::__zeropod::ZeroPodFixed>::SIZE;
}
