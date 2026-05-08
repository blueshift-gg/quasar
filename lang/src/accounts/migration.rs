use {crate::prelude::*, core::marker::PhantomData};

/// Account wrapper for type-safe on-chain migration from `From` to `To`.
///
/// Follows Anchor's migration pattern:
/// - Deref to `From::Target` for reading old data before migration
/// - `.migrate(new_data)` writes the `To` discriminator + data
/// - Exit enforcement: epilogue errors if `.migrate()` was not called
///
/// # Lifecycle
/// ```text
/// parse:    validate source owner + discriminator
/// handler:  read old fields via Deref, call .migrate(new_data)
/// epilogue: verify migration completed (disc matches To)
/// ```
///
/// # Realloc
/// Growth to the target type's space happens before the handler so
/// `.migrate()` can write safely. Shrink/normalization happens in the
/// epilogue. A `payer = ...` field is required whenever the size changes.
///
/// # Example
/// ```ignore
/// #[derive(Accounts)]
/// pub struct MigrateConfig {
///     #[account(mut, payer = payer)]
///     pub config: Migration<ConfigV1, ConfigV2>,
///     #[account(mut)]
///     pub payer: Signer,
///     pub system_program: Program<SystemProgram>,
/// }
///
/// impl MigrateConfig {
///     pub fn handler(&mut self) -> Result<(), ProgramError> {
///         let old_val = self.config.value;  // Deref to ConfigV1Data
///         self.config.migrate(ConfigV2Data {
///             value: old_val,
///             extra: PodU32::from(42),
///         })
///     }
/// }
/// ```
#[repr(transparent)]
pub struct Migration<From, To> {
    __view: AccountView,
    _marker: PhantomData<(From, To)>,
}

impl<From, To> AsAccountView for Migration<From, To> {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        &self.__view
    }
}

// Safety: Migration is repr(transparent) over AccountView.
unsafe impl<From, To> crate::traits::StaticView for Migration<From, To> {}

// Migration supports realloc (needed when target is larger than source).
impl<From, To> crate::ops::SupportsRealloc for Migration<From, To> {}

// Space for Migration is the target's space (what we're reallocating to).
impl<From, To: crate::traits::Space> crate::traits::Space for Migration<From, To> {
    const SPACE: usize = <To as crate::traits::Space>::SPACE;
}

impl<From, To> Migration<From, To> {
    #[inline(always)]
    fn view(&self) -> &AccountView {
        &self.__view
    }

    #[inline(always)]
    fn view_mut(&mut self) -> &mut AccountView {
        &mut self.__view
    }

    #[inline(always)]
    fn discriminator_is<Ty: crate::traits::Discriminator>(&self) -> bool {
        Self::data_starts_with::<Ty>(unsafe { self.view().borrow_unchecked() })
    }

    #[inline(always)]
    fn data_starts_with<Ty: crate::traits::Discriminator>(data: &[u8]) -> bool {
        data.starts_with(<Ty as crate::traits::Discriminator>::DISCRIMINATOR)
    }
}

impl<From, To> Migration<From, To>
where
    To: crate::traits::Space,
{
    /// Grow the account to the target type's space before the handler.
    /// Called by generated parse body when the derive detects a Migration
    /// field.
    #[inline(always)]
    pub fn grow_to_target(
        &mut self,
        payer: &AccountView,
        ctx: &crate::ops::OpCtxWithRent<'_>,
    ) -> Result<(), ProgramError> {
        let target_space = <To as crate::traits::Space>::SPACE;
        if self.view().data_len() >= target_space {
            return Ok(());
        }
        self.realloc_to_target(payer, ctx)
    }

    /// Normalize the account to exact target space in the epilogue.
    /// Called by generated epilogue when the derive detects a Migration field.
    #[inline(always)]
    pub fn normalize_to_target(
        &mut self,
        payer: &AccountView,
        ctx: &crate::ops::OpCtxWithRent<'_>,
    ) -> Result<(), ProgramError> {
        let target_space = <To as crate::traits::Space>::SPACE;
        if self.view().data_len() == target_space {
            return Ok(());
        }
        self.realloc_to_target(payer, ctx)
    }

    #[inline(always)]
    fn realloc_to_target(
        &mut self,
        payer: &AccountView,
        ctx: &crate::ops::OpCtxWithRent<'_>,
    ) -> Result<(), ProgramError> {
        crate::accounts::realloc_account(
            self.view_mut(),
            <To as crate::traits::Space>::SPACE,
            payer,
            Some(ctx.rent),
        )
    }
}

impl<From, To> Migration<From, To>
where
    To: crate::traits::Discriminator,
{
    /// Check if migration has been completed (discriminator matches `To`).
    #[inline(always)]
    pub fn is_migrated(&self) -> bool {
        self.discriminator_is::<To>()
    }
}

impl<From, To> crate::account_load::AccountLoad for Migration<From, To>
where
    From: CheckOwner + crate::account_load::AccountLoad,
    To: crate::traits::Space + crate::traits::Discriminator,
{
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        From::check_owner(view)?;
        From::check(view)
    }

    #[inline(always)]
    fn check_checked(view: &AccountView) -> Result<(), ProgramError> {
        From::check_owner(view)?;
        From::check_checked(view)
    }
}

// ---------------------------------------------------------------------------
// Deref to From::Target — read old data before migration
// ---------------------------------------------------------------------------

impl<From, To> core::ops::Deref for Migration<From, To>
where
    From: core::ops::Deref + crate::traits::Discriminator,
    From::Target: Sized,
{
    type Target = From::Target;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        // SAFETY: check() validated disc + data_len during load.
        let disc_len = <From as crate::traits::Discriminator>::DISCRIMINATOR.len();
        unsafe { &*(self.view().data_ptr().add(disc_len) as *const From::Target) }
    }
}

// ---------------------------------------------------------------------------
// Migration API — matches Anchor's pattern
// ---------------------------------------------------------------------------

impl<From, To> Migration<From, To>
where
    From: crate::traits::Discriminator + crate::traits::Owner,
    To: core::ops::Deref
        + crate::traits::Owner
        + crate::traits::Space
        + crate::traits::Discriminator,
    To::Target: Sized,
{
    // Compile-time safety assertions.
    const _OWNER_EQ: () = assert!(
        crate::keys_eq_const(
            &<From as crate::traits::Owner>::OWNER,
            &<To as crate::traits::Owner>::OWNER,
        ),
        "migration source and target must have the same Owner"
    );
    const _DISC_NEQ: () = {
        let src = <From as crate::traits::Discriminator>::DISCRIMINATOR;
        let tgt = <To as crate::traits::Discriminator>::DISCRIMINATOR;
        let min_len = if src.len() < tgt.len() {
            src.len()
        } else {
            tgt.len()
        };
        let mut i = 0;
        let mut prefix_match = true;
        while i < min_len {
            if src[i] != tgt[i] {
                prefix_match = false;
            }
            i += 1;
        }
        assert!(
            !prefix_match,
            "migration source and target discriminators must not be prefixes of each other"
        );
    };
    const _STACK_BUDGET: () = assert!(
        core::mem::size_of::<To::Target>() < 3584,
        "migration target type too large for sBPF 4KB stack frame"
    );

    #[inline(always)]
    fn assert_migration_contract() {
        #[allow(clippy::let_unit_value)]
        {
            let _ = Self::_OWNER_EQ;
            let _ = Self::_DISC_NEQ;
            let _ = Self::_STACK_BUDGET;
        }
    }

    #[inline(always)]
    fn check_source_ready(&self) -> Result<(), ProgramError> {
        if self.view().data_len() < <To as crate::traits::Space>::SPACE {
            return Err(ProgramError::AccountDataTooSmall);
        }
        let data = unsafe { self.view().borrow_unchecked() };
        if Self::data_starts_with::<To>(data) {
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        if !Self::data_starts_with::<From>(data) {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }

    #[inline(always)]
    fn write_target(&mut self, new_data: &To::Target) {
        let view = self.view_mut();
        let disc = <To as crate::traits::Discriminator>::DISCRIMINATOR;
        unsafe {
            core::ptr::copy_nonoverlapping(disc.as_ptr(), view.data_mut_ptr(), disc.len());
            core::ptr::copy_nonoverlapping(
                new_data as *const To::Target as *const u8,
                view.data_mut_ptr().add(disc.len()),
                core::mem::size_of::<To::Target>(),
            );
        }
    }

    /// Migrate to the new schema. Writes the `To` discriminator + new data.
    ///
    /// The generated account lifecycle grows the account before the handler
    /// when `To::SPACE` is larger than the current data length.
    ///
    /// Returns `Err(AccountAlreadyInitialized)` if already migrated.
    ///
    /// # Example
    /// ```ignore
    /// let old_val = self.config.value;  // read via Deref
    /// self.config.migrate(ConfigV2Data {
    ///     value: old_val,
    ///     extra: PodU32::from(42),
    /// })?;
    /// ```
    #[inline(always)]
    pub fn migrate(&mut self, new_data: To::Target) -> Result<(), ProgramError> {
        Self::assert_migration_contract();
        self.check_source_ready()?;
        self.write_target(&new_data);
        Ok(())
    }

    /// Idempotent migration. If already migrated, returns `Ok(())`.
    /// Otherwise, writes the new data.
    #[inline(always)]
    pub fn migrate_idempotent(&mut self, new_data: To::Target) -> Result<(), ProgramError> {
        Self::assert_migration_contract();
        if self.is_migrated() {
            return Ok(());
        }
        self.check_source_ready()?;
        self.write_target(&new_data);
        Ok(())
    }
}
