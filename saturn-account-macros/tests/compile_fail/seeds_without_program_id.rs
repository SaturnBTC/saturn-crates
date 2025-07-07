use saturn_account_macros::Accounts;
use saturn_account_parser::codec::BorshAccount;

#[derive(Accounts)]
struct SeedsNoProgram<'info> {
    #[account(seeds = &[b"seed"])]
    pda: BorshAccount<'info, u64>,
}

fn main() {} 