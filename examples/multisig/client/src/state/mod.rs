pub mod multisig_config;

pub use multisig_config::*;

pub enum ProgramAccount {
    MultisigConfig(MultisigConfig),
}

pub fn decode_account(data: &[u8]) -> Option<ProgramAccount> {
    if data.starts_with(MULTISIG_CONFIG_ACCOUNT_DISCRIMINATOR) {
        let value = wincode::deserialize::<MultisigConfig>(data).ok()?;
        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != data.len() {
            return None;
        }
        return Some(ProgramAccount::MultisigConfig(value));
    }
    None
}
