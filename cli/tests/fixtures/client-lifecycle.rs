use quasar_lang::prelude::*;

declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
pub struct ConfigV1 { pub count: u32 }
#[account(discriminator = 2)]
pub struct ConfigV2 { pub count: u64 }
impl From<ConfigV1> for ConfigV2 {
    fn from(old: ConfigV1) -> Self { Self { count: old.count.into() } }
}
#[account(discriminator = 3)]
#[seeds(b"vault")]
pub struct Vault {
    pub count: u64,
    #[max_len(16)]
    pub nums: Vec<u64>,
}

#[derive(InstructionData)]
pub struct Payload { pub payload_hash: [u8; 32] }

#[derive(Accounts)]
pub struct Touch {
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
    #[account(mut, migrate(from = ConfigV1, payer = payer))]
    pub config: Account<ConfigV2>,
    #[account(init(payer = payer), seeds())]
    pub vault: Pda<Account<Vault>>,
}

#[program]
mod client_lifecycle {
    use super::*;
    #[instruction(1)]
    pub fn touch(_ctx: Ctx<Touch, Payload>) -> Result<()> { Ok(()) }
}
