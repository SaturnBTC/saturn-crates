use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;
use arch_program::pubkey::Pubkey;

#[derive(Accounts)]
struct InitWithoutProgramID<'info> {
    #[account(mut, signer)]
    payer: arch_program::account::AccountInfo<'info>,
    #[account(init, mut, payer = payer, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
    data: Account<'info, u64>,
}

fn main() {} 