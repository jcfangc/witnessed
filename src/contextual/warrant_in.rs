use crate::contextual::{WitnessIn, WitnessedIn};

/// A marker trait authorizing skipping `W::verify_in(env, ..)` for constructing
/// `WitnessedIn<'a, Env, T, W>` under a concrete environment `env`.
///
/// # Safety
/// Implementors must guarantee that values produced under this warrant satisfy `W`'s invariant
/// **with respect to the provided `env`**, assuming any `WitnessedIn` inputs used to compute them
/// are valid under the same `env`.
///
/// In other words, the rule must be *closed* under the same environment instance.
pub unsafe trait WarrantIn<T, Env: ?Sized, W: WitnessIn<T, Env>> {
    /// # Warrant
    /// It is **strongly recommended** to document, at each call site, why `f()` is guaranteed
    /// to satisfy `W`'s invariant under `env`. This makes reviews and audits easier, because
    /// `warrant_in` intentionally skips `W::verify_in` in release builds.
    ///
    /// Suggested call-site pattern:
    /// ```ignore
    /// // Warrant: under this env, combining two normalized values yields a normalized value.
    /// let x: WitnessedIn<'_, EnvTy, f32, NormW> =
    ///     <Rule as WarrantIn<f32, EnvTy, NormW>>::warrant_in(env, || combine(*a, *b));
    /// ```
    #[inline]
    fn warrant_in<'a>(env: &'a Env, f: impl FnOnce() -> T) -> WitnessedIn<'a, Env, T, W> {
        let out = f();
        #[cfg(debug_assertions)]
        {
            if W::verify_in(env, &out).is_err() {
                panic!("warrant violated witness");
            }
        }
        WitnessedIn::new_unchecked(env, out)
    }
}

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
mod warrant_in_tests {
    use crate::contextual::WarrantIn;
    use crate::contextual::test_support::{NormErr, Normalized};
    use crate::contextual::{WitnessIn, WitnessedIn};
    use core::sync::atomic::{AtomicUsize, Ordering};

    // A warrant rule claiming: if a,b are members of the same normalized env, then a+b is also
    // a member (this is NOT generally true). We'll use it with a constructed env where it holds.
    //
    // Safety rationale (restricted to our test env): env = [0.2, 0.3, 0.5], a=0.2, b=0.3 => a+b=0.5 in env.
    struct AddMember01;
    unsafe impl WarrantIn<f32, [f32], Normalized> for AddMember01 {}

    #[inline]
    fn n<'a>(env: &'a [f32], x: f32) -> WitnessedIn<'a, [f32], f32, Normalized> {
        WitnessedIn::<'a, [f32], f32, Normalized>::try_new_in(env, x).unwrap()
    }

    #[test]
    fn normalized_add_member_is_normalized_member_under_same_env() {
        let env_vec = vec![0.2, 0.3, 0.5];
        let env = env_vec.as_slice();

        let a = n(env, 0.2);
        let b = n(env, 0.3);

        let c = AddMember01::warrant_in(env, || *a + *b);

        assert!((*c - 0.5).abs() < 1e-6);
        assert!(core::ptr::eq(c.env(), env));
        assert!(Normalized::verify_in(env, c.as_ref()).is_ok());
    }

    // in debug: warrant_in will verify_in, and panic if violated
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "warrant violated witness")]
    fn warrant_in_panics_if_rule_violated_in_debug() {
        let env_vec = vec![0.2, 0.3, 0.5];
        let env = env_vec.as_slice();

        // 0.2 + 0.5 = 0.7, which is not in env -> should panic in debug
        let a = n(env, 0.2);
        let b = n(env, 0.5);
        let _ = AddMember01::warrant_in(env, || *a + *b);
    }

    // prove warrant_in does not do any extra checks in release, and only one verify_in in debug
    static VERIFY_IN: AtomicUsize = AtomicUsize::new(0);

    struct CountedNorm;
    impl WitnessIn<f32, [f32]> for CountedNorm {
        type Error = NormErr;

        fn verify_in(env: &[f32], x: &f32) -> Result<(), Self::Error> {
            VERIFY_IN.fetch_add(1, Ordering::Relaxed);
            Normalized::verify_in(env, x)
        }
    }

    struct AddMemberCounted;
    unsafe impl WarrantIn<f32, [f32], CountedNorm> for AddMemberCounted {}

    #[inline]
    fn nc<'a>(env: &'a [f32], x: f32) -> WitnessedIn<'a, [f32], f32, CountedNorm> {
        WitnessedIn::<'a, [f32], f32, CountedNorm>::try_new_in(env, x).unwrap()
    }

    #[test]
    fn warrant_in_only_verifies_in_debug() {
        let env_vec = vec![0.2, 0.3, 0.5];
        let env = env_vec.as_slice();

        VERIFY_IN.store(0, Ordering::Relaxed);

        let a = nc(env, 0.2); // verify_in += 1
        let b = nc(env, 0.3); // verify_in += 1
        assert_eq!(VERIFY_IN.load(Ordering::Relaxed), 2);

        let _c = AddMemberCounted::warrant_in(env, || *a + *b);

        #[cfg(debug_assertions)]
        assert_eq!(VERIFY_IN.load(Ordering::Relaxed), 3); // warrant_in verify_in += 1
        #[cfg(not(debug_assertions))]
        assert_eq!(VERIFY_IN.load(Ordering::Relaxed), 2); // no extra verify_in
    }
}
