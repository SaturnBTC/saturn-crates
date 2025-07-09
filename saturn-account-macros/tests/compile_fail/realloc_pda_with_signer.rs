use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;

#[derive(Accounts)]
struct ReallocPdaSigner<'info> {
    #[account(mut, signer)]
    user: Account<'info, u64>,
    // PDA account incorrectly marked signer while also realloc
    #[account(mut, realloc, signer, seeds = &[b"seed"], program_id = Pubkey::default(), space = 64)]
    pda_data: Account<'info, u64>,
}

fn main() {}
