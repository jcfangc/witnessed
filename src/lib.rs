#![no_std]
// This crate is `no_std`: the public API depends only on `core`, so it can be used in
// embedded/bare-metal or other environments without the Rust standard library.

mod witness_ext;
mod witnessed;
mod witnessing;

pub use witness_ext::WitnessExt;
pub use witnessed::Witnessed;
pub use witnessing::Witnessing;

// Tests run with `std` enabled. We explicitly link `std` in the test configuration so unit tests
// can use conveniences like `vec!`, `format!`, `String`/`Vec`, and `std`-only helpers.
// This does not affect the library in non-test builds.
#[cfg(test)]
#[macro_use]
extern crate std;

#[cfg(test)]
pub(crate) mod test_support;

#[cfg(test)]
mod tests_for_witness_by;

#[cfg(test)]
mod tests_for_size;

#[cfg(test)]
mod tests_for_ord;

#[cfg(test)]
mod tests_for_hash;

#[cfg(test)]
mod tests_for_eq;

#[cfg(test)]
mod tests_for_debug;
