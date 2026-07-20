#[doc(hidden)]
#[derive(::quasar_lang::__zeropod::ZeroPod)]
pub struct __PayloadSchema {
    pub amount: u64,
    pub flag: bool,
}
#[doc(hidden)]
pub type PayloadZc = __PayloadSchemaZc;
#[doc(hidden)]
#[allow(unexpected_cfgs)]
mod __payload_zc_offchain {
    use super::*;
    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    unsafe impl<__C: wincode::config::ConfigCore> wincode::SchemaWrite<__C>
    for __PayloadSchemaZc {
        type Src = Self;
        fn size_of(_src: &Self) -> wincode::error::WriteResult<usize> {
            Ok(core::mem::size_of::<Self>())
        }
        fn write(
            mut __writer: impl wincode::io::Writer,
            src: &Self,
        ) -> wincode::error::WriteResult<()> {
            let __bytes = unsafe {
                core::slice::from_raw_parts(
                    src as *const Self as *const u8,
                    core::mem::size_of::<Self>(),
                )
            };
            __writer.write(__bytes)?;
            Ok(())
        }
    }
    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    unsafe impl<'__de, __C: wincode::config::ConfigCore> wincode::SchemaRead<'__de, __C>
    for __PayloadSchemaZc {
        type Dst = Self;
        fn read(
            mut __reader: impl wincode::io::Reader<'__de>,
            __dst: &mut core::mem::MaybeUninit<Self>,
        ) -> wincode::error::ReadResult<()> {
            let __bytes = __reader.take_scoped(core::mem::size_of::<Self>())?;
            let __zc = unsafe {
                core::ptr::read_unaligned(__bytes.as_ptr() as *const Self)
            };
            ::quasar_lang::__zeropod::ZcValidate::validate_ref(&__zc)
                .map_err(|_| wincode::error::ReadError::InvalidValue(
                    "pod validation failed",
                ))?;
            __dst.write(__zc);
            Ok(())
        }
    }
}
impl ::quasar_lang::instruction_arg::InstructionArg for Payload {
    type Zc = PayloadZc;
    #[inline(always)]
    fn from_zc(zc: &Self::Zc) -> Self {
        let pod = zc;
        Self {
            amount: <u64 as ::quasar_lang::instruction_arg::InstructionArg>::from_zc(
                &pod.amount,
            ),
            flag: <bool as ::quasar_lang::instruction_arg::InstructionArg>::from_zc(
                &pod.flag,
            ),
        }
    }
    #[inline(always)]
    fn to_zc(&self) -> Self::Zc {
        PayloadZc {
            amount: <u64 as ::quasar_lang::instruction_arg::InstructionArg>::to_zc(
                &self.amount,
            ),
            flag: <bool as ::quasar_lang::instruction_arg::InstructionArg>::to_zc(
                &self.flag,
            ),
        }
    }
    #[inline(always)]
    fn validate_zc(zc: &Self::Zc) -> Result<(), solana_program_error::ProgramError> {
        <Self::Zc as ::quasar_lang::__zeropod::ZcValidate>::validate_ref(zc)
            .map_err(|_| solana_program_error::ProgramError::InvalidInstructionData)
    }
}
impl From<Payload> for PayloadZc {
    #[inline(always)]
    fn from(v: Payload) -> Self {
        <Payload as ::quasar_lang::instruction_arg::InstructionArg>::to_zc(&v)
    }
}
impl From<PayloadZc> for Payload {
    #[inline(always)]
    fn from(v: PayloadZc) -> Self {
        <Payload as ::quasar_lang::instruction_arg::InstructionArg>::from_zc(&v)
    }
}
impl ::quasar_lang::ZcField for Payload {
    type Pod = PayloadZc;
    const POD_SIZE: usize = core::mem::size_of::<PayloadZc>();
}
#[doc(hidden)]
#[allow(unexpected_cfgs)]
mod __payload_offchain {
    use super::*;
    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    unsafe impl<__C: wincode::config::ConfigCore> wincode::SchemaWrite<__C> for Payload {
        type Src = Self;
        fn size_of(_src: &Self) -> wincode::error::WriteResult<usize> {
            Ok(core::mem::size_of::<PayloadZc>())
        }
        fn write(
            mut __writer: impl wincode::io::Writer,
            src: &Self,
        ) -> wincode::error::WriteResult<()> {
            let __zc = <Self as ::quasar_lang::instruction_arg::InstructionArg>::to_zc(
                src,
            );
            let __bytes = unsafe {
                core::slice::from_raw_parts(
                    &__zc as *const PayloadZc as *const u8,
                    core::mem::size_of::<PayloadZc>(),
                )
            };
            __writer.write(__bytes)?;
            Ok(())
        }
    }
    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    unsafe impl<'__de, __C: wincode::config::ConfigCore> wincode::SchemaRead<'__de, __C>
    for Payload {
        type Dst = Self;
        fn read(
            mut __reader: impl wincode::io::Reader<'__de>,
            __dst: &mut core::mem::MaybeUninit<Self>,
        ) -> wincode::error::ReadResult<()> {
            let __bytes = __reader.take_scoped(core::mem::size_of::<PayloadZc>())?;
            let __zc = unsafe {
                core::ptr::read_unaligned(__bytes.as_ptr() as *const PayloadZc)
            };
            <PayloadZc as ::quasar_lang::__zeropod::ZcValidate>::validate_ref(&__zc)
                .map_err(|_| wincode::error::ReadError::InvalidValue(
                    "pod validation failed",
                ))?;
            __dst
                .write(
                    <Self as ::quasar_lang::instruction_arg::InstructionArg>::from_zc(
                        &__zc,
                    ),
                );
            Ok(())
        }
    }
}
#[cfg(feature = "idl-build")]
::quasar_lang::__private_inventory::submit! {
    ::quasar_lang::idl_build::TypeFragment { build : { fn __build() ->
    ::quasar_lang::idl_build::__reexport::IdlTypeDef {
    ::quasar_lang::idl_build::__reexport::IdlTypeDef { name :
    ::quasar_lang::idl_build::s("Payload"), kind :
    ::quasar_lang::idl_build::__reexport::IdlTypeDefKind::Struct, docs :
    ::quasar_lang::idl_build::Vec::new(), fields :
    ::quasar_lang::idl_build::vec![::quasar_lang::idl_build::__reexport::IdlFieldDef {
    name : ::quasar_lang::idl_build::s("amount"), ty :
    ::quasar_lang::idl_build::__reexport::IdlType::Primitive(::quasar_lang::idl_build::s("u64")),
    codec : None, docs : ::quasar_lang::idl_build::Vec::new(), },
    ::quasar_lang::idl_build::__reexport::IdlFieldDef { name :
    ::quasar_lang::idl_build::s("flag"), ty :
    ::quasar_lang::idl_build::__reexport::IdlType::Primitive(::quasar_lang::idl_build::s("bool")),
    codec : None, docs : ::quasar_lang::idl_build::Vec::new(), }], variants :
    ::quasar_lang::idl_build::Vec::new(), repr : None, alias : None, fallback : None,
    codec : None, layout : None, space : None, semantics : None, } } __build }, }
}
