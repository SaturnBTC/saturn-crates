use arch_program::program_error::ProgramError;

pub enum EscrowError {
    InvalidUtxoContainingRunes,
    NoAccountUtxoFound,
    InvalidUtxoCount,
    InvalidAccountCount,
}

impl From<EscrowError> for ProgramError {
    fn from(error: EscrowError) -> Self {
        match error {
            EscrowError::InvalidUtxoContainingRunes => ProgramError::Custom(0),
            EscrowError::NoAccountUtxoFound => ProgramError::Custom(1),
            EscrowError::InvalidUtxoCount => ProgramError::Custom(2),
            EscrowError::InvalidAccountCount => ProgramError::Custom(3),
        }
    }
}
