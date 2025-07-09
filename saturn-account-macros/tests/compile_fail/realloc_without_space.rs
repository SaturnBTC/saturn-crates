use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;

#[derive(Accounts)]
struct ReallocNoSpace<'info> {
    #[account(signer)]
    payer: Account<'info, u64>,
    #[account(realloc, payer = payer)]
    data: Account<'info, u64>,
}

fn main() {} 