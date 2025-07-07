use saturn_account_macros::Accounts;
use saturn_account_parser::codec::BorshAccount;

#[derive(Accounts)]
struct InitWithoutProgramID<'info> {
    #[account(init, payer = payer)]
    data: BorshAccount<'info, u64>,
    #[account(signer)]
    payer: arch_program::account::AccountInfo<'info>,
}

fn main() {} 