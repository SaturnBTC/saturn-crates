use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;

#[derive(Accounts)]
struct ZeroCopyTypeMismatch<'info> {
    #[account(mut, signer)]
    payer: arch_program::account::AccountInfo<'info>,
    #[account(zero_copy, init, mut, payer = payer, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
    // Intentional: using Account instead of AccountLoader
    data: Account<'info, u64>,
}

fn main() {}
