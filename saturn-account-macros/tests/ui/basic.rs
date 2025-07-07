use arch_program::account::AccountInfo;
use saturn_account_macros::Accounts;
use saturn_account_parser as _;
use saturn_account_parser::codec::BorshAccount;

#[derive(Accounts)]
struct MyAccounts<'info> {
    #[account(signer)]
    caller: BorshAccount<'info, u64>,
    #[account(len = 2)]
    pdas: Vec<AccountInfo<'info>>,
}

fn main() {} 