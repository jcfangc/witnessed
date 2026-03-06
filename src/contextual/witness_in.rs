use crate::contextual::WitnessedIn;

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
/// Construction of witnessed values occurs through the crate-controlled boundary
/// `witness_in` / `WitnessedIn::try_new_in`.
///
/// The resulting `WitnessedIn` value is **type-level bound** to the lifetime `'a`
/// of `env`, but the environment reference itself is **not stored at runtime**.
/// This allows the proof to remain associated with the environment while keeping
/// the runtime representation minimal.
pub trait WitnessIn<T, Env: ?Sized>: Sized {
    type Error;

    /// Verify the invariant relative to `env` without consuming or rewriting `input`.
    fn verify_in(env: &Env, input: &T) -> Result<(), Self::Error>;

    /// Construct a witnessed value relative to `env`.
    ///
    /// The returned `WitnessedIn` is tied to the lifetime of `env` but does not
    /// store the environment reference at runtime.
    #[inline]
    fn witness_in<'a>(
        env: &'a Env,
        input: T,
    ) -> Result<WitnessedIn<'a, Env, T, Self>, Self::Error> {
        WitnessedIn::<'a, Env, T, Self>::try_new_in(env, input)
    }
}
