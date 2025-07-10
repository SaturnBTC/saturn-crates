use saturn_account_macros::Accounts;
use saturn_account_parser::Account;
use saturn_program_macros::saturn_program;
use saturn_program_macros::declare_id;

declare_id!("8YE2m8RGmFjyWkHfMV6aA1eeaoAj8ZqEXnoY6v1WKEwd");

#[derive(Accounts)]
struct DummyAccounts<'info> {
    #[account(signer, mut)]
    caller: Account<'info, u64>,
}

// This should fail because `core::fmt::Debug` does not implement
// `FixedCapacitySet<Item = arch_program::rune::RuneAmount>`.
#[saturn_program(btc_tx_cfg(
    max_inputs_to_sign = 1,
    max_modified_accounts = 1,
    rune_set = "core::fmt::Debug"
))]
mod handlers {
    use super::*;

    pub fn dummy<'info>(
        ctx: Context<'info, DummyAccounts<'info>>,
        _params: u8,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        Ok(())
    }
}

fn main() {}
