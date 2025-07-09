use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;

#[derive(Accounts)]
struct ProgramIDNoSeeds<'info> {
    #[account(program_id = arch_program::pubkey::Pubkey::default())]
    pda: Account<'info, u64>,
}

fn main() {} 