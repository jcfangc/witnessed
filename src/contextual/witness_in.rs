use crate::contextual::{WitnessedInOwned, WitnessedInRef};

/// A type-level witness for invariants defined *relative to* an environment `Env`.
///
/// This is the contextual counterpart of [`Witness<T>`]:
///
/// - `Witness<T>` models an absolute invariant `W(T)`.
/// - `WitnessIn<T, Env>` models a contextual invariant `W(Env, T)`.
///
/// Typical use cases:
///
/// - stateful / parameterized checks (thresholds, indices, precomputed stats)
/// - invariants whose meaning depends on external context
///   (e.g. "normalized with respect to dataset D")
///
/// # Semantics
///
/// Implementations must treat `verify_in` as a **pure predicate** over `(env, input)`.
///
/// In particular:
///
/// - it must **not mutate** `env`
/// - it must **not mutate or rewrite** `input`
/// - it must not perform normalization or canonicalization
///
/// Construction of witnessed values occurs through the crate-controlled boundary.
///
/// - `witness_in_ref` returns a borrowed contextual witness tied to `env`'s lifetime
/// - `witness_in_owned` returns an owned contextual witness parameterized by `Env`
///
/// Neither wrapper stores the environment at runtime.
pub trait WitnessIn<T, Env: ?Sized>: Sized {
    type Error;

    /// Verify the invariant relative to `env` without consuming or rewriting `input`.
    fn verify_in(env: &Env, input: &T) -> Result<(), Self::Error>;

    /// Construct a borrowed contextual witness relative to `env`.
    ///
    /// The returned `WitnessedInRef` is tied to the lifetime of `env` but does not
    /// store the environment reference at runtime.
    #[inline]
    fn witness_in_ref<'a>(
        env: &'a Env,
        input: T,
    ) -> Result<WitnessedInRef<'a, Env, T, Self>, Self::Error> {
        WitnessedInRef::<'a, Env, T, Self>::try_new_in(env, input)
    }

    /// Construct an owned-form contextual witness relative to `env`.
    ///
    /// The returned `WitnessedInOwned` is parameterized by `Env` but does not store
    /// the environment at runtime.
    ///
    /// This is useful when the environment type itself is an owned handle such as
    /// `Arc<_>`, and you do not want to thread a borrow lifetime through your APIs.
    #[inline]
    fn witness_in_owned(env: &Env, input: T) -> Result<WitnessedInOwned<Env, T, Self>, Self::Error>
    where
        Env: Sized,
    {
        WitnessedInOwned::<Env, T, Self>::try_new_in(env, input)
    }
}
