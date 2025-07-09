use saturn_account_macros::Accounts;
use saturn_account_parser::codec::AccountLoader;
use arch_program::account::AccountInfo;
use saturn_account_discriminator_derive::Discriminator;

#[derive(bytemuck::Pod, bytemuck::Zeroable, Discriminator, Copy, Clone)]
#[repr(C)]
pub struct Shard {
    pub value: u64,
}

#[derive(Accounts)]
struct ReallocShards<'info> {
    #[account(signer, mut)]
    payer: AccountInfo<'info>,
    #[account(mut, shards, len = 2, realloc, payer = payer, space = 16, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
    shards: Vec<AccountLoader<'info, Shard>>,
}

fn main() {} 