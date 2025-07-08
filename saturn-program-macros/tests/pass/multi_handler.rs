use borsh::{BorshSerialize, BorshDeserialize};
use saturn_account_macros::Accounts;
use saturn_account_parser::codec::BorshAccount;
use saturn_program_macros::saturn_program;

#[derive(Accounts)]
struct DummyAccounts<'info> {
    #[account(signer)]
    user: BorshAccount<'info, u64>,
}

mod instruction {
    use super::*;
    #[derive(BorshSerialize, BorshDeserialize)]
    pub enum Instr {
        Foo(u8),
        Bar(u16),
    }
}

#[saturn_program(instruction = "crate::instruction::Instr")]
mod handlers {
    use super::*;

    pub fn foo<'info>(
        ctx: &mut Context<'info, DummyAccounts<'info>>, // should be rewritten
        _v: u8,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        let _ = ctx.program_id;
        Ok(())
    }

    pub fn bar<'info>(
        ctx: &mut Context<'info, DummyAccounts<'info>>, // should be rewritten
        _v: u16,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        let _ = ctx.program_id;
        Ok(())
    }
}

fn main() {} 