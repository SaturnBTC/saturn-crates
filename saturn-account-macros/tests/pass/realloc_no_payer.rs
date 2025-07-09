use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;

#[derive(Accounts)]
struct ReallocNoPayerPass<'info> {
    #[account(mut)]
    data: Account<'info, u64>,
    // resize from zero-copy or borsh perspective without specifying a payer
    #[account(mut, realloc, space = 32)]
    bigger: Account<'info, u64>,
}

fn main() {} 