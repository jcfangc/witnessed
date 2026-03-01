use crate::intrinsic::Witnessed;

/// A type-level witness that can attest `T` satisfies some invariant.
///
/// Implementors are expected to check (and optionally normalize) `input` via `attest`,
/// and provide a non-consuming `verify` for debug-time validation.
pub trait Witness<T>: Sized {
    type Error;

    /// Verify the invariant without consuming / rewriting the value.
    ///
    /// This is used by warrant-based construction in debug builds to avoid `Clone`.
    fn verify(input: &T) -> Result<(), Self::Error>;
    /// Construct a witnessed value via the crate-controlled boundary.
    #[inline]
    fn witness(input: T) -> Result<Witnessed<T, Self>, Self::Error> {
        Witnessed::<T, Self>::try_new(input)
    }
}
