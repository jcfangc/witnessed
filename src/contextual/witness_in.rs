use crate::contextual::WitnessedIn;

/// A type-level witness for an invariant that is defined *relative to* an environment `Env`.
///
/// This is the contextual counterpart of [`Witness<T>`]. Instead of modeling an absolute
/// predicate `W(T)`, `WitnessIn<T, Env>` models a relation `W(Env, T)`.
///
/// Typical use cases:
/// - stateful / parameterized checks (thresholds, indices, precomputed stats) provided via `Env`
/// - invariants whose meaning depends on external context (e.g. "normalized w.r.t. dataset D")
///
/// # Semantics
///
/// Implementors must treat `verify_in` as a pure check: it must not mutate `env` nor rewrite
/// the value. Construction is performed via the crate-controlled boundary `witness_in` /
/// `WitnessedIn::try_new_in`.
pub trait WitnessIn<T, Env: ?Sized>: Sized {
    type Error;

    /// Verify the invariant relative to `env` without consuming / rewriting the value.
    fn verify_in(env: &Env, input: &T) -> Result<(), Self::Error>;

    /// Construct a witnessed value relative to `env` via the crate-controlled boundary.
    #[inline]
    fn witness_in<'a>(
        env: &'a Env,
        input: T,
    ) -> Result<WitnessedIn<'a, Env, T, Self>, Self::Error> {
        WitnessedIn::<'a, Env, T, Self>::try_new_in(env, input)
    }
}
