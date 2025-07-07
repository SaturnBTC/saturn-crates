use saturn_account_macros::Accounts;

#[derive(Accounts)]
struct BumpArray<'info> {
    #[account(seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
    pda: saturn_account_parser::codec::BorshAccount<'info, u64>,
    #[account(bump, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
    bump: [u8; 1],
}

fn main() {} 