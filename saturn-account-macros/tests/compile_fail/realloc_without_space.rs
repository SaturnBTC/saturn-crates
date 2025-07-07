use saturn_account_macros::Accounts;
use saturn_account_parser::codec::BorshAccount;

#[derive(Accounts)]
struct ReallocNoSpace<'info> {
    #[account(signer)]
    payer: BorshAccount<'info, u64>,
    #[account(realloc, payer = payer)]
    data: BorshAccount<'info, u64>,
}

fn main() {} 