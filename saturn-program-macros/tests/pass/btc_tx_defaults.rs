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
    btc_tx_cfg(max_inputs_to_sign = 4, max_modified_accounts = 4)
)]
mod handlers {
    use super::*;

    pub fn my_handler<'info>(
        ctx: &mut Context<'info, DummyAccounts<'info>>,
        _params: u8,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        // Access BTC builder to ensure it is available when `btc_tx_cfg` is present
        let _ = &ctx.btc_tx;
        let _ = ctx.program_id;
        Ok(())
    }
}

fn main() {} 