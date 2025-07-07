use saturn_account_macros::Accounts;
use saturn_account_parser::codec::BorshAccount;

#[derive(Accounts)]
struct ProgramIDNoSeeds<'info> {
    #[account(program_id = arch_program::pubkey::Pubkey::default())]
    pda: BorshAccount<'info, u64>,
}

fn main() {} 