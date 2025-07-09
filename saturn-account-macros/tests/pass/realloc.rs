use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;

#[derive(Accounts)]
struct ReallocPass<'info> {
    #[account(signer, mut)]
    payer: Account<'info, u64>,
    #[account(mut, realloc, payer = payer, space = 8)]
    data: Account<'info, u64>,
}

fn main() {}
