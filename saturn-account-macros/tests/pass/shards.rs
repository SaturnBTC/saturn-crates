use saturn_account_macros::Accounts;
use saturn_account_parser as _;
use saturn_account_parser::codec::AccountLoader;
use saturn_account_discriminator_derive::Discriminator;

#[derive(bytemuck::Pod, bytemuck::Zeroable, Discriminator, Copy, Clone)]
#[repr(C)]
pub struct Shard {
    pub value: u64,
}

#[derive(Accounts)]
struct ShardOnly<'info> {
    #[account(shards, len = 5)]
    shards: Vec<AccountLoader<'info, Shard>>,
}

fn main() {}
