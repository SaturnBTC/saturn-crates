use saturn_error::saturn_error;

#[saturn_error(offset = 100)]
pub enum ErrorCode {
    #[error("Required UTXO matching the predicate was not found")]
    MissingRequiredUtxo,
    #[error("There are leftover UTXOs that were not consumed by the parser")]
    UnexpectedExtraUtxos,
    #[error("UTXO value (satoshis) did not match the expected value")]
    InvalidUtxoValue,
    #[error("UTXO runes presence (none/some) did not match expectation")]
    InvalidRunesPresence,
    #[error("Required rune id was not found in the UTXO")]
    InvalidRuneId,
    #[error("Rune amount in UTXO did not match expectation")]
    InvalidRuneAmount,
}
