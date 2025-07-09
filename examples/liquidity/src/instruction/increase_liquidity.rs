use arch_program::pubkey::Pubkey;
use saturn_account_macros::Accounts;
use saturn_account_parser::{
    codec::{BorshAccount, ZeroCopyAccount},
    AccountLoader,
};

use crate::{LiquidityPoolConfig, LiquidityPoolShard, LiquidityPosition};

#[derive(Accounts)]
pub struct IncreaseLiquidityAccounts<'info> {
    #[account(signer)]
    pub caller: BorshAccount<'info, u64>,

    #[account(
        seeds = &[b"CONFIG"],
        program_id = Pubkey::default(),
    )]
    pub config: ZeroCopyAccount<'info, LiquidityPoolConfig>,

    #[account(
        bump,
        seeds = &[b"CONFIG"],
        program_id = Pubkey::default(),
    )]
    pub config_bump: [u8; 1],

    #[account(
        seeds = &[b"POSITION", &caller.info().key.0],
        program_id = Pubkey::default(),
        init,
        payer = caller,
    )]
    pub position: ZeroCopyAccount<'info, LiquidityPosition>,

    #[account(
        bump,
        seeds = &[b"POSITION", &caller.info().key.0],
        program_id = Pubkey::default(),
    )]
    pub position_bump: [u8; 1],

    #[account(
        shards,
        len = 50,
        seeds = &[b"SHARD"],
        program_id = Pubkey::default()
    )]
    pub shards: Vec<AccountLoader<'info, LiquidityPoolShard>>,
}
