use arch_program::pubkey::Pubkey;
use saturn_account_macros::Accounts;
use saturn_account_parser::{
    codec::{BorshAccount, ZeroCopyAccount},
    AccountLoader,
};

use crate::{LiquidityPoolConfig, LiquidityPoolShard};

#[derive(Accounts)]
pub struct AddPoolShardsAccounts<'info> {
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
        shards,
        len = 10,
        seeds = &[b"SHARD"],
        program_id = Pubkey::default(),
    )]
    pub shards: Vec<AccountLoader<'info, LiquidityPoolShard>>,
}
