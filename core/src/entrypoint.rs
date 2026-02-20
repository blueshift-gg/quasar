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
        let __accounts_start = unsafe { ($ptr as *mut u8).add(core::mem::size_of::<u64>()) };

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
                        <$accounts_ty>::parse_accounts(__accounts_start, &mut __buf)
                    };
                    let __accounts = unsafe { __buf.assume_init() };
                    $handler(Context {
                        program_id: __program_id,
                        accounts: &__accounts,
                        remaining_ptr: __remaining_ptr,
                        data: $ix_data,
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
            pub struct NoAlloc;
            extern crate alloc;
            unsafe impl alloc::alloc::GlobalAlloc for NoAlloc {
                #[inline]
                unsafe fn alloc(&self, _: core::alloc::Layout) -> *mut u8 {
                    panic!("");
                }
                #[inline]
                unsafe fn dealloc(&self, _: *mut u8, _: core::alloc::Layout) {
                    // Can't dealloc if you never alloc ;)
                }
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
        fn panic(_info: &core::panic::PanicInfo<'_>) {
            solana_program_log::log("PANIC");
        }
    };
}
