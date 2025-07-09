use saturn_account_macros::Accounts;

#[derive(bytemuck::Pod, bytemuck::Zeroable, Copy, Clone)]
#[repr(C)]
pub struct Data {
    pub value: u64,
}

#[derive(Accounts)]
struct ZeroCopyPlain<'info> {
    #[account(zero_copy, of = Data)]
    acc: arch_program::account::AccountInfo<'info>,
}

fn main() {} 