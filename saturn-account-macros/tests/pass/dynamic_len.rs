use arch_program::account::AccountInfo;
use saturn_account_macros::Accounts;
use saturn_account_parser as _;

// A compile-time constant that will be used as the `len =` expression. This mimics
// the common Anchor pattern where the slice length is defined elsewhere instead
// of being an inline literal.
const NUM_PDAS: usize = 4;

#[derive(Accounts)]
struct DynLenAccounts<'info> {
    #[account(len = NUM_PDAS)]
    pdas: Vec<AccountInfo<'info>>, // fixed-length slice with dynamic constant
}

fn main() {}