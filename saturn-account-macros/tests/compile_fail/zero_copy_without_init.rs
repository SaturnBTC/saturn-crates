use saturn_account_macros::Accounts;
use saturn_account_parser::codec::BorshAccount;

#[derive(Accounts)]
struct ZeroCopyNoInit<'info> {
    #[account(zero_copy)]
    // Wrong on purpose. BorshAccount is not a ZeroCopyAccount.
    data: BorshAccount<'info, u64>,
}

fn main() {}
