use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[account]
pub struct Timelock {
    // Do we need to store this?
    pub receiver_account_pubkey: Pubkey,
    pub receiver_token_account_pubkey: Pubkey,
    pub temp_token_account_pubkey: Pubkey,
    pub unlock_timestamp: i64,
    pub bump: u8,
}

/*
Required initializer: hardcoded pubkey that has to sign. Why hardcoded?
This is also the authority for the tokens-to-lock.

Why change ownership of the tokens-to-lock, rather than just transfering them to the program?

Using seeds = [receiver.key()] means you can only have one timelock per receiver (rather than per receiver + mint)

Maybe simpler to use a single program authority pda?
*/

#[program]
pub mod anchor_timelock {
    use super::*;
    pub fn lock(ctx: Context<Lock>, timelock_bump: u8, lock_duration: i64) -> ProgramResult {
        let timelock = &mut ctx.accounts.timelock;
        timelock.unlock_timestamp = Clock::get()?
            .unix_timestamp
            .checked_add(lock_duration)
            .ok_or_else(|| TimelockError::DurationOverflow)?;
        timelock.receiver_account_pubkey = ctx.accounts.receiver.key();
        timelock.receiver_token_account_pubkey = ctx.accounts.receiver_tokens.key();
        timelock.bump = timelock_bump;

        anchor_spl::token::set_authority(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token::SetAuthority {
                    current_authority: ctx.accounts.initializer.to_account_info(),
                    account_or_mint: ctx.accounts.tokens_to_lock.to_account_info(),
                },
            ),
            spl_token::instruction::AuthorityType::AccountOwner,
            Some(ctx.accounts.timelock.key()),
        )?;

        Ok(())
    }

    pub fn unlock(ctx: Context<Unlock>) -> ProgramResult {
        if Clock::get()?.unix_timestamp < ctx.accounts.timelock.unlock_timestamp {
            return Err(TimelockError::HasntUnlockedYet.into());
        }

        anchor_spl::token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token::Transfer {
                    from: ctx.accounts.timelocked_tokens.to_account_info(),
                    to: ctx.accounts.receiver_tokens.to_account_info(),
                    authority: ctx.accounts.timelock.to_account_info(),
                },
                &[&[
                    &ctx.accounts.receiver.key().as_ref(),
                    &[ctx.accounts.timelock.bump],
                ]],
            ),
            ctx.accounts.timelocked_tokens.amount,
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(timelock_bump: u8)]
pub struct Lock<'info> {
    #[account(
        init,
        payer = initializer,
        space = 8 + 32 + 32 + 32 + 8 + 1,
        seeds = [receiver.key().as_ref()],
        bump = timelock_bump
    )]
    pub timelock: Account<'info, Timelock>,

    #[account(mut)]
    pub initializer: Signer<'info>,
    #[account(mut, constraint = tokens_to_lock.owner == initializer.key())]
    pub tokens_to_lock: Account<'info, TokenAccount>,

    pub receiver: AccountInfo<'info>,
    pub receiver_tokens: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Unlock<'info> {
    #[account(
        mut,
        close = receiver,
        constraint = timelock.receiver_account_pubkey == receiver.key(),
        constraint = timelock.receiver_token_account_pubkey == receiver_tokens.key()
    )]
    pub timelock: Account<'info, Timelock>,

    #[account(mut)]
    pub timelocked_tokens: Account<'info, TokenAccount>,

    #[account(mut)]
    pub receiver: AccountInfo<'info>,
    #[account(mut)]
    pub receiver_tokens: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[error]
pub enum TimelockError {
    #[msg("Duration caused overflow")]
    DurationOverflow,

    #[msg("Timelock hasn't unlocked yet")]
    HasntUnlockedYet,
}
