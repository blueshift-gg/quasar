use solana_account_view::AccountView;
use solana_address::Address;
use solana_program_error::ProgramError;

pub trait FromAccountView<'info>: Sized {
    fn from_account_view(view: &'info AccountView) -> Result<Self, ProgramError>;
}

pub trait Owner {
    const OWNER: Address;
}

pub trait Program {
    const ID: Address;
}

pub trait Discriminator {
    const DISCRIMINATOR: u8;
}

pub trait Space {
    const SPACE: usize;
}

pub trait AccountCheck {
    #[inline(always)]
    fn check(_view: &AccountView) -> Result<(), ProgramError> { Ok(()) }
}

pub trait ParseAccounts<'info>: Sized {
    type Bumps: Copy;
    fn parse(accounts: &'info [AccountView]) -> Result<(Self, Self::Bumps), ProgramError>;
}

pub trait AsAccountView {
    fn to_account_view(&self) -> &AccountView;

    #[inline(always)]
    fn address(&self) -> &Address {
        self.to_account_view().address()
    }
}

pub trait QuasarAccount: Sized + Discriminator + Space {
    fn deserialize(data: &[u8]) -> Result<Self, ProgramError>;
    fn serialize(&self, data: &mut [u8]) -> Result<(), ProgramError>;
}

pub trait ZeroCopyDeref: Owner {
    type Target;

    unsafe fn deref_data(ptr: *const u8) -> *const Self::Target;
}

impl<T: QuasarAccount + Owner> ZeroCopyDeref for T {
    type Target = T;

    #[inline(always)]
    unsafe fn deref_data(ptr: *const u8) -> *const T {
        unsafe { ptr.add(1) as *const T }
    }
}
