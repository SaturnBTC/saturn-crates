use arch_program::account::AccountInfo;
use arch_program::program_error::ProgramError;
use borsh::{BorshDeserialize, BorshSerialize};
use std::io::Cursor;

/// Classical Borsh serialisation codec.
pub struct BorshCodec;

impl BorshCodec {
    /// Deserialises the shard held in `account` using Borsh.
    pub fn load<S>(account: &AccountInfo<'_>) -> Result<S, ProgramError>
    where
        S: BorshDeserialize + BorshSerialize,
    {
        let data = account.try_borrow_data()?;
        S::try_from_slice(&data).map_err(|_| ProgramError::InvalidAccountData)
    }

    /// Serialises `shard` back into `account.data` using Borsh.
    pub fn store<S>(account: &AccountInfo<'_>, shard: &S) -> Result<(), ProgramError>
    where
        S: BorshSerialize,
    {
        let mut data = account.try_borrow_mut_data()?;
        // Write directly into the account's data buffer via an in-place cursor.
        let mut cursor = {
            let slice: &mut [u8] = &mut *data;
            Cursor::new(slice)
        };
        shard
            .serialize(&mut cursor)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        Ok(())
    }
}

/// Anchor-style Borsh account wrapper mirroring Anchor's `Account<'info, T>`.
#[allow(clippy::module_name_repetitions)]
pub struct Account<'a, T>
where
    T: BorshSerialize + BorshDeserialize,
{
    account: &'a AccountInfo<'a>,
    data: T,
}

impl<'a, T> Account<'a, T>
where
    T: BorshSerialize + BorshDeserialize,
{
    pub fn load(account: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        let data = BorshCodec::load::<T>(account)?;
        Ok(Self { account, data })
    }

    /// Returns a reference to the underlying `AccountInfo` object.
    pub fn info(&self) -> &'a AccountInfo<'a> {
        self.account
    }

    /// Convenience helper that clones the underlying `AccountInfo`.
    pub fn clone_account(&self) -> AccountInfo<'a> {
        self.account.clone()
    }
}

impl<'a, T> Account<'a, T>
where
    T: BorshSerialize + BorshDeserialize + Default,
{
    pub fn init(account: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        let default_val: T = Default::default();
        BorshCodec::store::<T>(account, &default_val)?;
        Ok(Self {
            account,
            data: default_val,
        })
    }
}

impl<'a, T> core::ops::Deref for Account<'a, T>
where
    T: BorshSerialize + BorshDeserialize,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<'a, T> core::ops::DerefMut for Account<'a, T>
where
    T: BorshSerialize + BorshDeserialize,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<'a, T> Drop for Account<'a, T>
where
    T: BorshSerialize + BorshDeserialize,
{
    fn drop(&mut self) {
        let _ = BorshCodec::store::<T>(self.account, &self.data);
    }
}

/// Alias so callers can write `BorshAccount<'info, T>` like in Anchor.
pub type BorshAccount<'a, T> = Account<'a, T>;

// Allow treating `BorshAccount` as an `AccountInfo` via `AsRef`.
impl<'a, T> AsRef<arch_program::account::AccountInfo<'a>> for Account<'a, T>
where
    T: BorshSerialize + BorshDeserialize,
{
    fn as_ref(&self) -> &arch_program::account::AccountInfo<'a> {
        self.account
    }
}
