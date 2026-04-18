#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

/// Composite field type: QuasarSerialize generates InstructionArg + SettingsZc.
/// The #[account] macro's map_to_pod_type maps Settings → SettingsZc in the
/// ZC struct, and zc_assign_from_value calls to_zc() for set_inner.
#[derive(Copy, Clone, QuasarSerialize)]
pub struct Settings {
    pub value: u64,
    pub flags: u8,
}

#[account(discriminator = 1, set_inner)]
pub struct Config {
    pub settings: Settings,
    pub bump: u8,
}

fn main() {}
