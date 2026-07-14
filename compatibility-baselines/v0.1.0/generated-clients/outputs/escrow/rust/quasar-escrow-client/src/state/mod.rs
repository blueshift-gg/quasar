pub mod escrow;

pub use escrow::*;

pub enum ProgramAccount {
    Escrow(Escrow),
}

pub fn decode_account(data: &[u8]) -> Option<ProgramAccount> {
    if data.starts_with(ESCROW_ACCOUNT_DISCRIMINATOR) {
        let value = wincode::deserialize::<Escrow>(data).ok()?;
        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != data.len() { return None; }
        return Some(ProgramAccount::Escrow(value));
    }
    None
}
