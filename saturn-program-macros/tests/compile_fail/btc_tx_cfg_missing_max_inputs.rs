use saturn_program_macros::saturn_program;

mod instruction {
    #[derive(borsh::BorshSerialize, borsh::BorshDeserialize)]
    pub enum Instr {
        Dummy,
    }
}

#[saturn_program(
    btc_tx_cfg(max_modified_accounts = 2) // missing max_inputs_to_sign
)]
mod handlers {}

fn main() {} 