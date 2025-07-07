use saturn_account_macros::Accounts;
use saturn_account_parser::codec::BorshAccount;

#[derive(Accounts)]
struct ReallocPass<'info> {
    #[account(signer)]
    payer: BorshAccount<'info, u64>,
    #[account(realloc, payer = payer, space = 8)]
    data: BorshAccount<'info, u64>,
}

fn main() {}
