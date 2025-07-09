use saturn_account_macros::Accounts;

#[derive(bytemuck::Pod, bytemuck::Zeroable, Copy, Clone)]
#[repr(C)]
pub struct Data {
    pub value: u64,
}

#[derive(Accounts)]
struct ContradictoryFlags<'info> {
    #[account(mut, signer)]
    payer: arch_program::account::AccountInfo<'info>,
    #[account(zero_copy, of = Data, init, mut, signer, payer = payer, program_id = arch_program::pubkey::Pubkey::default())]
    acc: arch_program::account::AccountInfo<'info>,
}

fn main() {}
