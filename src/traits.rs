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
    const DISCRIMINATOR: &'static [u8];
}

pub trait Space {
    const SPACE: usize;
}

pub trait AccountCheck {
    #[inline(always)]
    fn check(_view: &AccountView) -> Result<(), ProgramError> { Ok(()) }
}

pub trait AccountCount {
    const COUNT: usize;
}

pub trait QuasarAccount: Sized + Discriminator + Space {
    fn deserialize(data: &[u8]) -> Result<Self, ProgramError>;
    fn serialize(&self, data: &mut [u8]) -> Result<(), ProgramError>;
}
