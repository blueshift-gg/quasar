#[repr(u32)]
pub enum TestError {
    Unauthorized = 6000u32,
    InvalidAddress = 6001u32,
    CustomConstraint = 6002u32,
}
impl From<TestError> for ::quasar_lang::__solana_program_error::ProgramError {
    #[inline(always)]
    fn from(e: TestError) -> Self {
        ::quasar_lang::__solana_program_error::ProgramError::Custom(e as u32)
    }
}
impl TryFrom<u32> for TestError {
    type Error = ::quasar_lang::__solana_program_error::ProgramError;
    #[inline(always)]
    fn try_from(error: u32) -> Result<Self, Self::Error> {
        match error {
            6000u32 => Ok(TestError::Unauthorized),
            6001u32 => Ok(TestError::InvalidAddress),
            6002u32 => Ok(TestError::CustomConstraint),
            _ => {
                Err(::quasar_lang::__solana_program_error::ProgramError::InvalidArgument)
            }
        }
    }
}
#[cfg(feature = "idl-build")]
::quasar_lang::__private_inventory::submit! {
    ::quasar_lang::idl_build::ErrorFragment { build : { fn __build() ->
    ::quasar_lang::idl_build::Vec < ::quasar_lang::idl_build::__reexport::IdlErrorDef > {
    ::quasar_lang::idl_build::vec![::quasar_lang::idl_build::__reexport::IdlErrorDef {
    code : 6000u32, name : ::quasar_lang::idl_build::s("Unauthorized"), msg : None, },
    ::quasar_lang::idl_build::__reexport::IdlErrorDef { code : 6001u32, name :
    ::quasar_lang::idl_build::s("InvalidAddress"), msg : None, },
    ::quasar_lang::idl_build::__reexport::IdlErrorDef { code : 6002u32, name :
    ::quasar_lang::idl_build::s("CustomConstraint"), msg : None, }] } __build }, }
}
