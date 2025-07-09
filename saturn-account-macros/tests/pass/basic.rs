use arch_program::account::AccountInfo;
use saturn_account_macros::Accounts;
use saturn_account_parser as _;
use saturn_account_parser::codec::Account;

#[derive(Accounts)]
struct MyAccounts<'info> {
    #[account(signer)]
    caller: Account<'info, u64>,
    #[account(len = 1)]
    system_program: Vec<AccountInfo<'info>>,
}

fn main() {}
