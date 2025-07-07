use saturn_account_macros::Accounts;
use saturn_account_parser::codec::BorshAccount;

#[derive(Accounts)]
struct MissingLifetime {
    #[account(signer)]
    user: BorshAccount<'static, u64>,
}

fn main() {} 