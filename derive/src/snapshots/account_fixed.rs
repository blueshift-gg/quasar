#[repr(transparent)]
pub struct MixedAccount {
    __view: AccountView,
}
/**Raw `#[repr(C)]` data layout for [`MixedAccount`].

Use this type when constructing account data values (e.g., for [`Migrate`](quasar_lang::traits::Migrate) implementations).*/
pub type MixedAccountData = __mixed_account_zc::MixedAccountZc;
unsafe impl StaticView for MixedAccount {}
impl AsAccountView for MixedAccount {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        &self.__view
    }
}
impl core::ops::Deref for MixedAccount {
    type Target = __mixed_account_zc::MixedAccountZc;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe {
            &*(self.__view.data_ptr().add(1usize)
                as *const __mixed_account_zc::MixedAccountZc)
        }
    }
}
impl core::ops::DerefMut for MixedAccount {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            &mut *(self.__view.data_mut_ptr().add(1usize)
                as *mut __mixed_account_zc::MixedAccountZc)
        }
    }
}
#[doc(hidden)]
pub mod __mixed_account_zc {
    use super::*;
    use quasar_lang::__zeropod as zeropod;
    #[derive(zeropod::ZeroPod)]
    pub struct __Schema {
        pub authority: Address,
        pub value: u64,
    }
    pub type MixedAccountZc = __SchemaZc;
}
impl Discriminator for MixedAccount {
    const DISCRIMINATOR: &'static [u8] = &[6];
}
impl Owner for MixedAccount {
    const OWNER: Address = crate::ID;
}
impl Space for MixedAccount {
    const SPACE: usize = 1usize
        + <__mixed_account_zc::__Schema as quasar_lang::__zeropod::ZeroPodFixed>::SIZE;
}
impl quasar_lang::account_layout::AccountLayout for MixedAccount {
    type Schema = __mixed_account_zc::__Schema;
    const DATA_OFFSET: usize = 1usize;
}
impl quasar_lang::checks::Discriminator for MixedAccount {}
impl quasar_lang::checks::ZeroPod for MixedAccount {}
impl quasar_lang::account_load::AccountLoad for MixedAccount {
    #[inline(always)]
    fn check(
        view: &quasar_lang::__internal::AccountView,
    ) -> Result<(), quasar_lang::__solana_program_error::ProgramError> {
        <MixedAccount as quasar_lang::checks::Discriminator>::check(view)?;
        <MixedAccount as quasar_lang::checks::ZeroPod>::check(view)?;
        Ok(())
    }
    #[inline(always)]
    fn check_checked(
        view: &quasar_lang::__internal::AccountView,
    ) -> Result<(), quasar_lang::__solana_program_error::ProgramError> {
        <MixedAccount as quasar_lang::checks::Discriminator>::check_checked(view)?;
        <MixedAccount as quasar_lang::checks::ZeroPod>::check_checked(view)?;
        Ok(())
    }
}
impl quasar_lang::account_init::AccountInit for MixedAccount {
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
impl quasar_lang::ops::SupportsRealloc for MixedAccount {}
#[cfg(feature = "idl-build")]
quasar_lang::__private_inventory::submit! {
    quasar_lang::idl_build::AccountFragment { build : { fn __build() ->
    (quasar_lang::idl_build::__reexport::IdlAccountDef,
    quasar_lang::idl_build::__reexport::IdlTypeDef,) {
    (quasar_lang::idl_build::__reexport::IdlAccountDef { name :
    quasar_lang::idl_build::s("MixedAccount"), discriminator :
    quasar_lang::idl_build::vec![6u8], docs : quasar_lang::idl_build::Vec::new(), space :
    Some(quasar_lang::idl_build::__reexport::IdlSpace { discriminator : Some(1usize), min
    : < MixedAccount as quasar_lang::traits::Space > ::SPACE as u64, max : None, formula
    : None, }), }, quasar_lang::idl_build::__reexport::IdlTypeDef { name :
    quasar_lang::idl_build::s("MixedAccount"), kind :
    quasar_lang::idl_build::__reexport::IdlTypeDefKind::Struct, docs :
    quasar_lang::idl_build::Vec::new(), fields :
    quasar_lang::idl_build::vec![quasar_lang::idl_build::__reexport::IdlFieldDef { name :
    quasar_lang::idl_build::s("authority"), ty :
    quasar_lang::idl_build::__reexport::IdlType::Primitive(quasar_lang::idl_build::s("pubkey")),
    codec : None, docs : quasar_lang::idl_build::Vec::new(), },
    quasar_lang::idl_build::__reexport::IdlFieldDef { name :
    quasar_lang::idl_build::s("value"), ty :
    quasar_lang::idl_build::__reexport::IdlType::Primitive(quasar_lang::idl_build::s("u64")),
    codec : None, docs : quasar_lang::idl_build::Vec::new(), }], variants :
    quasar_lang::idl_build::Vec::new(), repr : None, alias : None, fallback : None, codec
    : None, layout : Some(quasar_lang::idl_build::__reexport::IdlLayout::Fixed { fields :
    quasar_lang::idl_build::vec![quasar_lang::idl_build::s("authority"),
    quasar_lang::idl_build::s("value")], }), space : None, semantics : None, },) }
    __build }, }
}
