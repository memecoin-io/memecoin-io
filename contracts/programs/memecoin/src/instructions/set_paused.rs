use anchor_lang::prelude::*;

use crate::errors::MemecoinError;
use crate::state::Config;

#[derive(Accounts)]
pub struct SetPaused<'info> {
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [Config::SEED],
        bump = config.bump,
        has_one = admin @ MemecoinError::Unauthorized,
    )]
    pub config: Box<Account<'info, Config>>,
}

pub fn handler(ctx: Context<SetPaused>, paused: bool) -> Result<()> {
    ctx.accounts.config.paused = paused;
    msg!("Memecoin.io paused = {}", paused);
    Ok(())
}
