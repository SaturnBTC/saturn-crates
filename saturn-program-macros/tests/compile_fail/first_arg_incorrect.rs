use borsh::{BorshDeserialize, BorshSerialize};
use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;
use saturn_program_macros::saturn_program;

#[derive(Accounts)]
struct DummyAccounts<'info> {
    #[account(signer)]
    acc: Account<'info, u64>,
}

mod instruction {
    use super::*;
    #[derive(BorshSerialize, BorshDeserialize)]
    pub enum Instr {
        Bad(u8),
    }
}

#[saturn_program]
mod handlers {
    use super::*;
    // The first argument type is incorrect: should be Context
    pub fn bad<'info>(
        _ctx: DummyAccounts<'info>,
        _p: u8,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        Ok(())
    }
}

fn main() {}
