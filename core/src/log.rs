//! Structured transaction logging via the `sol_log_data` syscall.
//!
//! On non-SBF targets the call is a no-op (consumed by `black_box` to
//! prevent the compiler from optimizing away the arguments).

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
use solana_define_syscall::definitions::sol_log_data;

/// Writes structured data to the transaction log via `sol_log_data`.
///
/// Each slice in `data` is emitted as a separate base64-encoded field in the
/// log entry. On non-SBF targets this is a no-op.
#[inline(always)]
pub fn log_data(data: &[&[u8]]) {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    // SAFETY: data is a valid slice of slices. The syscall reads data.len()
    // pointers from data.as_ptr(), each pointing to a valid &[u8].
    unsafe {
        sol_log_data(data.as_ptr() as *const u8, data.len() as u64);
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    {
        core::hint::black_box(data);
    }
}
