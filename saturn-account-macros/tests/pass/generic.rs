use arch_program::account::AccountInfo;
use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;

#[derive(Accounts)]
struct GenericAccounts<'info, T: Copy> {
    #[account(signer)]
    owner: Account<'info, u64>,
    #[account(len = 3)]
    helpers: Vec<AccountInfo<'info>>,
    // generic phantom field to ensure generics do not break macro
    _pd: core::marker::PhantomData<T>,
}

fn main() {} 