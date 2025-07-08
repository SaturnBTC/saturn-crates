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

#[saturn_program(
    instruction = "crate::instruction::Instr",
    btc_tx_cfg(max_inputs_to_sign = 3, max_modified_accounts = 6, rune_capacity = 8)
)]
mod handlers {
    use super::*;

    pub fn my_handler<'info>(
        ctx: &mut Context<'info, DummyAccounts<'info>>, // macro should rewrite this path
        _params: u8,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        // Access BTC builder to ensure it exists when rune_capacity is used
        let _ = &ctx.btc_tx;
        Ok(())
    }
}

fn main() {} 