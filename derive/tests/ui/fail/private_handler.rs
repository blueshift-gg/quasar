use quasar_lang::prelude::*;

declare_id!("FaNXqQYPePivF3hVJ3ZdM74zzMa9P4QU7HX7t6RrDygg");

#[derive(Accounts)]
pub struct Greet {
    pub user: Signer,
}

#[program]
mod greeting {
    use super::*;

    #[instruction(0)]
    fn greet(_ctx: Ctx<Greet>) -> Result {
        Ok(())
    }
}

fn main() {}
