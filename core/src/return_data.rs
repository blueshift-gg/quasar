//! Instruction return data via the `sol_set_return_data` syscall.
//!
//! Return data is available to the calling program after the CPI completes
//! via `sol_get_return_data`. On non-SBF targets the call is a no-op.

/// Sets the return data for the current instruction.
///
/// The data is available to the caller via `sol_get_return_data` after the
/// instruction completes. On non-SBF targets this is a no-op.
#[inline(always)]
pub fn set_return_data(_data: &[u8]) {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        use solana_define_syscall::definitions::sol_set_return_data;
        // SAFETY: _data is a valid slice; the syscall reads _data.len() bytes
        // from _data.as_ptr().
        unsafe {
            sol_set_return_data(_data.as_ptr(), _data.len() as u64);
        }
    }
}
