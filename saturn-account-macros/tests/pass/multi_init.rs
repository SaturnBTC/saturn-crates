use saturn_account_macros::Accounts;
use saturn_account_parser::codec::BorshAccount;

#[derive(Accounts)]
struct MultiInit<'info> {
    #[account(signer)]
    alice: arch_program::account::AccountInfo<'info>,
    #[account(signer)]
    bob: arch_program::account::AccountInfo<'info>,

    #[account(init, payer = alice, program_id = arch_program::pubkey::Pubkey::default())]
    pool: BorshAccount<'info, u64>,

    #[account(init, payer = bob, program_id = arch_program::pubkey::Pubkey::default())]
    vault: BorshAccount<'info, u64>,
}

fn main() {} 