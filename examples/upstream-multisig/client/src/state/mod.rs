pub mod multisig_config;

pub use multisig_config::*;

pub enum ProgramAccount {
    MultisigConfig(MultisigConfig),
}

pub fn decode_account(data: &[u8]) -> Option<ProgramAccount> {
    if data.starts_with(MULTISIG_CONFIG_ACCOUNT_DISCRIMINATOR) {
        return wincode::deserialize::<MultisigConfig>(data)
            .ok()
            .map(ProgramAccount::MultisigConfig);
    }
    None
}
