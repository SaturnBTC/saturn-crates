use arch_program::account::AccountInfo;
use saturn_account_macros::Accounts;
use saturn_account_parser as _;
use saturn_account_parser::codec::Account;

#[derive(Accounts)]
struct InitIfNeededAccs<'info> {
    #[account(signer, mut)]
    payer: AccountInfo<'info>,
    #[account(mut, init_if_needed, payer = payer, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
    maybe_new: Account<'info, u64>,
    #[account(len = 1)]
    sys: Vec<AccountInfo<'info>>,
}

fn main() {} 