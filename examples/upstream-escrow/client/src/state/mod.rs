pub mod escrow;

pub use escrow::*;

pub enum ProgramAccount {
    Escrow(Escrow),
}

pub fn decode_account(data: &[u8]) -> Option<ProgramAccount> {
    if data.starts_with(ESCROW_ACCOUNT_DISCRIMINATOR) {
        return wincode::deserialize::<Escrow>(data)
            .ok()
            .map(ProgramAccount::Escrow);
    }
    None
}
