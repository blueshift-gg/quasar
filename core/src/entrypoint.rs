//! Program entrypoint macros.
//!
//! `dispatch!` routes instruction data to handler functions based on a
//! discriminator prefix. `no_alloc!`, `heap_alloc!`, and `panic_handler!`
//! configure the program's runtime environment (allocator and panic behavior).

pub use solana_account_view::{
    AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED,
};

#[macro_export]
macro_rules! dispatch {
    ($ptr:expr, $ix_data:expr, $disc_len:literal, {
        $([$($disc_byte:literal),+] => $handler:ident($accounts_ty:ty)),+ $(,)?
    }) => {{
        let __program_id: &[u8; 32] = unsafe {
            &*($ix_data.as_ptr().add($ix_data.len()) as *const [u8; 32])
        };
        const __U64_SIZE: usize = core::mem::size_of::<u64>();
        let __accounts_start = unsafe { ($ptr as *mut u8).add(__U64_SIZE) };

        if $ix_data.len() < $disc_len {
            return Err(ProgramError::InvalidInstructionData);
        }
        let __disc: [u8; $disc_len] = unsafe {
            *($ix_data.as_ptr() as *const [u8; $disc_len])
        };
        match __disc {
            $(
                [$($disc_byte),+] => {
                    let mut __buf = core::mem::MaybeUninit::<
                        [AccountView; <$accounts_ty as AccountCount>::COUNT]
                    >::uninit();
                    let __remaining_ptr = unsafe {
                        <$accounts_ty>::parse_accounts(__accounts_start, &mut __buf)?
                    };
                    let __accounts = unsafe { __buf.assume_init() };
                    $handler(Context {
                        program_id: __program_id,
                        accounts: &__accounts,
                        remaining_ptr: __remaining_ptr,
                        data: $ix_data,
                        accounts_boundary: unsafe { $ix_data.as_ptr().sub(__U64_SIZE) },
                    })
                }
            ),+
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }};
}

#[macro_export]
macro_rules! no_alloc {
    () => {
        pub mod allocator {
            extern crate alloc;
            pub struct NoAlloc;
            unsafe impl alloc::alloc::GlobalAlloc for NoAlloc {
                #[inline]
                unsafe fn alloc(&self, _: core::alloc::Layout) -> *mut u8 {
                    panic!("");
                }
                #[inline]
                unsafe fn dealloc(&self, _: *mut u8, _: core::alloc::Layout) {}
            }

            #[cfg(any(target_os = "solana", target_arch = "bpf"))]
            #[global_allocator]
            static A: NoAlloc = NoAlloc;
        }
    };
}

#[macro_export]
macro_rules! panic_handler {
    () => {
        #[cfg(any(target_os = "solana", target_arch = "bpf"))]
        #[panic_handler]
        fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
            $crate::prelude::log("PANIC");
            loop {}
        }
    };
}

/// The bump allocator used as the default Rust heap when running programs.
#[macro_export]
macro_rules! heap_alloc {
    () => {
        pub mod allocator {
            extern crate alloc;

            /// Maximum heap length in bytes that a program can request.
            pub const MAX_HEAP_LENGTH: u32 = 256 * 1024; // 256 KiB
            /// Start address of the memory region used for program heap.
            pub const HEAP_START_ADDRESS: u64 = 0x300000000;

            pub struct BumpAllocator {
                start: usize,
                end: usize,
            }
            impl BumpAllocator {
                pub const unsafe fn new_unchecked(start: usize, len: usize) -> Self {
                    Self {
                        start,
                        end: start + len,
                    }
                }
            }

            #[allow(clippy::arithmetic_side_effects)]
            unsafe impl alloc::alloc::GlobalAlloc for BumpAllocator {
                #[inline]
                unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
                    let pos_ptr = self.start as *mut usize;
                    let pos = *pos_ptr;

                    let allocation = (pos.wrapping_add(layout.align() - 1)) & !(layout.align() - 1);

                    // Overflow guard: any request > 256 KiB cannot succeed and prevents
                    // allocation + layout.size() from wrapping usize.
                    let end = allocation.wrapping_add(layout.size());
                    if $crate::utils::hint::unlikely(layout.size() > MAX_HEAP_LENGTH as usize)
                        || $crate::utils::hint::unlikely(self.end < end)
                    {
                        return core::ptr::null_mut();
                    }

                    *pos_ptr = end;
                    allocation as *mut u8
                }

                #[inline]
                unsafe fn alloc_zeroed(&self, layout: core::alloc::Layout) -> *mut u8 {
                    self.alloc(layout)
                }

                #[inline]
                unsafe fn dealloc(&self, _: *mut u8, _: core::alloc::Layout) {}
            }

            #[cfg(any(target_os = "solana", target_arch = "bpf"))]
            #[global_allocator]
            static A: BumpAllocator = unsafe {
                BumpAllocator::new_unchecked(HEAP_START_ADDRESS as usize, MAX_HEAP_LENGTH as usize)
            };

            #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
            mod __private_alloc {
                extern crate std as __std;
            }
        }
    };
}
