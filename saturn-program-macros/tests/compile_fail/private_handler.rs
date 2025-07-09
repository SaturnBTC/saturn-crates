use borsh::{BorshDeserialize, BorshSerialize};
use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;
use saturn_program_macros::saturn_program;
use saturn_account_parser::Context;

#[derive(Accounts)]
struct Dummy<'info> {
    #[account(signer)]
    acc: Account<'info, u64>,
}

mod instruction {
    use super::*;
    #[derive(BorshSerialize, BorshDeserialize)]
    pub enum Instr {
        Foo(u8),
    }
}

#[saturn_program]
mod handlers {
    use super::*;
    // Missing `pub` visibility – should trigger compile error
    fn foo<'info>(
        _ctx: Context<'info, Dummy<'info>>,
        _p: u8,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        Ok(())
    }
}

fn main() {} 