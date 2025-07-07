use saturn_account_macros::Accounts;
use saturn_account_parser::codec::BorshAccount;

#[derive(Accounts)]
struct ReallocNoPayer<'info> {
    #[account(realloc, space = 16)]
    data: BorshAccount<'info, u64>,
}

fn main() {} 