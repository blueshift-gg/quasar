use {crate::prelude::*, core::marker::PhantomData};

/// Account wrapper for type-safe on-chain migration from `From` to `To`.
///
/// During the handler, `source()` provides zero-copy read access to the
/// source account's data. At epilogue, `finish()` reads the source data,
/// calls `Migrate::migrate()`, reallocs, and writes the target.
///
/// # Lifecycle
/// ```text
/// parse:    validate source owner + discriminator
/// handler:  read source fields via source() (zero-copy)
/// epilogue: finish() → migrate → realloc → write target
/// ```
///
/// # Safety
/// `source()` validates the source discriminator is still present before
/// returning a reference. After `finish()` runs, `source()` returns `None`
/// (the buffer now contains target data). This prevents type confusion from
/// interpreting target bytes as the source type.
///
/// `finish()` is idempotent — if the target discriminator is already present
/// (e.g., the epilogue was re-entered), the call is a no-op.
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

impl<From, To> crate::account_load::AccountLoad for Migration<From, To>
where
    From: AsAccountView + CheckOwner + AccountCheck + crate::traits::StaticView,
{
    type BehaviorTarget = Self;
    type Params = <From as AccountCheck>::Params;

    #[inline(always)]
    fn check(view: &AccountView, field_name: &str) -> Result<(), ProgramError> {
        // Validate against source type (owner + discriminator).
        crate::validation::check_account::<From>(view, field_name)
    }

    #[inline(always)]
    fn validate(&self, params: &Self::Params) -> Result<(), ProgramError> {
        <From as AccountCheck>::validate(&self.__view, params)
    }
}

impl<From, To> Migration<From, To>
where
    From: crate::traits::Discriminator,
{
    /// Read source account data before migration (zero-copy pointer cast).
    ///
    /// Returns `None` after `finish()` has run (the buffer now contains
    /// target data). This prevents type confusion.
    ///
    /// # Example
    /// ```ignore
    /// let val: u64 = ctx.accounts.config.source().unwrap().value.into();
    /// ```
    #[inline(always)]
    pub fn source(&self) -> Option<&From> {
        let data = unsafe { self.__view.borrow_unchecked() };
        if data.starts_with(<From as crate::traits::Discriminator>::DISCRIMINATOR) {
            Some(unsafe { &*(&self.__view as *const AccountView as *const From) })
        } else {
            None
        }
    }
}

impl<From, To> Migration<From, To>
where
    From: core::ops::Deref + crate::traits::Discriminator + crate::traits::Owner,
    From::Target: Sized + crate::traits::Migrate<To::Target>,
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
        // Check that neither discriminator is a prefix of the other.
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

    /// Perform migration. Safe to call multiple times (idempotent after first).
    ///
    /// On first call: reads source via zero-copy pointer cast, calls
    /// `Migrate::migrate()`, reallocs, writes target disc + data.
    /// On subsequent calls: detects target discriminator already present,
    /// returns Ok(()) immediately (no-op).
    ///
    /// After this method returns, `source()` returns `None` — the buffer
    /// now contains target data.
    #[inline(always)]
    pub fn finish(
        &mut self,
        payer: &AccountView,
        rent: &crate::sysvars::rent::Rent,
    ) -> Result<(), ProgramError> {
        // Force compile-time assertion evaluation.
        #[allow(clippy::let_unit_value)]
        {
            let _ = Self::_OWNER_EQ;
            let _ = Self::_DISC_NEQ;
            let _ = Self::_STACK_BUDGET;
        }

        let view = unsafe { &mut *(&mut self.__view as *mut AccountView) };

        // Guard: if target discriminator already present, migration already done.
        let data = unsafe { view.borrow_unchecked() };
        if data.starts_with(<To as crate::traits::Discriminator>::DISCRIMINATOR) {
            return Ok(());
        }
        // Verify source discriminator is still present (not some third state).
        if !data.starts_with(<From as crate::traits::Discriminator>::DISCRIMINATOR) {
            return Err(ProgramError::InvalidAccountData);
        }

        // 1. Zero-copy read: pointer cast into account data buffer.
        let source_ref: &From::Target = unsafe {
            &*(view
                .data_ptr()
                .add(<From as crate::traits::Discriminator>::DISCRIMINATOR.len())
                as *const From::Target)
        };

        // 2. Call user's migration logic (produces target on stack).
        let target: To::Target =
            <From::Target as crate::traits::Migrate<To::Target>>::migrate(source_ref);

        // 3. Realloc (handles both grow and shrink lamport adjustments).
        super::realloc_account(view, <To as crate::traits::Space>::SPACE, payer, Some(rent))?;

        // 4. Write target discriminator + data.
        unsafe {
            core::ptr::copy_nonoverlapping(
                <To as crate::traits::Discriminator>::DISCRIMINATOR.as_ptr(),
                view.data_mut_ptr(),
                <To as crate::traits::Discriminator>::DISCRIMINATOR.len(),
            );
            core::ptr::copy_nonoverlapping(
                &target as *const To::Target as *const u8,
                view.data_mut_ptr()
                    .add(<To as crate::traits::Discriminator>::DISCRIMINATOR.len()),
                core::mem::size_of::<To::Target>(),
            );
        }
        Ok(())
    }
}
