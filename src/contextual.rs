pub mod warrant_in;
pub mod witness_in;
pub mod witnessed_in;

pub use crate::contextual::warrant_in::WarrantIn;
pub use crate::contextual::witness_in::WitnessIn;
pub use crate::contextual::witnessed_in::WitnessedIn;

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    /// A test-only env-dependent witness that accepts any `T` under any `Env`.
    ///
    /// Useful when the test is about wrapper behavior (Deref/Debug/Hash/Ord),
    /// not about the witness logic itself.
    pub(crate) struct AnyIn;

    impl<T, Env: ?Sized> WitnessIn<T, Env> for AnyIn {
        type Error = core::convert::Infallible;
        #[inline]
        fn verify_in(_: &Env, _: &T) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    /// Env-dependent "normalized" witness for `f32` relative to an env container `&[f32]`.
    ///
    /// Invariant:
    /// - `env` is non-empty
    /// - all env elements are finite
    /// - sum(env) == 1 within `eps`
    /// - `x` is finite
    /// - `x` is a member of `env` (exact equality)
    pub(crate) struct Normalized;

    #[derive(Debug, PartialEq)]
    pub(crate) enum NormErr {
        EnvEmpty,
        EnvNonFinite,
        EnvSumNotOne { sum: f32 },
        ValueNonFinite,
        NotMember { x: f32 },
    }

    impl WitnessIn<f32, [f32]> for Normalized {
        type Error = NormErr;

        #[inline]
        fn verify_in(env: &[f32], x: &f32) -> Result<(), Self::Error> {
            if env.is_empty() {
                return Err(NormErr::EnvEmpty);
            }
            if !x.is_finite() {
                return Err(NormErr::ValueNonFinite);
            }
            if !env.iter().all(|v| v.is_finite()) {
                return Err(NormErr::EnvNonFinite);
            }

            let sum = env.iter().copied().sum::<f32>();
            let eps = 1e-6f32;
            if (sum - 1.0).abs() > eps {
                return Err(NormErr::EnvSumNotOne { sum });
            }

            env.iter()
                .any(|v| v == x)
                .then_some(())
                .ok_or(NormErr::NotMember { x: *x })
        }
    }
}
