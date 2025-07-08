use arch_program::program_error::ProgramError;
use bytemuck::{Pod, Zeroable};
use saturn_account_parser::codec::zero_copy::AccountLoader;

/// Lightweight handle around an [`AccountLoader`].
///
/// The handle does **not** keep the underlying `Ref` / `RefMut` alive; instead
/// it provides scoped helper methods that borrow the zero-copy shard on demand
/// and release the borrow right after the supplied closure returns.  This keeps
/// the runtime borrow flag active for the shortest possible time and avoids
/// long-lived mutable references during the instruction.
///
/// The generic `S` represents the concrete zero-copy struct that implements
/// [`StateShard`].  We purposely omit that trait bound here to keep the handle
/// usable in lower-level contexts – the bound will be required when we
/// implement `StateShard` for `ShardHandle` itself in a later step.
#[derive(Copy, Clone)]
pub struct ShardHandle<'info, S>
where
    S: Pod + Zeroable + 'static,
{
    loader: &'info AccountLoader<'info, S>,
}

impl<'info, S> ShardHandle<'info, S>
where
    S: Pod + Zeroable + 'static,
{
    /// Wrap an existing [`AccountLoader`].
    #[inline]
    pub const fn new(loader: &'info AccountLoader<'info, S>) -> Self {
        Self { loader }
    }

    /// Access the wrapped loader.
    #[inline]
    pub const fn loader(&self) -> &'info AccountLoader<'info, S> {
        self.loader
    }

    /// Provides an immutable borrow of the underlying shard for the duration
    /// of `f`.
    #[inline]
    pub fn with_ref<R>(&self, f: impl FnOnce(&S) -> R) -> Result<R, ProgramError> {
        let borrow = self.loader.load()?;
        // `Ref` implements `Deref<Target = S>` so we can pass `&*borrow`.
        Ok(f(&*borrow))
        // `borrow` is dropped here, releasing the runtime borrow.
    }

    /// Provides a mutable borrow of the underlying shard for the duration of
    /// `f`.
    #[inline]
    pub fn with_mut<R>(&self, f: impl FnOnce(&mut S) -> R) -> Result<R, ProgramError> {
        let mut borrow = self.loader.load_mut()?;
        let res = f(&mut *borrow);
        // borrow is dropped at end of scope.
        Ok(res)
    }
}
