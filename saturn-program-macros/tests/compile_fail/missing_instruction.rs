use saturn_program_macros::saturn_program;

#[saturn_program(bitcoin_transaction = true, btc_tx_cfg(max_inputs_to_sign = 1, max_modified_accounts = 1))]
mod handlers {}

fn main() {} 