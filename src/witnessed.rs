use core::marker::PhantomData;

#[repr(transparent)]
pub struct Witnessed<T, W> {
    inner: T,
    _marker: PhantomData<fn() -> W>,
}

impl<T, W> Witnessed<T, W> {
    /// Consume and return the inner value.
    ///
    /// Note: extracting `T` loses the witness guarantee in the type system.
    #[inline]
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T, W> Witnessed<T, W> {
    /// Internal constructor; keeps `Witnessed` unforgeable across crates.
    ///
    /// Do NOT make this public: the entire pattern relies on forcing construction through
    /// `W::attest` (via `try_new` / `W::witness`) so invariants cannot be bypassed downstream.
    #[inline]
    pub(crate) fn new_unchecked(inner: T) -> Self {
        Self {
            inner,
            _marker: PhantomData,
        }
    }
}

use core::{fmt, hash, ops::Deref};

impl<T, W> Deref for Witnessed<T, W> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T, W> AsRef<T> for Witnessed<T, W> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

impl<T: Clone, W> Clone for Witnessed<T, W> {
    #[inline]
    fn clone(&self) -> Self {
        Self::new_unchecked(self.inner.clone())
    }
}
impl<T: Copy, W> Copy for Witnessed<T, W> {}

impl<T: fmt::Debug, W> fmt::Debug for Witnessed<T, W> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Witnessed").field(&self.inner).finish()
    }
}

impl<T: PartialEq, W> PartialEq for Witnessed<T, W> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.inner.eq(&other.inner)
    }
}
impl<T: Eq, W> Eq for Witnessed<T, W> {}

impl<T: PartialOrd, W> PartialOrd for Witnessed<T, W> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.inner.partial_cmp(&other.inner)
    }
}
impl<T: Ord, W> Ord for Witnessed<T, W> {
    #[inline]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.inner.cmp(&other.inner)
    }
}

impl<T: hash::Hash, W> hash::Hash for Witnessed<T, W> {
    #[inline]
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}
