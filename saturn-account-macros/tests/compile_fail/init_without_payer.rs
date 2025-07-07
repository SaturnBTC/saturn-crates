use saturn_account_macros::Accounts;
use saturn_account_parser::codec::BorshAccount;

#[derive(Accounts)]
struct InitWithoutPayer<'info> {
    #[account(init, program_id = arch_program::pubkey::Pubkey::default())]
    data: BorshAccount<'info, u64>,
}

fn main() {} 