use saturn_account_macros::Accounts;
use saturn_account_parser as _;
use saturn_account_parser::codec::Account;

// Simulate a reusable constant determining the new space for account reallocation.
const EXTRA_SPACE: u64 = 32;

#[derive(Accounts)]
struct DynSpaceAccounts<'info> {
    #[account(signer, mut)]
    payer: Account<'info, u64>,
    #[account(mut, realloc, payer = payer, space = EXTRA_SPACE)]
    data: Account<'info, u64>,
}

fn main() {} 