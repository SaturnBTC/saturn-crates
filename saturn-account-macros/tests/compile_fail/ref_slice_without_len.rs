use saturn_account_macros::Accounts;

#[derive(Accounts)]
struct RefSliceNoLen<'info> {
    #[account]
    pdas: &'info [arch_program::account::AccountInfo<'info>],
}

fn main() {} 