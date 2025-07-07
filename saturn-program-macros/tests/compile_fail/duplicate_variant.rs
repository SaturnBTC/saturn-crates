use borsh::{BorshDeserialize, BorshSerialize};
use saturn_account_macros::Accounts;
use saturn_account_parser::codec::BorshAccount;
use saturn_account_parser::Context;
use saturn_program_macros::saturn_program;

#[derive(Accounts)]
struct DummyAccounts<'info> {
    #[account(signer)]
    caller: BorshAccount<'info, u64>,
}

mod instruction {
    use super::*;
    #[derive(BorshSerialize, BorshDeserialize)]
    pub enum Instr {
        FooHandler(u8),
    }
}

#[saturn_program(instruction = "crate::instruction::Instr")]
mod handlers {
    use super::*;
    pub fn foo_handler<'info>(
        _ctx: &mut Context<'_, '_, '_, 'info, DummyAccounts<'info>>,
        _params: u8,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        Ok(())
    }

    pub fn fooHandler<'info>(
        _ctx: &mut Context<'_, '_, '_, 'info, DummyAccounts<'info>>,
        _params: u8,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        Ok(())
    }
}

fn main() {} 