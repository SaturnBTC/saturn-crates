use saturn_program_macros::saturn_program;

mod instruction {
    #[derive(borsh::BorshSerialize, borsh::BorshDeserialize)]
    pub enum Instr {
        Dummy,
    }
}

#[saturn_program(
    btc_tx_cfg(foo = 1, max_inputs_to_sign = 1, max_modified_accounts = 1)
)]
mod handlers {}

fn main() {} 