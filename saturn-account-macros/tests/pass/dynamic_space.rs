use saturn_account_macros::Accounts;
use saturn_account_parser as _;
use saturn_account_parser::codec::BorshAccount;

// Simulate a reusable constant determining the new space for account reallocation.
const EXTRA_SPACE: u64 = 32;

#[derive(Accounts)]
struct DynSpaceAccounts<'info> {
    #[account(signer)]
    payer: BorshAccount<'info, u64>,
    #[account(realloc, payer = payer, space = EXTRA_SPACE)]
    data: BorshAccount<'info, u64>,
}

fn main() {} 