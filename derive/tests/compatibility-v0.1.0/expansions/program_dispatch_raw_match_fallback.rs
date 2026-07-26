::quasar_lang::define_account!(
    pub struct QuasarRawGapDemoProgram => [::quasar_lang::checks::Executable,
    ::quasar_lang::checks::Address]
);
impl ::quasar_lang::traits::Id for QuasarRawGapDemoProgram {
    const ID: ::quasar_lang::prelude::Address = crate::ID;
}
#[repr(transparent)]
pub struct EventAuthority {
    view: ::quasar_lang::__internal::AccountView,
}
impl ::quasar_lang::traits::AsAccountView for EventAuthority {
    #[inline(always)]
    fn to_account_view(&self) -> &::quasar_lang::__internal::AccountView {
        &self.view
    }
}
impl EventAuthority {
    const __PDA: (::quasar_lang::prelude::Address, u8) = ::quasar_lang::pda::find_program_address_const(
        &[b"__event_authority"],
        &crate::ID,
    );
    pub const ADDRESS: ::quasar_lang::prelude::Address = Self::__PDA.0;
    pub const BUMP: u8 = Self::__PDA.1;
    #[inline(always)]
    pub fn from_account_view(
        view: &::quasar_lang::__internal::AccountView,
    ) -> Result<&Self, ::quasar_lang::__solana_program_error::ProgramError> {
        if !::quasar_lang::keys_eq(view.address(), &Self::ADDRESS) {
            return Err(
                ::quasar_lang::__solana_program_error::ProgramError::InvalidSeeds,
            );
        }
        Ok(unsafe {
            &*(view as *const ::quasar_lang::__internal::AccountView as *const Self)
        })
    }
    /// Construct without validation.
    ///
    /// # Safety
    /// Caller must ensure account address matches the expected PDA.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked(
        view: &::quasar_lang::__internal::AccountView,
    ) -> &Self {
        unsafe {
            &*(view as *const ::quasar_lang::__internal::AccountView as *const Self)
        }
    }
}
unsafe impl ::quasar_lang::traits::StaticView for EventAuthority {}
impl ::quasar_lang::account_load::AccountLoad for EventAuthority {
    #[inline(always)]
    fn check(
        view: &::quasar_lang::__internal::AccountView,
    ) -> Result<(), ::quasar_lang::__solana_program_error::ProgramError> {
        if !::quasar_lang::keys_eq(view.address(), &Self::ADDRESS) {
            return Err(
                ::quasar_lang::__solana_program_error::ProgramError::InvalidSeeds,
            );
        }
        Ok(())
    }
}
#[cfg_attr(not(any(target_arch = "bpf", target_os = "solana")), allow(dead_code))]
mod quasar_raw_gap_demo {
    use super::*;
    #[instruction(discriminator = 1, raw)]
    pub fn raw_one(ctx: Context) -> Result<(), ProgramError> {
        let _ = ctx.data;
        Ok(())
    }
    #[instruction(discriminator = 7, raw)]
    pub fn raw_seven(ctx: Context) -> Result<(), ProgramError> {
        let _ = ctx.data;
        Ok(())
    }
    #[inline(always)]
    fn __handle_event(
        ptr: *mut u8,
        instruction_data: &[u8],
    ) -> Result<(), ::quasar_lang::__solana_program_error::ProgramError> {
        unsafe {
            ::quasar_lang::event::handle_event(
                ptr,
                instruction_data,
                &super::EventAuthority::ADDRESS,
            )
        }
    }
    #[inline(always)]
    fn __dispatch(
        ptr: *mut u8,
        instruction_data: &[u8],
    ) -> Result<(), ::quasar_lang::__solana_program_error::ProgramError> {
        if !instruction_data.is_empty() && instruction_data[0] == 0xFF {
            return __handle_event(ptr, instruction_data);
        }
        if instruction_data.len() >= 1usize {
            let __raw_disc: [u8; 1usize] = unsafe {
                *(instruction_data.as_ptr() as *const [u8; 1usize])
            };
            if matches!(__raw_disc, [1] | [7]) {
                let __raw_program_id: &[u8; 32] = unsafe {
                    &*(instruction_data.as_ptr().add(instruction_data.len())
                        as *const [u8; 32])
                };
                const __RAW_U64: usize = core::mem::size_of::<u64>();
                let __raw_num_accounts = unsafe { *(ptr as *const u64) };
                let __raw_accounts_start = unsafe { (ptr as *mut u8).add(__RAW_U64) };
                let __raw_boundary = unsafe { instruction_data.as_ptr().sub(__RAW_U64) };
                const __RAW_MAX: usize = 64;
                let __raw_count = core::cmp::min(__raw_num_accounts as usize, __RAW_MAX);
                let mut __raw_buf = core::mem::MaybeUninit::<
                    [::quasar_lang::__internal::AccountView; __RAW_MAX],
                >::uninit();
                let (__raw_parsed, __raw_remaining) = unsafe {
                    ::quasar_lang::__internal::parse_all_accounts_unchecked(
                        __raw_accounts_start,
                        __raw_buf.as_mut_ptr()
                            as *mut ::quasar_lang::__internal::AccountView,
                        __raw_count,
                        __raw_boundary,
                    )?
                };
                let __raw_accounts = unsafe {
                    core::slice::from_raw_parts_mut(
                        __raw_buf.as_mut_ptr()
                            as *mut ::quasar_lang::__internal::AccountView,
                        __raw_parsed,
                    )
                };
                let __raw_ctx = unsafe {
                    ::quasar_lang::context::Context::from_raw_parts(
                        __raw_program_id,
                        __raw_accounts,
                        &instruction_data[1usize..],
                        __raw_remaining,
                        __raw_boundary as *const u8,
                    )
                };
                match __raw_disc {
                    [1] => {
                        return raw_one(__raw_ctx);
                    }
                    [7] => {
                        return raw_seven(__raw_ctx);
                    }
                    _ => unsafe { core::hint::unreachable_unchecked() }
                }
            }
        }
        Err(::quasar_lang::__solana_program_error::ProgramError::InvalidInstructionData)
    }
    #[unsafe(no_mangle)]
    #[allow(unexpected_cfgs)]
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    pub unsafe extern "C" fn entrypoint(
        ptr: *mut u8,
        instruction_data: *const u8,
    ) -> u64 {
        #[cfg(feature = "alloc")]
        {
            let heap_start = super::allocator::HEAP_START_ADDRESS as usize;
            unsafe {
                *(heap_start as *mut usize) = heap_start + core::mem::size_of::<usize>();
            }
        }
        let instruction_data = unsafe {
            core::slice::from_raw_parts(
                instruction_data,
                *(instruction_data.sub(8) as *const u64) as usize,
            )
        };
        match __dispatch(ptr, instruction_data) {
            Ok(_) => 0,
            Err(e) => e.into(),
        }
    }
    #[allow(unexpected_cfgs)]
    #[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
    pub mod cpi {
        use super::*;
    }
}
#[allow(unexpected_cfgs)]
#[cfg(any(not(any(target_arch = "bpf", target_os = "solana")), feature = "alloc"))]
extern crate alloc;
#[allow(unexpected_cfgs)]
#[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
pub use quasar_raw_gap_demo::cpi;
#[allow(unexpected_cfgs)]
#[cfg(any(target_os = "solana", target_arch = "bpf"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    ::quasar_lang::abort_program()
}
#[allow(unexpected_cfgs)]
#[cfg(feature = "alloc")]
::quasar_lang::heap_alloc!();
#[allow(unexpected_cfgs)]
#[cfg(not(feature = "alloc"))]
::quasar_lang::no_alloc!();
#[cfg(feature = "idl-build")]
::quasar_lang::__private_inventory::submit! {
    ::quasar_lang::idl_build::InstructionFragment { build : { fn __build() ->
    ::quasar_lang::idl_build::__reexport::IdlInstruction {
    ::quasar_lang::idl_build::__reexport::IdlInstruction { name :
    ::quasar_lang::idl_build::s("raw_one"), discriminator :
    ::quasar_lang::idl_build::vec![1u8], docs : ::quasar_lang::idl_build::Vec::new(),
    accounts : ::quasar_lang::idl_build::Vec::new(), args :
    ::quasar_lang::idl_build::Vec::new(), layout : None, remaining_accounts : None, } }
    __build }, accounts_struct_name : "", discriminator_source :
    ::quasar_lang::idl_build::InstructionDiscriminatorSource::Explicit, }
}
#[cfg(feature = "idl-build")]
::quasar_lang::__private_inventory::submit! {
    ::quasar_lang::idl_build::InstructionFragment { build : { fn __build() ->
    ::quasar_lang::idl_build::__reexport::IdlInstruction {
    ::quasar_lang::idl_build::__reexport::IdlInstruction { name :
    ::quasar_lang::idl_build::s("raw_seven"), discriminator :
    ::quasar_lang::idl_build::vec![7u8], docs : ::quasar_lang::idl_build::Vec::new(),
    accounts : ::quasar_lang::idl_build::Vec::new(), args :
    ::quasar_lang::idl_build::Vec::new(), layout : None, remaining_accounts : None, } }
    __build }, accounts_struct_name : "", discriminator_source :
    ::quasar_lang::idl_build::InstructionDiscriminatorSource::Explicit, }
}
/// Assemble all IDL fragments and return JSON.
#[cfg(feature = "idl-build")]
pub fn __quasar_build_idl() -> ::quasar_lang::idl_build::String {
    let address = ::quasar_lang::idl_build::address_to_base58(&crate::ID);
    let idl = ::quasar_lang::idl_build::build_idl(
        &address,
        "quasar_raw_gap_demo",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
    );
    ::quasar_lang::idl_build::__reexport::serde_json::to_string_pretty(&idl)
        .expect("generated IDL should serialize")
}
#[allow(unexpected_cfgs)]
#[cfg(
    all(feature = "idl-build", test, not(any(target_os = "solana", target_arch = "bpf")))
)]
#[test]
fn __quasar_emit_idl() {
    extern crate std;
    std::println!("__QUASAR_IDL_JSON_BEGIN__");
    std::println!("{}", __quasar_build_idl());
    std::println!("__QUASAR_IDL_JSON_END__");
}
