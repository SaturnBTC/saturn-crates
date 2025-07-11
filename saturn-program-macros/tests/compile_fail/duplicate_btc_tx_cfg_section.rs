use saturn_program_macros::saturn_program;

mod instruction {
    #[derive(borsh::BorshSerialize, borsh::BorshDeserialize)]
    pub enum Instr {
        Dummy,
    }
}

#[saturn_program(
    btc_tx_cfg(max_inputs_to_sign = 1, max_modified_accounts = 1),
    btc_tx_cfg(max_inputs_to_sign = 2, max_modified_accounts = 2)
)]
mod handlers {}

fn main() {} 