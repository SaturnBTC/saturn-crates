use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;
use arch_program::pubkey::Pubkey;

#[derive(Accounts)]
struct SeedsNoProgram<'info> {
    #[account(seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
    pda: Account<'info, u64>,
}

fn main() {} 