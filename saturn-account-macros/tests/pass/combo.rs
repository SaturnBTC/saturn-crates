use arch_program::account::AccountInfo;
use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;

#[derive(Accounts)]
struct Combo<'info> {
    // classic signer & writable
    #[account(signer, mut)]
    user: AccountInfo<'info>,

    // init account with payer, seeds, program owner and explicit space
    #[account(mut, init, payer = user, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default(), space = 16)]
    new_pda: Account<'info, u64>,
}

fn main() {} 