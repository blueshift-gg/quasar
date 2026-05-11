use {
    crate::{
        prelude::*,
        traits::{
            check_account_count, AccountBumps, AccountGroup, ParseAccountsRaw,
            ParseAccountsUnchecked,
        },
    },
    core::mem::MaybeUninit,
};

/// Fixed-size repeated account group.
///
/// `AccountsArray<T, N>` always consumes exactly `N * T::COUNT` accounts, where
/// `T` is another `#[derive(Accounts)]` struct.
pub struct AccountsArray<T, const N: usize> {
    items: [T; N],
}

impl<T, const N: usize> AccountsArray<T, N> {
    #[inline(always)]
    pub fn as_slice(&self) -> &[T] {
        &self.items
    }

    #[inline(always)]
    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.items.iter()
    }

    #[inline(always)]
    pub const fn len(&self) -> usize {
        N
    }

    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        N == 0
    }
}

impl<T, const N: usize> core::ops::Deref for AccountsArray<T, N> {
    type Target = [T; N];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.items
    }
}

impl<T, const N: usize> AsRef<[T]> for AccountsArray<T, N> {
    #[inline(always)]
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T, const N: usize> AccountCount for AccountsArray<T, N>
where
    T: AccountCount,
{
    const COUNT: usize = T::COUNT * N;
    const NEEDS_EVENT_CPI: bool = N > 0 && T::NEEDS_EVENT_CPI;
}

unsafe impl<T, const N: usize> ParseAccountsRaw for AccountsArray<T, N>
where
    T: ParseAccountsRaw,
{
    #[inline(always)]
    unsafe fn parse_accounts_raw(
        mut input: *mut u8,
        base: *mut AccountView,
        offset: usize,
        program_id: &Address,
    ) -> Result<*mut u8, ProgramError> {
        let mut i = 0usize;
        while i < N {
            input = T::parse_accounts_raw(input, base, offset + i * T::COUNT, program_id)?;
            i += 1;
        }
        Ok(input)
    }
}

impl<'input, T, const N: usize> ParseAccounts<'input> for AccountsArray<T, N>
where
    T: ParseAccounts<'input> + ParseAccountsUnchecked<'input> + AccountCount,
{
    type Bumps = [T::Bumps; N];

    #[inline(always)]
    fn parse(
        accounts: &'input mut [AccountView],
        program_id: &Address,
    ) -> Result<(Self, Self::Bumps), ProgramError> {
        check_account_count(accounts.len(), Self::COUNT)?;
        unsafe { Self::parse_unchecked(accounts, program_id) }
    }

    #[inline(always)]
    fn parse_with_instruction_data(
        accounts: &'input mut [AccountView],
        data: &[u8],
        program_id: &Address,
    ) -> Result<(Self, Self::Bumps), ProgramError> {
        check_account_count(accounts.len(), Self::COUNT)?;
        unsafe { Self::parse_with_instruction_data_unchecked(accounts, data, program_id) }
    }

    const HAS_EPILOGUE: bool = T::HAS_EPILOGUE;

    #[inline(always)]
    fn epilogue(&mut self) -> Result<(), ProgramError> {
        let mut i = 0usize;
        while i < N {
            self.items[i].epilogue()?;
            i += 1;
        }
        Ok(())
    }
}

unsafe impl<'input, T, const N: usize> ParseAccountsUnchecked<'input> for AccountsArray<T, N>
where
    T: ParseAccounts<'input> + ParseAccountsUnchecked<'input> + AccountCount,
{
    #[inline(always)]
    unsafe fn parse_unchecked(
        accounts: &'input mut [AccountView],
        program_id: &Address,
    ) -> Result<(Self, Self::Bumps), ProgramError> {
        Self::parse_with_instruction_data_unchecked(accounts, &[], program_id)
    }

    #[inline(always)]
    unsafe fn parse_with_instruction_data_unchecked(
        accounts: &'input mut [AccountView],
        data: &[u8],
        program_id: &Address,
    ) -> Result<(Self, Self::Bumps), ProgramError> {
        let mut items = MaybeUninit::<[T; N]>::uninit();
        let mut bumps = MaybeUninit::<[T::Bumps; N]>::uninit();
        let items_ptr = items.as_mut_ptr() as *mut T;
        let bumps_ptr = bumps.as_mut_ptr() as *mut T::Bumps;

        let mut rest = accounts;
        let mut i = 0usize;
        while i < N {
            let (chunk, next) = rest.split_at_mut(T::COUNT);
            rest = next;
            let (item, item_bumps) =
                match T::parse_with_instruction_data_unchecked(chunk, data, program_id) {
                    Ok(parsed) => parsed,
                    Err(err) => {
                        let mut j = 0usize;
                        while j < i {
                            unsafe {
                                core::ptr::drop_in_place(items_ptr.add(j));
                                core::ptr::drop_in_place(bumps_ptr.add(j));
                            }
                            j += 1;
                        }
                        return Err(err);
                    }
                };
            core::ptr::write(items_ptr.add(i), item);
            core::ptr::write(bumps_ptr.add(i), item_bumps);
            i += 1;
        }

        Ok((
            Self {
                items: items.assume_init(),
            },
            bumps.assume_init(),
        ))
    }
}

impl<T, const N: usize> AccountBumps for AccountsArray<T, N>
where
    T: AccountBumps,
{
    type Bumps = [T::Bumps; N];
}

impl<T, const N: usize> AccountGroup for AccountsArray<T, N> where T: AccountGroup {}
