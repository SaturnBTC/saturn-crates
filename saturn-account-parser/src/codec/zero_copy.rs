use arch_program::account::AccountInfo;
use arch_program::program_error::ProgramError;
use bytemuck::{Pod, Zeroable};
use core::mem::{align_of, size_of};
use std::cell::{Ref, RefMut};

/// Zero-copy codec: re-interprets the account data buffer as a `T` without
/// performing any heap allocations or copies.
pub struct ZeroCopyCodec;

impl ZeroCopyCodec {
    /// Copies the shard out of the account into an owned value.
    pub fn load_copy<S>(account: &AccountInfo<'_>) -> Result<S, ProgramError>
    where
        S: Pod + Zeroable + Clone,
    {
        let data = account.try_borrow_data()?;
        if data.len() < size_of::<S>() {
            return Err(ProgramError::InvalidAccountData);
        }
        // SAFETY: bounds checked above + `S` is Pod.
        let bytes = &data[..size_of::<S>()];
        Ok(*bytemuck::from_bytes(bytes))
    }

    /// Stores an owned value back into the account's data buffer.
    pub fn store_copy<S>(account: &AccountInfo<'_>, shard: &S) -> Result<(), ProgramError>
    where
        S: Pod + Zeroable,
    {
        let mut data = account.try_borrow_mut_data()?;
        if data.len() < size_of::<S>() {
            return Err(ProgramError::InvalidAccountData);
        }
        // SAFETY: same as in `load_copy`.
        let bytes = &mut data[..size_of::<S>()];
        bytes.copy_from_slice(bytemuck::bytes_of(shard));
        Ok(())
    }

    /// Returns a mutable reference into the account's data buffer interpreted as `S`.
    pub fn load_mut_ref<'a, S>(account: &'a AccountInfo<'a>) -> Result<RefMut<'a, S>, ProgramError>
    where
        S: Pod + Zeroable + 'static,
    {
        let mut data = account.try_borrow_mut_data()?;

        if data.len() < size_of::<S>() {
            return Err(ProgramError::InvalidAccountData);
        }

        // Ensure proper alignment.
        if (data.as_ptr() as usize) % align_of::<S>() != 0 {
            return Err(ProgramError::InvalidAccountData);
        }

        // SAFETY: alignment + size checks above guarantee safe reinterpretation.
        let ref_mut = RefMut::map(data, |slice| {
            let slice = &mut slice[..size_of::<S>()];
            unsafe { &mut *(slice.as_mut_ptr() as *mut S) }
        });

        Ok(ref_mut)
    }

    /// Returns an immutable reference into the account's data buffer.
    pub fn load_ref<'a, S>(account: &'a AccountInfo<'a>) -> Result<Ref<'a, S>, ProgramError>
    where
        S: Pod + Zeroable + 'static,
    {
        let data = account.try_borrow_data()?;

        if data.len() < size_of::<S>() {
            return Err(ProgramError::InvalidAccountData);
        }

        // Ensure proper alignment.
        if (data.as_ptr() as usize) % align_of::<S>() != 0 {
            return Err(ProgramError::InvalidAccountData);
        }

        // SAFETY: same guarantees as `load_mut_ref`, but immutable.
        let ref_imm = Ref::map(data, |slice| {
            let slice = &slice[..size_of::<S>()];
            unsafe { &*(slice.as_ptr() as *const S) }
        });
        Ok(ref_imm)
    }

    /// Legacy helper that leaks the underlying `RefMut`.
    #[deprecated(note = "Use `load_mut_ref` to avoid runtime-borrow poisoning")]
    #[allow(clippy::needless_doctest_main)]
    pub fn load_mut<'a, S>(account: &'a AccountInfo<'a>) -> Result<&'a mut S, ProgramError>
    where
        S: Pod + Zeroable + 'static,
    {
        let mut ref_mut = Self::load_mut_ref::<S>(account)?;
        let ptr: *mut S = &mut *ref_mut as *mut S;
        std::mem::forget(ref_mut);
        // SAFETY: pointer remains valid for lifetime of the program invocation.
        Ok(unsafe { &mut *ptr })
    }
}

// -----------------------------------------------------------------------------
// Anchor-style zero-copy account loader
// -----------------------------------------------------------------------------

#[allow(clippy::module_name_repetitions)]
pub struct AccountLoader<'a, T>
where
    T: Pod + Zeroable + 'static,
{
    account: &'a AccountInfo<'a>,
    _phantom: core::marker::PhantomData<T>,
}

impl<'a, T> AccountLoader<'a, T>
where
    T: Pod + Zeroable + 'static,
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

    /// Direct access to the wrapped `AccountInfo`.
    pub fn info(&self) -> &'a AccountInfo<'a> {
        self.account
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
        let size = core::mem::size_of::<T>();
        account_info.realloc(size, true)?;
        Ok(())
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

        // (Re)allocate the account to the exact size and make it rent-exempt.
        account_info.realloc(size, true)?;

        // Zero-fill to ensure a well-defined starting state.
        {
            let mut data = account_info.try_borrow_mut_data()?;
            if data.len() < size {
                return Err(ProgramError::InvalidAccountData);
            }
            for byte in &mut data[..size] {
                *byte = 0;
            }
        }

        // Return a mutable zero-copy reference.
        ZeroCopyCodec::load_mut_ref::<T>(account_info)
    }
}

/// Short alias mirroring `BorshAccount`.
pub type ZeroCopyAccount<'a, T> = AccountLoader<'a, T>;
