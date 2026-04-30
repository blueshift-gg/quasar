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

impl<From: AsAccountView, To> AsAccountView for Migration<From, To> {
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

impl<From, To> crate::account_load::AccountLoad for Migration<From, To>
where
    From: AsAccountView + CheckOwner + AccountCheck + crate::traits::StaticView,
{
    type BehaviorTarget = Self;

    #[inline(always)]
    fn check(view: &AccountView, field_name: &str) -> Result<(), ProgramError> {
        // Validate against source type (owner + discriminator).
        crate::validation::check_account::<From>(view, field_name)
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
        // From is repr(transparent) over AccountView, so this cast is sound.
        let disc_len = <From as crate::traits::Discriminator>::DISCRIMINATOR.len();
        unsafe { &*(self.__view.data_ptr().add(disc_len) as *const From::Target) }
    }
}

// ---------------------------------------------------------------------------
// Migration API — matches Anchor's pattern
// ---------------------------------------------------------------------------

impl<From, To> Migration<From, To>
where
    From: core::ops::Deref + crate::traits::Discriminator + crate::traits::Owner,
    From::Target: Sized,
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
        // Force compile-time assertion evaluation.
        #[allow(clippy::let_unit_value)]
        {
            let _ = Self::_OWNER_EQ;
            let _ = Self::_DISC_NEQ;
            let _ = Self::_STACK_BUDGET;
        }

        let view = unsafe { &mut *(&mut self.__view as *mut AccountView) };
        let required = <To as crate::traits::Space>::SPACE;
        if view.data_len() < required {
            return Err(ProgramError::AccountDataTooSmall);
        }
        let data = unsafe { view.borrow_unchecked() };

        // Already migrated → error (like Anchor's AccountAlreadyMigrated)
        if data.starts_with(<To as crate::traits::Discriminator>::DISCRIMINATOR) {
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        // Source disc must still be present
        if !data.starts_with(<From as crate::traits::Discriminator>::DISCRIMINATOR) {
            return Err(ProgramError::InvalidAccountData);
        }

        // Write target discriminator + data.
        unsafe {
            let disc = <To as crate::traits::Discriminator>::DISCRIMINATOR;
            core::ptr::copy_nonoverlapping(disc.as_ptr(), view.data_mut_ptr(), disc.len());
            core::ptr::copy_nonoverlapping(
                &new_data as *const To::Target as *const u8,
                view.data_mut_ptr().add(disc.len()),
                core::mem::size_of::<To::Target>(),
            );
        }
        Ok(())
    }

    /// Idempotent migration. If already migrated, returns `Ok(())`.
    /// Otherwise, writes the new data.
    #[inline(always)]
    pub fn migrate_idempotent(&mut self, new_data: To::Target) -> Result<(), ProgramError> {
        let data = unsafe { self.__view.borrow_unchecked() };
        if data.starts_with(<To as crate::traits::Discriminator>::DISCRIMINATOR) {
            return Ok(());
        }
        self.migrate(new_data)
    }

    /// Check if migration has been completed (discriminator matches `To`).
    #[inline(always)]
    pub fn is_migrated(&self) -> bool {
        let data = unsafe { self.__view.borrow_unchecked() };
        data.starts_with(<To as crate::traits::Discriminator>::DISCRIMINATOR)
    }
}

impl<From, To> crate::traits::FieldLifecycle for Migration<From, To>
where
    From: core::ops::Deref + crate::traits::Discriminator + crate::traits::Owner,
    From::Target: Sized,
    To: core::ops::Deref
        + crate::traits::Owner
        + crate::traits::Space
        + crate::traits::Discriminator,
    To::Target: Sized,
{
    const HAS_LIFECYCLE_BEFORE: bool = true;
    const HAS_LIFECYCLE_EXIT: bool = true;

    #[inline(always)]
    fn before_lifecycle(
        &mut self,
        payer: Option<&AccountView>,
        ctx: &crate::ops::OpCtx<'_>,
    ) -> Result<(), ProgramError> {
        let target_space = <To as crate::traits::Space>::SPACE;
        let view = unsafe { &mut *(self as *mut Self as *mut AccountView) };
        if view.data_len() < target_space {
            let payer = payer.ok_or(ProgramError::NotEnoughAccountKeys)?;
            crate::accounts::realloc_account(view, target_space, payer, Some(ctx.rent()?))?;
        }
        Ok(())
    }

    #[inline(always)]
    fn exit_lifecycle(
        &mut self,
        payer: Option<&AccountView>,
        ctx: &crate::ops::OpCtx<'_>,
    ) -> Result<(), ProgramError> {
        // Normalize to target size after the handler. Growth already happened
        // before the handler so migrate() can write safely; this handles shrink
        // migrations and explicit over-allocation.
        let view = unsafe { &mut *(self as *mut Self as *mut AccountView) };
        let target_space = <To as crate::traits::Space>::SPACE;
        if view.data_len() != target_space {
            let payer = payer.ok_or(ProgramError::NotEnoughAccountKeys)?;
            crate::accounts::realloc_account(view, target_space, payer, Some(ctx.rent()?))?;
        }
        // Verify .migrate() was called
        if self.is_migrated() {
            Ok(())
        } else {
            Err(ProgramError::Custom(
                crate::error::QuasarError::AccountNotMigrated as u32,
            ))
        }
    }
}
