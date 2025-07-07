use saturn_account_macros::Accounts;

#[derive(Accounts)]
struct ContradictoryFlags<'info> {
    #[account(zero_copy, init, payer = payer, program_id = arch_program::pubkey::Pubkey::default())]
    acc: arch_program::account::AccountInfo<'info>,
    #[account(signer)]
    payer: arch_program::account::AccountInfo<'info>,
}

fn main() {} 