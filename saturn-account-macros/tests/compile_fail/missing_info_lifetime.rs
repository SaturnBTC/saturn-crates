use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;

#[derive(Accounts)]
struct MissingLifetime {
    #[account(signer)]
    user: Account<'static, u64>,
}

fn main() {} 