use borsh::{BorshSerialize, BorshDeserialize};
use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;
use saturn_account_parser::Context;
use saturn_program_macros::saturn_program;
use saturn_program_macros::declare_id;

declare_id!("8YE2m8RGmFjyWkHfMV6aA1eeaoAj8ZqEXnoY6v1WKEwd");

#[derive(Accounts)]
struct DummyAccounts<'info> {
    #[account(signer)]
    user: Account<'info, u64>,
}

mod instruction {
    use super::*;
    #[derive(BorshSerialize, BorshDeserialize)]
    pub enum Instr {
        Foo(u8),
        Bar(u16),
    }
}

#[saturn_program]
mod handlers {
    use super::*;

    pub fn foo<'info>(
        ctx: Context<'info, DummyAccounts<'info>>, // should be rewritten
        _v: u8,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        let _ = ctx.program_id;
        Ok(())
    }

    pub fn bar<'info>(
        ctx: Context<'info, DummyAccounts<'info>>, // should be rewritten
        _v: u16,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        let _ = ctx.program_id;
        Ok(())
    }
}

fn main() {} 