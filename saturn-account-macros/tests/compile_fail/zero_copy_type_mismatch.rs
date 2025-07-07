use saturn_account_macros::Accounts;
use saturn_account_parser::codec::BorshAccount;

#[derive(Accounts)]
struct ZeroCopyTypeMismatch<'info> {
    #[account(zero_copy, init, payer = payer, program_id = arch_program::pubkey::Pubkey::default())]
    // Intentional: using BorshAccount instead of ZeroCopyAccount
    data: BorshAccount<'info, u64>,
    #[account(signer)]
    payer: arch_program::account::AccountInfo<'info>,
}

fn main() {} 