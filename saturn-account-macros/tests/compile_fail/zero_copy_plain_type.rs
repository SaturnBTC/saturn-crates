use saturn_account_macros::Accounts;

#[derive(Accounts)]
struct ZeroCopyPlain<'info> {
    #[account(zero_copy)]
    acc: arch_program::account::AccountInfo<'info>,
}

fn main() {} 