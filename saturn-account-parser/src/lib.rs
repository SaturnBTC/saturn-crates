use arch_program::{account::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

use crate::error::ErrorCode;

pub mod codec;
pub use codec::{Account, AccountLoader};
pub mod error;
pub mod tx_builder;
pub use tx_builder::TxBuilderWrapper;

/// Anchor-style instruction context that bundles the executing program id, a
/// typed view over the accounts (implements [`Accounts`]), and any extra
/// accounts that were passed but not part of the typed struct.
///
/// It is generic over the `Accounts` implementation to keep the same ergonomics
/// as Anchor while staying flexible for Saturn programs.
pub struct Context<'info, T: Accounts<'info>, TxBuilder = ()> {
    /// Public key of the program that is currently executing.
    pub program_id: &'info Pubkey,

    /// A **typed view** over the instruction's accounts (the struct that
    /// derives `Accounts`).
    pub accounts: &'info mut T,

    /// Any extra accounts that were supplied but not listed in the `Accounts`
    /// struct.
    pub remaining_accounts: &'info [AccountInfo<'info>],

    /// Optional Bitcoin transaction builder available when the program opts-in
    /// via the `bitcoin_transaction` attribute flag.
    pub btc_tx: TxBuilder,
}

// Convenience constructors
impl<'info, T: Accounts<'info>> Context<'info, T> {
    /// Same fields as before – no Bitcoin builder.
    pub fn new_simple(
        program_id: &'info Pubkey,
        accounts: &'info mut T,
        remaining_accounts: &'info [AccountInfo<'info>],
    ) -> Self {
        Self {
            program_id,
            accounts,
            remaining_accounts,
            btc_tx: (),
        }
    }
}

impl<'info, T: Accounts<'info>, TxBuilder> Context<'info, T, TxBuilder> {
    /// Constructor used by the macro when a Bitcoin transaction builder is provided.
    pub fn new_with_btc_tx(
        program_id: &'info Pubkey,
        accounts: &'info mut T,
        remaining_accounts: &'info [AccountInfo<'info>],
        btc_tx: TxBuilder,
    ) -> Self {
        Self {
            program_id,
            accounts,
            remaining_accounts,
            btc_tx,
        }
    }
}

pub trait Accounts<'a>: Sized {
    fn try_accounts(accounts: &'a [AccountInfo<'a>]) -> Result<Self, ProgramError>;
}

/// Retrieves a reference to a specific account from a slice. Certain checks
/// can be executed by providing expected values for `is_signer`,
/// `is_writable` and `key`.
pub fn get_account<'a>(
    accounts: &'a [AccountInfo<'a>],
    index: usize,
    is_signer: Option<bool>,
    is_writable: Option<bool>,
    key: Option<Pubkey>,
) -> Result<&'a AccountInfo<'a>, ProgramError> {
    // msg!("get_account: {}", index);

    let acc = accounts
        .get(index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    if let Some(acc_is_signer) = is_signer {
        if acc.is_signer != acc_is_signer {
            return Err(ProgramError::Custom(
                ErrorCode::IncorrectIsSignerFlag.into(),
            ));
        }
    }

    if let Some(acc_is_writable) = is_writable {
        if acc.is_writable != acc_is_writable {
            return Err(ProgramError::Custom(
                ErrorCode::IncorrectIsWritableFlag.into(),
            ));
        }
    }

    if let Some(key) = key {
        if acc.key != &key {
            return Err(ProgramError::InvalidAccountOwner);
        }
    }

    Ok(acc)
}

pub fn get_pda_account<'a>(
    accounts: &'a [AccountInfo<'a>],
    index: usize,
    is_signer: Option<bool>,
    is_writable: Option<bool>,
    seeds: &[&[u8]],
    program_id: &Pubkey,
) -> Result<&'a AccountInfo<'a>, ProgramError> {
    // First, retrieve the desired account while validating signer / writable flags
    let acc = get_account(accounts, index, is_signer, is_writable, None)?;

    // Derive the expected PDA address from the provided seeds
    let (expected_key, _bump) = Pubkey::find_program_address(seeds, program_id);

    if acc.key != &expected_key {
        return Err(ProgramError::Custom(ErrorCode::InvalidPda.into()));
    }

    Ok(acc)
}

pub fn get_indexed_pda_account<'a>(
    accounts: &'a [AccountInfo<'a>],
    index: usize,
    is_signer: Option<bool>,
    is_writable: Option<bool>,
    base_seeds: &[&[u8]],
    idx: u16,
    program_id: &Pubkey,
) -> Result<&'a AccountInfo<'a>, ProgramError> {
    // Retrieve the account reference first (signer / writable flags checked)
    let acc = get_account(accounts, index, is_signer, is_writable, None)?;

    // Encode the dynamic index as LE bytes (2-byte little-endian)
    let idx_bytes = idx.to_le_bytes();

    // -----------------------------------------------------------------------------------------------------------------
    // Build the full seed slice **without** performing a heap allocation.
    // We do this by copying the provided `base_seeds` into a fixed-size stack array and appending `idx_bytes`.
    // The Solana (and Arch) runtime caps the number of seeds for PDA derivation, which is exposed via
    // `arch_program::pubkey::MAX_SEEDS`. We enforce that limit and use it as the array length.
    // -----------------------------------------------------------------------------------------------------------------
    const MAX_SEEDS: usize = arch_program::pubkey::MAX_SEEDS;

    // Return an error if the caller provides more seeds than allowed once we append the index.
    let total_seeds = base_seeds.len() + 1; // +1 for `idx_bytes`
    if total_seeds > MAX_SEEDS {
        return Err(ProgramError::InvalidSeeds);
    }

    // A helper const for an empty slice so we can easily initialise the array.
    const EMPTY_SLICE: &[u8] = &[];

    // Fixed-size stack array that will temporarily hold all seed references. Initialised with empty slices.
    let mut seed_buf: [&[u8]; MAX_SEEDS] = [EMPTY_SLICE; MAX_SEEDS];

    // Copy the user's seeds into the buffer.
    seed_buf[..base_seeds.len()].copy_from_slice(base_seeds);
    // Append the LE-encoded `idx`.
    seed_buf[base_seeds.len()] = &idx_bytes;

    // Slice the buffer down to the actual number of seeds we filled.
    let seeds_slice = &seed_buf[..total_seeds];

    // Derive PDA from seeds and compare.
    let (expected_key, _bump) = Pubkey::find_program_address(seeds_slice, program_id);
    if acc.key != &expected_key {
        return Err(ProgramError::Custom(ErrorCode::InvalidPda.into()));
    }

    Ok(acc)
}
