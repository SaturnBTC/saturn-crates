use saturn_account_macros::Accounts;

#[derive(Accounts)]
struct DupFlags<'info> {
    #[account(signer, signer)]
    user: arch_program::account::AccountInfo<'info>,
}

fn main() {} 