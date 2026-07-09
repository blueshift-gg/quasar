#[repr(C)]
pub struct MakeEvent {
    pub escrow: Address,
    pub maker: Address,
    pub deposit: u64,
    pub receive: u64,
}
const _: () = assert!(
    core::mem::size_of:: < MakeEvent > () == 80,
    "event struct has padding; cannot use memcpy serialization"
);
impl quasar_lang::traits::Event for MakeEvent {
    const DISCRIMINATOR: &'static [u8] = &[0];
    const DATA_SIZE: usize = 80usize;
    #[inline(always)]
    fn write_data(&self, buf: &mut [u8]) {
        unsafe {
            core::ptr::copy_nonoverlapping(
                self as *const Self as *const u8,
                buf.as_mut_ptr(),
                80,
            );
        }
    }
    #[inline(always)]
    fn emit(
        &self,
        f: impl FnOnce(&[u8]) -> Result<(), ProgramError>,
    ) -> Result<(), ProgramError> {
        const __DATA_SIZE: usize = 80usize;
        const __BUF_SIZE: usize = 1 + 1usize + __DATA_SIZE;
        let mut buf = core::mem::MaybeUninit::<[u8; __BUF_SIZE]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        let data_offset = unsafe {
            quasar_lang::event::write_cpi_disc(ptr, Self::DISCRIMINATOR)
        };
        self.write_data(unsafe {
            core::slice::from_raw_parts_mut(ptr.add(data_offset), __DATA_SIZE)
        });
        f(unsafe { buf.assume_init_ref() })
    }
}
impl MakeEvent {
    #[inline(always)]
    pub fn emit_log(&self) {
        let mut buf = core::mem::MaybeUninit::<[u8; 81usize]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        let data_offset = unsafe {
            quasar_lang::event::write_log_disc(
                ptr,
                <Self as quasar_lang::traits::Event>::DISCRIMINATOR,
            )
        };
        <Self as quasar_lang::traits::Event>::write_data(
            self,
            unsafe { core::slice::from_raw_parts_mut(ptr.add(data_offset), 80usize) },
        );
        quasar_lang::log::log_data(&[unsafe { buf.assume_init_ref() }]);
    }
}
#[cfg(feature = "idl-build")]
quasar_lang::__private_inventory::submit! {
    quasar_lang::idl_build::EventFragment { build : { fn __build() ->
    (quasar_lang::idl_build::__reexport::IdlEventDef,
    quasar_lang::idl_build::__reexport::IdlTypeDef,) {
    (quasar_lang::idl_build::__reexport::IdlEventDef { name :
    quasar_lang::idl_build::s("MakeEvent"), discriminator :
    quasar_lang::idl_build::vec![0u8], docs : quasar_lang::idl_build::Vec::new(), ty :
    None, }, quasar_lang::idl_build::__reexport::IdlTypeDef { name :
    quasar_lang::idl_build::s("MakeEvent"), kind :
    quasar_lang::idl_build::__reexport::IdlTypeDefKind::Struct, docs :
    quasar_lang::idl_build::Vec::new(), fields :
    quasar_lang::idl_build::vec![quasar_lang::idl_build::__reexport::IdlFieldDef { name :
    quasar_lang::idl_build::s("escrow"), ty :
    quasar_lang::idl_build::__reexport::IdlType::Primitive(quasar_lang::idl_build::s("pubkey")),
    codec : None, docs : quasar_lang::idl_build::Vec::new(), },
    quasar_lang::idl_build::__reexport::IdlFieldDef { name :
    quasar_lang::idl_build::s("maker"), ty :
    quasar_lang::idl_build::__reexport::IdlType::Primitive(quasar_lang::idl_build::s("pubkey")),
    codec : None, docs : quasar_lang::idl_build::Vec::new(), },
    quasar_lang::idl_build::__reexport::IdlFieldDef { name :
    quasar_lang::idl_build::s("deposit"), ty :
    quasar_lang::idl_build::__reexport::IdlType::Primitive(quasar_lang::idl_build::s("u64")),
    codec : None, docs : quasar_lang::idl_build::Vec::new(), },
    quasar_lang::idl_build::__reexport::IdlFieldDef { name :
    quasar_lang::idl_build::s("receive"), ty :
    quasar_lang::idl_build::__reexport::IdlType::Primitive(quasar_lang::idl_build::s("u64")),
    codec : None, docs : quasar_lang::idl_build::Vec::new(), }], variants :
    quasar_lang::idl_build::Vec::new(), repr : None, alias : None, fallback : None, codec
    : None, layout : None, space : None, semantics : None, },) } __build }, }
}
