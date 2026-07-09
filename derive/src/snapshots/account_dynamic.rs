#[repr(transparent)]
pub struct DynamicAccount {
    __view: AccountView,
}
/**Raw `#[repr(C)]` data layout for [`DynamicAccount`].

Use this type when constructing account data values (e.g., for [`Migrate`](quasar_lang::traits::Migrate) implementations).*/
pub type DynamicAccountData = __dynamic_account_zc::DynamicAccountZc;
unsafe impl StaticView for DynamicAccount {}
impl AsAccountView for DynamicAccount {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        &self.__view
    }
}
impl core::ops::Deref for DynamicAccount {
    type Target = __dynamic_account_zc::DynamicAccountZc;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe {
            &*(self.__view.data_ptr().add(1usize)
                as *const __dynamic_account_zc::DynamicAccountZc)
        }
    }
}
impl core::ops::DerefMut for DynamicAccount {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            &mut *(self.__view.data_mut_ptr().add(1usize)
                as *mut __dynamic_account_zc::DynamicAccountZc)
        }
    }
}
#[doc(hidden)]
pub mod __dynamic_account_zc {
    use super::*;
    use quasar_lang::__zeropod as zeropod;
    #[derive(zeropod::ZeroPod)]
    #[zeropod(compact)]
    pub struct __Schema {
        pub name: zeropod::pod::PodString<8, 1usize>,
        pub tags: zeropod::pod::PodVec<
            <Address as quasar_lang::instruction_arg::InstructionArg>::Zc,
            2,
            2usize,
        >,
    }
    pub type DynamicAccountZc = __SchemaHeader;
}
const _: () = assert!(
    core::mem::size_of:: < DynamicAccount > () == core::mem::size_of:: < AccountView >
    (), "Pod-dynamic struct must be #[repr(transparent)] over AccountView"
);
impl Discriminator for DynamicAccount {
    const DISCRIMINATOR: &'static [u8] = &[5];
}
impl Owner for DynamicAccount {
    const OWNER: Address = crate::ID;
}
impl Space for DynamicAccount {
    const SPACE: usize = 1usize
        + <__dynamic_account_zc::__Schema as quasar_lang::ZeroPodCompact>::HEADER_SIZE;
}
impl quasar_lang::account_load::AccountLoad for DynamicAccount {
    #[inline(always)]
    fn check(
        view: &quasar_lang::__internal::AccountView,
    ) -> Result<(), quasar_lang::__solana_program_error::ProgramError> {
        let __data = unsafe { view.borrow_unchecked() };
        let __min = 1usize
            + <__dynamic_account_zc::__Schema as quasar_lang::ZeroPodCompact>::HEADER_SIZE;
        if __data.len() < __min {
            return Err(ProgramError::AccountDataTooSmall);
        }
        if unsafe { *__data.get_unchecked(0usize) } != 5 {
            return Err(ProgramError::InvalidAccountData);
        }
        <__dynamic_account_zc::__Schema as quasar_lang::ZeroPodCompact>::validate(unsafe {
                __data.get_unchecked(1usize..)
            })
            .map_err(|_| ProgramError::InvalidAccountData)?;
        Ok(())
    }
    #[inline(always)]
    fn check_checked(
        view: &quasar_lang::__internal::AccountView,
    ) -> Result<(), quasar_lang::__solana_program_error::ProgramError> {
        let __data_ref = view.try_borrow()?;
        let __data: &[u8] = &__data_ref;
        let __min = 1usize
            + <__dynamic_account_zc::__Schema as quasar_lang::ZeroPodCompact>::HEADER_SIZE;
        if __data.len() < __min {
            return Err(ProgramError::AccountDataTooSmall);
        }
        if unsafe { *__data.get_unchecked(0usize) } != 5 {
            return Err(ProgramError::InvalidAccountData);
        }
        <__dynamic_account_zc::__Schema as quasar_lang::ZeroPodCompact>::validate(unsafe {
                __data.get_unchecked(1usize..)
            })
            .map_err(|_| ProgramError::InvalidAccountData)?;
        Ok(())
    }
}
impl quasar_lang::account_init::AccountInit for DynamicAccount {
    type InitParams<'a> = ();
    #[inline(always)]
    fn init<'a, R: quasar_lang::ops::RentAccess>(
        ctx: quasar_lang::account_init::InitCtx<'a, R>,
        _params: &(),
    ) -> Result<(), quasar_lang::prelude::ProgramError> {
        quasar_lang::account_init::init_account(
            ctx.payer,
            ctx.target,
            ctx.space,
            ctx.program_id,
            ctx.signers,
            ctx.rent.get()?,
            <Self as quasar_lang::traits::Discriminator>::DISCRIMINATOR,
        )
    }
}
impl quasar_lang::ops::SupportsRealloc for DynamicAccount {}
impl DynamicAccount {
    pub const MIN_SPACE: usize = 1usize
        + <__dynamic_account_zc::__Schema as quasar_lang::ZeroPodCompact>::HEADER_SIZE;
    pub const MAX_SPACE: usize = Self::MIN_SPACE + 8
        + 2
            * core::mem::size_of::<
                <Address as quasar_lang::instruction_arg::InstructionArg>::Zc,
            >();
    #[inline(always)]
    pub fn name(&self) -> &str {
        let __data = unsafe { self.__view.borrow_unchecked() };
        let __r = unsafe {
            __dynamic_account_zc::__SchemaRef::new_unchecked(
                __data.get_unchecked(1usize..),
            )
        };
        __r.name()
    }
    #[inline(always)]
    pub fn tags(
        &self,
    ) -> &[<Address as quasar_lang::instruction_arg::InstructionArg>::Zc] {
        let __data = unsafe { self.__view.borrow_unchecked() };
        let __r = unsafe {
            __dynamic_account_zc::__SchemaRef::new_unchecked(
                __data.get_unchecked(1usize..),
            )
        };
        __r.tags()
    }
}
#[must_use = "bind the as_mut() guard to mutate cached fields; changes auto-save on drop"]
pub struct DynamicAccountCompactMut<'a> {
    __view: &'a mut AccountView,
    __payer: &'a AccountView,
    pub name: quasar_lang::pod::PodString<8, 1usize>,
    pub tags: quasar_lang::pod::PodVec<
        <Address as quasar_lang::instruction_arg::InstructionArg>::Zc,
        2,
        2usize,
    >,
}
impl<'a> core::ops::Deref for DynamicAccountCompactMut<'a> {
    type Target = __dynamic_account_zc::DynamicAccountZc;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe {
            &*(self.__view.data_ptr().add(1usize)
                as *const __dynamic_account_zc::DynamicAccountZc)
        }
    }
}
impl<'a> DynamicAccountCompactMut<'a> {
    pub fn save(&mut self) -> Result<(), ProgramError> {
        let __tail_size: usize = 0 + self.name.len()
            + self.tags.len()
                * core::mem::size_of::<
                    <Address as quasar_lang::instruction_arg::InstructionArg>::Zc,
                >();
        let __new_total = 1usize
            + <__dynamic_account_zc::__Schema as quasar_lang::ZeroPodCompact>::HEADER_SIZE
            + __tail_size;
        let __old_total = self.__view.data_len();
        if __new_total != __old_total {
            quasar_lang::accounts::account::realloc_account(
                self.__view,
                __new_total,
                self.__payer,
                None,
            )?;
        }
        let __compact_data = unsafe {
            core::slice::from_raw_parts_mut(
                self.__view.data_mut_ptr().add(1usize),
                __new_total - 1usize,
            )
        };
        let mut __compact = unsafe {
            __dynamic_account_zc::__SchemaMut::new_unchecked(__compact_data)
        };
        __compact
            .set_name(self.name.as_str())
            .map_err(|_| ProgramError::InvalidAccountData)?;
        __compact
            .set_tags(self.tags.as_slice())
            .map_err(|_| ProgramError::InvalidAccountData)?;
        __compact.commit().map_err(|_| ProgramError::InvalidAccountData)?;
        Ok(())
    }
    pub fn reload(&mut self) {
        let (name, tags) = {
            let __data = unsafe { self.__view.borrow_unchecked() };
            let __r = unsafe {
                __dynamic_account_zc::__SchemaRef::new_unchecked(
                    __data.get_unchecked(1usize..),
                )
            };
            let mut name = quasar_lang::pod::PodString::<8, 1usize>::default();
            if !name.set(__r.name()) {
                quasar_lang::abort_program();
            }
            let mut tags = quasar_lang::pod::PodVec::<
                <Address as quasar_lang::instruction_arg::InstructionArg>::Zc,
                2,
                2usize,
            >::default();
            if !tags.set_from_slice(__r.tags()) {
                quasar_lang::abort_program();
            }
            (name, tags)
        };
        self.name = name;
        self.tags = tags;
    }
}
impl<'a> Drop for DynamicAccountCompactMut<'a> {
    fn drop(&mut self) {
        if self.save().is_err() {
            quasar_lang::abort_program();
        }
    }
}
impl DynamicAccount {
    #[inline(always)]
    pub fn as_mut<'a>(
        &'a mut self,
        payer: &'a AccountView,
    ) -> DynamicAccountCompactMut<'a> {
        let (name, tags) = {
            let __data = unsafe { self.__view.borrow_unchecked() };
            let __r = unsafe {
                __dynamic_account_zc::__SchemaRef::new_unchecked(
                    __data.get_unchecked(1usize..),
                )
            };
            let mut name = quasar_lang::pod::PodString::<8, 1usize>::default();
            if !name.set(__r.name()) {
                quasar_lang::abort_program();
            }
            let mut tags = quasar_lang::pod::PodVec::<
                <Address as quasar_lang::instruction_arg::InstructionArg>::Zc,
                2,
                2usize,
            >::default();
            if !tags.set_from_slice(__r.tags()) {
                quasar_lang::abort_program();
            }
            (name, tags)
        };
        let __view = unsafe { &mut *(&mut self.__view as *mut AccountView) };
        DynamicAccountCompactMut {
            __view,
            __payer: payer,
            name,
            tags,
        }
    }
}
pub struct DynamicAccountCompactWriter<'a> {
    __view: &'a mut AccountView,
    __payer: &'a AccountView,
    __rent_lpb: u64,
    __rent_threshold: u64,
    __name: Option<&'a str>,
    __tags: Option<&'a [<Address as quasar_lang::instruction_arg::InstructionArg>::Zc]>,
}
impl<'a> core::ops::Deref for DynamicAccountCompactWriter<'a> {
    type Target = __dynamic_account_zc::DynamicAccountZc;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe {
            &*(self.__view.data_ptr().add(1usize)
                as *const __dynamic_account_zc::DynamicAccountZc)
        }
    }
}
impl<'a> core::ops::DerefMut for DynamicAccountCompactWriter<'a> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            &mut *(self.__view.data_mut_ptr().add(1usize)
                as *mut __dynamic_account_zc::DynamicAccountZc)
        }
    }
}
impl<'a> DynamicAccountCompactWriter<'a> {
    #[inline(always)]
    pub fn set_name(&mut self, value: &'a str) -> Result<(), ProgramError> {
        if value.len() > 8 {
            return Err(QuasarError::DynamicFieldTooLong.into());
        }
        self.__name = Some(value);
        Ok(())
    }
    #[inline(always)]
    pub fn set_tags(
        &mut self,
        value: &'a [<Address as quasar_lang::instruction_arg::InstructionArg>::Zc],
    ) -> Result<(), ProgramError> {
        if value.len() > 2 {
            return Err(QuasarError::DynamicFieldTooLong.into());
        }
        self.__tags = Some(value);
        Ok(())
    }
    pub fn commit(&mut self) -> Result<(), ProgramError> {
        let name = self.__name.ok_or(QuasarError::CompactWriterFieldNotSet)?;
        let tags = self.__tags.ok_or(QuasarError::CompactWriterFieldNotSet)?;
        let __new_total = 1usize
            + <__dynamic_account_zc::__Schema as quasar_lang::ZeroPodCompact>::HEADER_SIZE
            + name.len()
            + tags.len()
                * core::mem::size_of::<
                    <Address as quasar_lang::instruction_arg::InstructionArg>::Zc,
                >();
        let __old_total = self.__view.data_len();
        if __new_total != __old_total {
            quasar_lang::accounts::account::realloc_account_raw(
                self.__view,
                __new_total,
                self.__payer,
                self.__rent_lpb,
                self.__rent_threshold,
            )?;
        }
        let __compact_data = unsafe {
            core::slice::from_raw_parts_mut(
                self.__view.data_mut_ptr().add(1usize),
                __new_total - 1usize,
            )
        };
        let mut __compact = unsafe {
            __dynamic_account_zc::__SchemaMut::new_unchecked(__compact_data)
        };
        __compact.set_name(name).map_err(|_| ProgramError::InvalidAccountData)?;
        __compact.set_tags(tags).map_err(|_| ProgramError::InvalidAccountData)?;
        __compact.commit().map_err(|_| ProgramError::InvalidAccountData)?;
        Ok(())
    }
}
impl DynamicAccount {
    #[inline(always)]
    pub fn compact_writer<'a>(
        &'a mut self,
        payer: &'a AccountView,
        rent_lpb: u64,
        rent_threshold: u64,
    ) -> DynamicAccountCompactWriter<'a> {
        let __view = unsafe { &mut *(&mut self.__view as *mut AccountView) };
        DynamicAccountCompactWriter {
            __view,
            __payer: payer,
            __rent_lpb: rent_lpb,
            __rent_threshold: rent_threshold,
            __name: None,
            __tags: None,
        }
    }
}
pub struct DynamicAccountInner<'a> {
    pub name: &'a str,
    pub tags: &'a [<Address as quasar_lang::instruction_arg::InstructionArg>::Zc],
}
impl DynamicAccount {
    #[inline(always)]
    pub fn set_inner(
        &mut self,
        inner: DynamicAccountInner<'_>,
        payer: &AccountView,
        rent_lpb: u64,
        rent_threshold: u64,
    ) -> Result<(), ProgramError> {
        let name = inner.name;
        let tags = inner.tags;
        if name.len() > 8 {
            return Err(QuasarError::DynamicFieldTooLong.into());
        }
        if tags.len() > 2 {
            return Err(QuasarError::DynamicFieldTooLong.into());
        }
        let __space = Self::MIN_SPACE + name.len()
            + tags.len()
                * core::mem::size_of::<
                    <Address as quasar_lang::instruction_arg::InstructionArg>::Zc,
                >();
        let __view = unsafe { &mut *(self as *mut Self as *mut AccountView) };
        if __space != __view.data_len() {
            quasar_lang::accounts::account::realloc_account_raw(
                __view,
                __space,
                payer,
                rent_lpb,
                rent_threshold,
            )?;
        }
        let __ptr = __view.data_mut_ptr();
        let __zc = unsafe {
            &mut *(__ptr.add(1usize) as *mut __dynamic_account_zc::DynamicAccountZc)
        };
        let __compact_data = unsafe {
            core::slice::from_raw_parts_mut(
                __ptr.add(1usize),
                __view.data_len() - 1usize,
            )
        };
        let mut __compact = unsafe {
            __dynamic_account_zc::__SchemaMut::new_unchecked(__compact_data)
        };
        __compact.set_name(name).map_err(|_| ProgramError::InvalidAccountData)?;
        __compact.set_tags(tags).map_err(|_| ProgramError::InvalidAccountData)?;
        __compact.commit().map_err(|_| ProgramError::InvalidAccountData)?;
        Ok(())
    }
}
#[cfg(feature = "idl-build")]
quasar_lang::__private_inventory::submit! {
    quasar_lang::idl_build::AccountFragment { build : { fn __build() ->
    (quasar_lang::idl_build::__reexport::IdlAccountDef,
    quasar_lang::idl_build::__reexport::IdlTypeDef,) {
    (quasar_lang::idl_build::__reexport::IdlAccountDef { name :
    quasar_lang::idl_build::s("DynamicAccount"), discriminator :
    quasar_lang::idl_build::vec![5u8], docs : quasar_lang::idl_build::Vec::new(), space :
    Some(quasar_lang::idl_build::__reexport::IdlSpace { discriminator : Some(1usize), min
    : < DynamicAccount as quasar_lang::traits::Space > ::SPACE as u64, max : None,
    formula : None, }), }, quasar_lang::idl_build::__reexport::IdlTypeDef { name :
    quasar_lang::idl_build::s("DynamicAccount"), kind :
    quasar_lang::idl_build::__reexport::IdlTypeDefKind::Struct, docs :
    quasar_lang::idl_build::Vec::new(), fields :
    quasar_lang::idl_build::vec![quasar_lang::idl_build::__reexport::IdlFieldDef { name :
    quasar_lang::idl_build::s("name"), ty :
    quasar_lang::idl_build::__reexport::IdlType::Primitive(quasar_lang::idl_build::s("string")),
    codec : Some(quasar_lang::idl_build::__reexport::IdlCodec::SizePrefixed { prefix :
    quasar_lang::idl_build::__reexport::ScalarRepr { ty :
    quasar_lang::idl_build::s("u8"), endian :
    quasar_lang::idl_build::__reexport::Endian::Le, }, storage :
    quasar_lang::idl_build::__reexport::Storage::Tail, max_bytes : Some(8), max_items :
    None, encoding : Some(quasar_lang::idl_build::s("utf8")), item : None, }), docs :
    quasar_lang::idl_build::Vec::new(), },
    quasar_lang::idl_build::__reexport::IdlFieldDef { name :
    quasar_lang::idl_build::s("tags"), ty :
    quasar_lang::idl_build::__reexport::IdlType::Vec { vec :
    quasar_lang::idl_build::Box::new(quasar_lang::idl_build::__reexport::IdlType::Primitive(quasar_lang::idl_build::s("pubkey"))),
    }, codec : Some(quasar_lang::idl_build::__reexport::IdlCodec::SizePrefixed { prefix :
    quasar_lang::idl_build::__reexport::ScalarRepr { ty :
    quasar_lang::idl_build::s("u16"), endian :
    quasar_lang::idl_build::__reexport::Endian::Le, }, storage :
    quasar_lang::idl_build::__reexport::Storage::Tail, max_bytes : None, max_items :
    Some(2), encoding : None, item : None, }), docs : quasar_lang::idl_build::Vec::new(),
    }], variants : quasar_lang::idl_build::Vec::new(), repr : None, alias : None,
    fallback : None, codec : None, layout :
    Some(quasar_lang::idl_build::__reexport::IdlLayout::Compact { inline_fields :
    quasar_lang::idl_build::vec![], tail_fields :
    quasar_lang::idl_build::vec![quasar_lang::idl_build::s("name"),
    quasar_lang::idl_build::s("tags")], wire :
    quasar_lang::idl_build::__reexport::CompactWire::InlineFieldsThenTailHeadersThenTailPayloads,
    }), space : None, semantics : None, },) } __build }, }
}
