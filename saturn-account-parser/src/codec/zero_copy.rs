//! Zero-copy codec and account loader.
//!
//! Unlike the Borsh codec, zero-copy access reinterprets the raw account
//! buffer as a Plain-Old-Data (`Pod`) struct without performing any heap
//! allocation or copy.  This provides maximum throughput at the cost of
//! stricter type and alignment requirements.
//!
//! The primary entry points are [`ZeroCopyCodec`] for bare loading/storing and
//! [`AccountLoader`] for an Anchor-style wrapper that
//! tracks borrows at runtime.

use arch_program::account::AccountInfo;
use arch_program::program_error::ProgramError;
use bytemuck::{Pod, Zeroable};
use core::mem::{align_of, size_of};
use std::cell::{Ref, RefMut};

/// Length in bytes of the account discriminator that prefixes every
/// zero-copy account.
const DISCRIMINATOR_LEN: usize = 8;

/// Every zero-copy account type must provide an 8-byte discriminator that
/// uniquely identifies its layout on-chain.  It mirrors Anchor’s convention
/// and will be written as the first eight bytes of the account data.
pub trait Discriminator {
    /// The constant 8-byte discriminator.  Implementations are typically
    /// generated via the upcoming `#[derive(Discriminator)]` procedural macro.
    const DISCRIMINATOR: [u8; 8];
}

/// Zero-copy codec: re-interprets the account data buffer as a `T` without
/// performing any heap allocations or copies.
pub struct ZeroCopyCodec;

impl ZeroCopyCodec {
    /// Copies the shard out of the account into an owned value.
    pub fn load_copy<S>(account: &AccountInfo<'_>) -> Result<S, ProgramError>
    where
        S: Pod + Zeroable + Clone + Discriminator,
    {
        let data = account.try_borrow_data()?;
        if data.len() < DISCRIMINATOR_LEN + size_of::<S>() {
            return Err(ProgramError::InvalidAccountData);
        }
        // Verify discriminator matches.
        if &data[..DISCRIMINATOR_LEN] != &<S as Discriminator>::DISCRIMINATOR
        {
            return Err(ProgramError::InvalidAccountData);
        }

        // SAFETY: bounds checked above + `S` is Pod.
        let bytes = &data[DISCRIMINATOR_LEN..DISCRIMINATOR_LEN + size_of::<S>()];
        Ok(*bytemuck::from_bytes(bytes))
    }

    /// Stores an owned value back into the account's data buffer.
    pub fn store_copy<S>(account: &AccountInfo<'_>, shard: &S) -> Result<(), ProgramError>
    where
        S: Pod + Zeroable + Discriminator,
    {
        let mut data = account.try_borrow_mut_data()?;
        if data.len() < DISCRIMINATOR_LEN + size_of::<S>() {
            return Err(ProgramError::InvalidAccountData);
        }
        if &data[..DISCRIMINATOR_LEN] != &<S as Discriminator>::DISCRIMINATOR
        {
            return Err(ProgramError::InvalidAccountData);
        }
        // SAFETY: same as in `load_copy`.
        let bytes = &mut data[DISCRIMINATOR_LEN..DISCRIMINATOR_LEN + size_of::<S>()];
        bytes.copy_from_slice(bytemuck::bytes_of(shard));
        Ok(())
    }

    /// Returns a mutable reference into the account's data buffer interpreted as `S`.
    pub fn load_mut_ref<'a, S>(account: &'a AccountInfo<'a>) -> Result<RefMut<'a, S>, ProgramError>
    where
        S: Pod + Zeroable + Discriminator + 'static,
    {
        // Disallow mutable access when the account was not marked writable by the caller.
        if !account.is_writable {
            return Err(ProgramError::Custom(
                crate::error::ErrorCode::IncorrectIsWritableFlag.into(),
            ));
        }

        let data = account.try_borrow_mut_data()?;

        if data.len() < DISCRIMINATOR_LEN + size_of::<S>() {
            return Err(ProgramError::InvalidAccountData);
        }

        if &data[..DISCRIMINATOR_LEN] != &<S as Discriminator>::DISCRIMINATOR
        {
            return Err(ProgramError::InvalidAccountData);
        }

        // Ensure proper alignment (after discriminator offset).
        if (data[DISCRIMINATOR_LEN..].as_ptr() as usize) % align_of::<S>() != 0 {
            return Err(ProgramError::InvalidAccountData);
        }

        // SAFETY: alignment + size checks above guarantee safe reinterpretation.
        let ref_mut = RefMut::map(data, |slice| {
            let slice = &mut slice[DISCRIMINATOR_LEN..DISCRIMINATOR_LEN + size_of::<S>()];
            unsafe { &mut *(slice.as_mut_ptr() as *mut S) }
        });

        Ok(ref_mut)
    }

    /// Returns an immutable reference into the account's data buffer.
    pub fn load_ref<'a, S>(account: &'a AccountInfo<'a>) -> Result<Ref<'a, S>, ProgramError>
    where
        S: Pod + Zeroable + Discriminator + 'static,
    {
        let data = account.try_borrow_data()?;

        if data.len() < DISCRIMINATOR_LEN + size_of::<S>() {
            return Err(ProgramError::InvalidAccountData);
        }

        if &data[..DISCRIMINATOR_LEN] != &<S as Discriminator>::DISCRIMINATOR
        {
            return Err(ProgramError::InvalidAccountData);
        }

        // Ensure proper alignment.
        if (data[DISCRIMINATOR_LEN..].as_ptr() as usize) % align_of::<S>() != 0 {
            return Err(ProgramError::InvalidAccountData);
        }

        // SAFETY: same guarantees as `load_mut_ref`, but immutable.
        let ref_imm = Ref::map(data, |slice| {
            let slice = &slice[DISCRIMINATOR_LEN..DISCRIMINATOR_LEN + size_of::<S>()];
            unsafe { &*(slice.as_ptr() as *const S) }
        });
        Ok(ref_imm)
    }
}

// -----------------------------------------------------------------------------
// Anchor-style zero-copy account loader
// -----------------------------------------------------------------------------

#[allow(clippy::module_name_repetitions)]
pub struct AccountLoader<'a, T>
where
    T: Pod + Zeroable + Discriminator + 'static,
{
    account: &'a AccountInfo<'a>,
    _phantom: core::marker::PhantomData<T>,
}

// ---------------- Generic helper methods ----------------
impl<'a, T> AccountLoader<'a, T>
where
    T: Pod + Zeroable + Discriminator + 'static,
{
    /// Creates a new loader wrapping the given account.
    pub fn new(account: &'a AccountInfo<'a>) -> Self {
        Self {
            account,
            _phantom: core::marker::PhantomData,
        }
    }

    /// Immutable borrow of the underlying zero-copy struct.
    pub fn load(&self) -> Result<Ref<'a, T>, ProgramError> {
        ZeroCopyCodec::load_ref::<T>(self.account)
    }

    /// Direct access to the wrapped `AccountInfo`.
    pub fn info(&self) -> &'a AccountInfo<'a> {
        self.account
    }

    /// Mutable borrow of the underlying zero-copy struct.
    pub fn load_mut(&self) -> Result<RefMut<'a, T>, ProgramError> {
        ZeroCopyCodec::load_mut_ref::<T>(self.account)
    }

    /// Initialises a brand-new zero-copy account (resize + zero-fill) and returns a mutable reference to it.
    pub fn load_init(&self) -> Result<RefMut<'a, T>, ProgramError>
    where
        T: Default,
    {
        Self::init_zero_copy_account(self.account)
    }

    /// Allocates and zero-initialises an account for zero-copy usage and
    /// returns a mutable reference to the freshly created struct.
    fn init_zero_copy_account<'info>(
        account_info: &'info AccountInfo<'info>,
    ) -> Result<RefMut<'info, T>, ProgramError>
    where
        T: Default,
    {
        let size = core::mem::size_of::<T>();

        let total_size = size + DISCRIMINATOR_LEN;

        // (Re)allocate the account to the exact size and make it rent-exempt.
        account_info.realloc(total_size, true)?;

        // Write discriminator + zero-fill remainder.
        {
            let mut data = account_info.try_borrow_mut_data()?;
            if data.len() < total_size {
                return Err(ProgramError::InvalidAccountData);
            }

            // write discriminator
            data[..DISCRIMINATOR_LEN]
                .copy_from_slice(&<T as Discriminator>::DISCRIMINATOR);

            // zero-fill struct bytes
            for byte in &mut data[DISCRIMINATOR_LEN..total_size] {
                *byte = 0;
            }
        }

        // Return a mutable zero-copy reference.
        ZeroCopyCodec::load_mut_ref::<T>(account_info)
    }

    /// Adjusts the account's data buffer to exactly `size_of::<T>()` bytes and
    /// keeps any excess lamports in place so the caller does not need to fund
    /// rent again.
    pub fn resize_account<'info>(
        account_info: &'info AccountInfo<'info>,
    ) -> Result<(), ProgramError>
    where
        T: Sized + Default,
    {
        let size = core::mem::size_of::<T>() + DISCRIMINATOR_LEN;
        account_info.realloc(size, true)?;
        Ok(())
    }
}

// Allow treating `AccountLoader` (any mutability) as an `AccountInfo` via `AsRef`.
impl<'a, T> AsRef<arch_program::account::AccountInfo<'a>> for AccountLoader<'a, T>
where
    T: Pod + Zeroable + Discriminator + 'static,
{
    fn as_ref(&self) -> &arch_program::account::AccountInfo<'a> {
        self.account
    }
}
