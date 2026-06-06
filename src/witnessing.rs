use crate::Witnessed;

/// Intermediate builder returned by `value.witness()`.
pub struct Witnessing<T> {
    pub(crate) value: T,
}

impl<T> Witnessing<T> {
    #[inline]
    pub fn by<W, E>(self, prove: impl FnOnce(&T) -> Result<W, E>) -> Result<Witnessed<T, W>, E> {
        let _witness = prove(&self.value)?;
        Ok(Witnessed::new_unchecked(self.value))
    }

    /// Witness this value without producing or checking a witness token.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that `self.value` satisfies the invariant
    /// represented by witness token type `W`.
    #[inline]
    pub unsafe fn by_unchecked<W>(self) -> Witnessed<T, W> {
        Witnessed::new_unchecked(self.value)
    }
}
