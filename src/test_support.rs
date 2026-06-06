use crate::{WitnessExt, Witnessed};

/// Generic test witness token.
pub(crate) struct AnyToken;

/// Deliberately non-ZST token.
///
/// Used to verify that `W` remains type-level only and does not affect
/// the layout of `Witnessed<T, W>`.
pub(crate) struct LargeToken(#[allow(unused)] [u8; 128]);

/// Build a witnessed value for tests that only care about wrapper behavior.
///
/// This intentionally bypasses witness production. It should only be used in
/// tests for transparent delegation/layout behavior, not for witness-boundary tests.
#[inline]
pub(crate) fn witnessed<T>(value: T) -> Witnessed<T, AnyToken> {
    unsafe { value.witness().by_unchecked::<AnyToken>() }
}
