use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;

#[derive(Accounts)]
struct MultiInit<'info> {
    #[account(signer, mut)]
    alice: arch_program::account::AccountInfo<'info>,
    #[account(signer, mut)]
    bob: arch_program::account::AccountInfo<'info>,

    #[account(mut, init, payer = alice, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
    pool: Account<'info, u64>,

    #[account(mut, init, payer = bob, seeds = &[b"seed2"], program_id = arch_program::pubkey::Pubkey::default())]
    vault: Account<'info, u64>,
}

fn main() {} 