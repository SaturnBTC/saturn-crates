use saturn_account_macros::Accounts;
use saturn_account_parser::codec::ZeroCopyAccount;
use arch_program::account::AccountInfo;

#[derive(Accounts)]
struct ReallocShards<'info> {
    #[account(signer)]
    payer: AccountInfo<'info>,
    #[account(shards, len = 2, realloc, payer = payer, space = 16, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
    shards: Vec<ZeroCopyAccount<'info, u64>>, // element must be zero copy for shards
}

fn main() {} 