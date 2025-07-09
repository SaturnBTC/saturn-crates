use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;

#[derive(Accounts)]
struct ReallocWithInit<'info> {
    #[account(signer)]
    payer: Account<'info, u64>,
    #[account(init, realloc, payer = payer, space = 8)]
    data: Account<'info, u64>,
}

fn main() {} 