quasar_lang::define_account!(
    pub struct QuasarDemo => [quasar_lang::checks::Executable,
    quasar_lang::checks::Address]
);
impl quasar_lang::traits::Id for QuasarDemo {
    const ID: Address = crate::ID;
}
#[repr(transparent)]
pub struct EventAuthority {
    view: AccountView,
}
impl AsAccountView for EventAuthority {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        &self.view
    }
}
impl EventAuthority {
    const __PDA: (Address, u8) = quasar_lang::pda::find_program_address_const(
        &[b"__event_authority"],
        &crate::ID,
    );
    pub const ADDRESS: Address = Self::__PDA.0;
    pub const BUMP: u8 = Self::__PDA.1;
    #[inline(always)]
    pub fn from_account_view(view: &AccountView) -> Result<&Self, ProgramError> {
        if !quasar_lang::keys_eq(view.address(), &Self::ADDRESS) {
            return Err(ProgramError::InvalidSeeds);
        }
        Ok(unsafe { &*(view as *const AccountView as *const Self) })
    }
    /// Construct without validation.
    ///
    /// # Safety
    /// Caller must ensure account address matches the expected PDA.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
        unsafe { &*(view as *const AccountView as *const Self) }
    }
}
unsafe impl quasar_lang::traits::StaticView for EventAuthority {}
impl quasar_lang::account_load::AccountLoad for EventAuthority {
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        if !quasar_lang::keys_eq(view.address(), &Self::ADDRESS) {
            return Err(ProgramError::InvalidSeeds);
        }
        Ok(())
    }
}
impl QuasarDemo {
    #[inline(always)]
    pub fn emit_event<E: quasar_lang::traits::Event>(
        &self,
        event: &E,
        event_authority: &EventAuthority,
    ) -> Result<(), ProgramError> {
        let program = self.to_account_view();
        let ea = event_authority.to_account_view();
        event
            .emit(|data| {
                quasar_lang::event::emit_event_cpi(
                    program,
                    ea,
                    data,
                    EventAuthority::BUMP,
                )
            })
    }
}
#[allow(dead_code)]
mod quasar_demo {
    use super::*;
    #[instruction(discriminator = 0)]
    pub fn initialize(ctx: Ctx<Initialize>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount)
    }
    #[instruction(discriminator = 1)]
    pub fn update(ctx: Ctx<Update>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[inline(always)]
    fn __handle_event(
        ptr: *mut u8,
        instruction_data: &[u8],
    ) -> Result<(), ProgramError> {
        unsafe {
            quasar_lang::event::handle_event(
                ptr,
                instruction_data,
                &super::EventAuthority::ADDRESS,
            )
        }
    }
    #[inline(always)]
    fn __dispatch(ptr: *mut u8, instruction_data: &[u8]) -> Result<(), ProgramError> {
        const __QUASAR_NEEDS_EVENT_CPI: bool = false
            || <Initialize as AccountCount>::NEEDS_EVENT_CPI
            || <Update as AccountCount>::NEEDS_EVENT_CPI;
        if !instruction_data.is_empty() && instruction_data[0] == 0xFF {
            if __QUASAR_NEEDS_EVENT_CPI {
                return __handle_event(ptr, instruction_data);
            }
            return Err(ProgramError::InvalidInstructionData);
        }
        {
            let __program_id: &[u8; 32] = unsafe {
                &*(instruction_data.as_ptr().add(instruction_data.len())
                    as *const [u8; 32])
            };
            const __U64_SIZE: usize = core::mem::size_of::<u64>();
            let __num_accounts = unsafe { *(ptr as *const u64) };
            let __accounts_start = unsafe { (ptr as *mut u8).add(__U64_SIZE) };
            if instruction_data.len() < 1usize {
                return Err(ProgramError::InvalidInstructionData);
            }
            let __disc: [u8; 1usize] = unsafe {
                *(instruction_data.as_ptr() as *const [u8; 1usize])
            };
            match __disc {
                [0] => {
                    if (__num_accounts as usize) < <Initialize as AccountCount>::COUNT {
                        return Err(ProgramError::NotEnoughAccountKeys);
                    }
                    if <Initialize as AccountCount>::COUNT >= 8usize {
                        __quasar_direct_initialize(
                            __program_id,
                            __accounts_start,
                            unsafe { instruction_data.get_unchecked(1usize..) },
                        )
                    } else {
                        {
                            let mut __buf = core::mem::MaybeUninit::<
                                [AccountView; <Initialize as AccountCount>::COUNT],
                            >::uninit();
                            let __remaining_ptr = unsafe {
                                <Initialize>::parse_accounts(
                                    __accounts_start,
                                    &mut __buf,
                                    unsafe {
                                        &*(__program_id as *const [u8; 32]
                                            as *const quasar_lang::prelude::Address)
                                    },
                                )?
                            };
                            let mut __accounts = unsafe { __buf.assume_init() };
                            let __data_after_disc = unsafe {
                                instruction_data.get_unchecked(1usize..)
                            };
                            initialize(unsafe {
                                Context::from_raw_parts(
                                    __program_id,
                                    &mut __accounts,
                                    __data_after_disc,
                                    __remaining_ptr,
                                    instruction_data.as_ptr().sub(__U64_SIZE),
                                )
                            })
                        }
                    }
                }
                [1] => {
                    if (__num_accounts as usize) < <Update as AccountCount>::COUNT {
                        return Err(ProgramError::NotEnoughAccountKeys);
                    }
                    if <Update as AccountCount>::COUNT >= 8usize {
                        __quasar_direct_update(
                            __program_id,
                            __accounts_start,
                            unsafe { instruction_data.get_unchecked(1usize..) },
                        )
                    } else {
                        {
                            let mut __buf = core::mem::MaybeUninit::<
                                [AccountView; <Update as AccountCount>::COUNT],
                            >::uninit();
                            let __remaining_ptr = unsafe {
                                <Update>::parse_accounts(
                                    __accounts_start,
                                    &mut __buf,
                                    unsafe {
                                        &*(__program_id as *const [u8; 32]
                                            as *const quasar_lang::prelude::Address)
                                    },
                                )?
                            };
                            let mut __accounts = unsafe { __buf.assume_init() };
                            let __data_after_disc = unsafe {
                                instruction_data.get_unchecked(1usize..)
                            };
                            update(unsafe {
                                Context::from_raw_parts(
                                    __program_id,
                                    &mut __accounts,
                                    __data_after_disc,
                                    __remaining_ptr,
                                    instruction_data.as_ptr().sub(__U64_SIZE),
                                )
                            })
                        }
                    }
                }
                _ => Err(ProgramError::InvalidInstructionData),
            }
        }
    }
    #[unsafe(no_mangle)]
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    #[allow(unexpected_cfgs)]
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
    #[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
    pub mod cpi {
        use super::*;
        __initialize_instruction!(InitializeInstruction, [0u8], { amount : u64 });
        __update_instruction!(UpdateInstruction, [1u8], {});
    }
}
#[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
extern crate alloc;
#[allow(unexpected_cfgs)]
#[cfg(all(any(target_os = "solana", target_arch = "bpf"), feature = "alloc"))]
extern crate alloc;
#[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
pub use quasar_demo::cpi;
#[cfg(any(target_os = "solana", target_arch = "bpf"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    quasar_lang::abort_program()
}
#[allow(unexpected_cfgs)]
#[cfg(feature = "alloc")]
quasar_lang::heap_alloc!();
#[allow(unexpected_cfgs)]
#[cfg(not(feature = "alloc"))]
quasar_lang::no_alloc!();
#[cfg(feature = "idl-build")]
quasar_lang::__private_inventory::submit! {
    quasar_lang::idl_build::InstructionFragment { build : { fn __build() ->
    quasar_lang::idl_build::__reexport::IdlInstruction {
    quasar_lang::idl_build::__reexport::IdlInstruction { name :
    quasar_lang::idl_build::s("initialize"), discriminator :
    quasar_lang::idl_build::vec![0u8], docs : quasar_lang::idl_build::Vec::new(),
    accounts : quasar_lang::idl_build::Vec::new(), args :
    quasar_lang::idl_build::vec![quasar_lang::idl_build::__reexport::IdlArg { name :
    quasar_lang::idl_build::s("amount"), ty :
    quasar_lang::idl_build::__reexport::IdlType::Primitive(quasar_lang::idl_build::s("u64")),
    codec : None, docs : quasar_lang::idl_build::Vec::new(), }], layout :
    Some(quasar_lang::idl_build::__reexport::IdlLayout::Fixed { fields :
    quasar_lang::idl_build::vec![quasar_lang::idl_build::s("amount")], }),
    remaining_accounts : None, } } __build }, accounts_struct_name : "Initialize",
    discriminator_source :
    quasar_lang::idl_build::InstructionDiscriminatorSource::Explicit, }
}
#[cfg(feature = "idl-build")]
quasar_lang::__private_inventory::submit! {
    quasar_lang::idl_build::InstructionFragment { build : { fn __build() ->
    quasar_lang::idl_build::__reexport::IdlInstruction {
    quasar_lang::idl_build::__reexport::IdlInstruction { name :
    quasar_lang::idl_build::s("update"), discriminator :
    quasar_lang::idl_build::vec![1u8], docs : quasar_lang::idl_build::Vec::new(),
    accounts : quasar_lang::idl_build::Vec::new(), args : quasar_lang::idl_build::vec![],
    layout : None, remaining_accounts : None, } } __build }, accounts_struct_name :
    "Update", discriminator_source :
    quasar_lang::idl_build::InstructionDiscriminatorSource::Explicit, }
}
/// Assemble all IDL fragments and return JSON.
#[cfg(feature = "idl-build")]
pub fn __quasar_build_idl() -> quasar_lang::idl_build::String {
    let address = quasar_lang::idl_build::address_to_base58(&crate::ID);
    let idl = quasar_lang::idl_build::build_idl(
        &address,
        "quasar_demo",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
    );
    quasar_lang::idl_build::__reexport::serde_json::to_string_pretty(&idl)
        .expect("generated IDL should serialize")
}
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
