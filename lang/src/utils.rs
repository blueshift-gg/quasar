//! Compiler optimization hints for cold branches on the SBF backend.

/// Compiler branch-prediction hints.
pub mod hint {
    /// Cold marker used by [`unlikely`].
    #[cold]
    pub const fn cold_path() {}

    /// Return `b`, marking `true` as the cold branch.
    #[inline(always)]
    pub const fn unlikely(b: bool) -> bool {
        if b {
            cold_path();
            true
        } else {
            false
        }
    }
}
