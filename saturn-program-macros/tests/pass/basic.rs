use arch_program::account::AccountInfo;
use borsh::{BorshDeserialize, BorshSerialize};
use saturn_account_macros::Accounts;
use saturn_account_parser::codec::BorshAccount;
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
        MyHandler(u8),
    }
}
use instruction::Instr;

#[saturn_program(instruction = "crate::instruction::Instr")]
mod handlers {
    use super::*;
    pub fn my_handler<'info>(
        ctx: &mut Context<'info, DummyAccounts<'info>>,
        _params: u8,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        let _ = ctx.program_id; // access something to avoid warnings
        Ok(())
    }
}

fn main() {}
