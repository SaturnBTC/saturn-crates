use borsh::{BorshDeserialize, BorshSerialize};
use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;
use saturn_program_macros::saturn_program;
use saturn_account_parser::Context;

#[derive(Accounts)]
struct DummyAccounts<'info> {
    #[account(signer)]
    caller: Account<'info, u64>,
}

mod instruction {
    use super::*;
    #[derive(BorshSerialize, BorshDeserialize)]
    pub enum Instr {
        FooHandler(u8),
    }
}

#[saturn_program]
mod handlers {
    use super::*;
    pub fn foo_handler<'info>(
        _ctx: Context<'info, DummyAccounts<'info>>,
        _params: u8,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        Ok(())
    }

    pub fn fooHandler<'info>(
        _ctx: Context<'info, DummyAccounts<'info>>,
        _params: u8,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        Ok(())
    }
}

fn main() {} 