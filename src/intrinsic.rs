pub mod warrant;
pub mod witness;
pub mod witnessed;

pub use crate::intrinsic::warrant::Warrant;
pub use crate::intrinsic::witness::Witness;
pub use crate::intrinsic::witnessed::Witnessed;

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    /// A test-only witness that accepts any `T`.
    pub(crate) struct Any;

    impl<T> Witness<T> for Any {
        type Error = core::convert::Infallible;
        #[inline]
        fn verify(_: &T) -> Result<(), Self::Error> {
            Ok(())
        }
    }
}

#[cfg(test)]
mod demo {
    use super::*;

    // Invariant: idx < 3
    struct IdxLt3;
    #[derive(Debug, PartialEq, Eq)]
    enum IdxErr {
        OutOfRange { idx: usize },
    }

    impl Witness<usize> for IdxLt3 {
        type Error = IdxErr;

        fn verify(input: &usize) -> Result<(), Self::Error> {
            (*input < 3)
                .then_some(())
                .ok_or(IdxErr::OutOfRange { idx: *input })
        }
    }

    // --- decoupled steps (simulate different modules) ---
    fn parse_idx(s: &str) -> usize {
        s.parse().unwrap()
    }
    fn compute_idx(raw: usize) -> usize {
        raw + 2 // business rule; can push it out of range
    }

    // consumer that *assumes* idx is valid (will panic if not)
    fn pick_unchecked(xs: &[i32; 3], idx: usize) -> i32 {
        xs[idx]
    }

    // boundary function: upgrades plain idx -> witnessed idx
    fn compute_idx_witnessed(raw: usize) -> Result<Witnessed<usize, IdxLt3>, IdxErr> {
        Witnessed::<usize, IdxLt3>::try_new(compute_idx(raw))
    }

    // consumer that requires the invariant in its type signature
    fn pick_checked(xs: &[i32; 3], idx: Witnessed<usize, IdxLt3>) -> i32 {
        xs[*idx]
    }

    #[test]
    #[should_panic]
    fn without_witness_decoupling_can_panic() {
        let xs = [10, 20, 30];

        // Decoupled: parse -> compute -> consume
        // "2" -> 2; +2 -> 4; xs[4] panics.
        let idx = compute_idx(parse_idx("2"));
        let _ = pick_unchecked(&xs, idx);
    }

    #[test]
    fn with_witness_panic_becomes_impossible_in_checked_path() {
        let xs = [10, 20, 30];

        // Same input, but we put the check at the boundary:
        // "2" -> 2; +2 -> 4; witnessing fails => Err; no panic.
        let bad = compute_idx_witnessed(parse_idx("2"));
        assert_eq!(bad, Err(IdxErr::OutOfRange { idx: 4 }));

        // For a valid case, witnessing succeeds and the checked consumer is safe.
        // "0" -> 0; +2 -> 2; xs[2] = 30
        let ok = compute_idx_witnessed(parse_idx("0")).unwrap();
        assert_eq!(pick_checked(&xs, ok), 30);

        // And importantly: you cannot accidentally do this:
        // pick_checked(&xs, 2);
        // ^ type mismatch: expected Witnessed<usize, IdxLt3>, found usize
    }
}
