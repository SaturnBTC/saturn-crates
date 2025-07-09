use borsh::{BorshDeserialize, BorshSerialize};
use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;
use saturn_program_macros::saturn_program;
use saturn_program_macros::declare_id;

declare_id!("8YE2m8RGmFjyWkHfMV6aA1eeaoAj8ZqEXnoY6v1WKEwd");

#[derive(Accounts)]
struct DummyAccounts<'info> {
    #[account(signer)]
    caller: Account<'info, u64>,
}

mod instruction {
    use super::*;
    #[derive(BorshSerialize, BorshDeserialize)]
    pub enum Instr {
        MyHandler(u8),
    }
}

#[saturn_program]
mod handlers {
    use super::*;
    pub fn my_handler<'info>(
        ctx: Context<'info, DummyAccounts<'info>>,
        _params: u8,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        let _ = ctx.program_id; // access something to avoid warnings
        Ok(())
    }
}

fn main() {}
