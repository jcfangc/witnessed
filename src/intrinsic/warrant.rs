use crate::intrinsic::{Witness, Witnessed};

/// A marker trait authorizing skipping `W::attest` for constructing `Witnessed<T, W>`.
///
/// # Safety
/// Implementors must guarantee that values produced under this warrant satisfy `W`'s invariant,
/// assuming any `Witnessed<T, W>` inputs used to compute them are valid.
pub unsafe trait Warrant<T, W: Witness<T>> {
    /// # Warrant
    /// It is **strongly recommended** to document, at each call site, why `f()` is guaranteed
    /// to satisfy `W`'s invariant. This makes reviews and audits easier, because `warrant`
    /// intentionally skips `W::attest`.
    ///
    /// Suggested call-site pattern:
    /// ```ignore
    /// // Warrant: if a,b in [0,1] then a*b in [0,1].
    /// let x: Witnessed<f32, ZeroOne> = <Mul01 as Warrant<f32, ZeroOne>>::warrant(|| *a * *b);
    /// ```
    #[inline]
    fn warrant(f: impl FnOnce() -> T) -> Witnessed<T, W> {
        let out = f();
        #[cfg(debug_assertions)]
        {
            if W::verify(&out).is_err() {
                panic!("warrant violated witness");
            }
        }
        Witnessed::new_unchecked(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicUsize, Ordering};

    // ZeroOne witness (0..=1, finite)
    struct ZeroOne;
    #[derive(Debug, PartialEq)]
    enum ZErr {
        NonFinite,
        OutOfRange(f32),
    }

    impl Witness<f32> for ZeroOne {
        type Error = ZErr;

        #[inline]
        fn verify(x: &f32) -> Result<(), Self::Error> {
            if !x.is_finite() {
                return Err(ZErr::NonFinite);
            }
            (0.0 <= *x && *x <= 1.0)
                .then_some(())
                .ok_or(ZErr::OutOfRange(*x))
        }
    }

    // A warrant rule claiming: product of two ZeroOne is still ZeroOne.
    // Safety rationale: if a,b in [0,1], then a*b in [0,1].
    struct Mul01;
    unsafe impl Warrant<f32, ZeroOne> for Mul01 {}

    #[inline]
    fn z(x: f32) -> Witnessed<f32, ZeroOne> {
        Witnessed::<f32, ZeroOne>::try_new(x).unwrap()
    }

    #[test]
    fn zero_one_mul_is_zero_one() {
        let a = z(0.2);
        let b = z(0.3);

        let c = Mul01::warrant(|| *a * *b);

        assert!((*c - 0.06).abs() < 1e-6);
        assert!(ZeroOne::verify(c.as_ref()).is_ok());
    }

    // in debug: warrant will verify, and panic if violated
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "warrant violated witness")]
    fn warrant_panics_if_rule_violated_in_debug() {
        let _ = Mul01::warrant(|| 1.2);
    }

    // prove warrant does not do any extra checks in release, and only one verify in debug
    static VERIFY: AtomicUsize = AtomicUsize::new(0);

    struct Counted01;
    impl Witness<f32> for Counted01 {
        type Error = ZErr;

        #[inline]
        fn verify(x: &f32) -> Result<(), Self::Error> {
            VERIFY.fetch_add(1, Ordering::Relaxed);
            ZeroOne::verify(x)
        }
    }

    struct MulCounted01;
    unsafe impl Warrant<f32, Counted01> for MulCounted01 {}

    #[test]
    fn warrant_only_verifies_in_debug() {
        VERIFY.store(0, Ordering::Relaxed);

        let a = Witnessed::<f32, Counted01>::try_new(0.4).unwrap(); // verify += 1
        let b = Witnessed::<f32, Counted01>::try_new(0.5).unwrap(); // verify += 1
        assert_eq!(VERIFY.load(Ordering::Relaxed), 2);

        let _c = MulCounted01::warrant(|| *a * *b);

        #[cfg(debug_assertions)]
        assert_eq!(VERIFY.load(Ordering::Relaxed), 3); // warrant verify += 1
        #[cfg(not(debug_assertions))]
        assert_eq!(VERIFY.load(Ordering::Relaxed), 2); // no extra verify
    }
}
