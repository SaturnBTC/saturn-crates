use saturn_account_macros::Accounts;

#[derive(Accounts)]
struct LenOnSingle<'info> {
    #[account(len = 2)]
    acc: arch_program::account::AccountInfo<'info>,
}

fn main() {} 