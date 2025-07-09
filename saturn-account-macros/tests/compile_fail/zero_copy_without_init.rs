use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;

#[derive(BorshSerialize, BorshDeserialize, Copy, Clone)]
#[repr(C)]
pub struct Data {
    pub value: u64,
}

#[derive(Accounts)]
struct ZeroCopyNoInit<'info> {
    #[account(zero_copy, of = Data)]
    // Wrong on purpose. Account is not a AccountLoader.
    data: Account<'info, Data>,
}

fn main() {}
