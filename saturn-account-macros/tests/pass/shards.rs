use saturn_account_macros::Accounts;
use saturn_account_parser as _;
use saturn_account_parser::codec::AccountLoader;

#[derive(Accounts)]
struct ShardOnly<'info> {
    #[account(shards, len = 5)]
    shards: Vec<AccountLoader<'info, u64>>,
}

fn main() {}
