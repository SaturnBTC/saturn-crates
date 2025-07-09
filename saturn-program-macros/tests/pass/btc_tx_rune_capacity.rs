use borsh::{BorshDeserialize, BorshSerialize};
use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;
use saturn_program_macros::declare_id;
use saturn_program_macros::saturn_program;

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

#[saturn_program(btc_tx_cfg(max_inputs_to_sign = 3, max_modified_accounts = 6, rune_capacity = 8))]
mod handlers {
    use super::*;

    pub fn my_handler<'info>(
        ctx: Context<'info, DummyAccounts<'info>>, // macro should rewrite this path
        _params: u8,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        // Access BTC builder to ensure it exists when rune_capacity is used
        let _ = &ctx.btc_tx;
        Ok(())
    }
}

fn main() {}
