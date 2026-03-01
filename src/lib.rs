#![no_std]
// This crate is `no_std`: the public API depends only on `core`, so it can be used in
// embedded/bare-metal or other environments without the Rust standard library.

// Tests run with `std` enabled. We explicitly link `std` in the test configuration so unit tests
// can use conveniences like `vec!`, `format!`, `String`/`Vec`, and `std`-only helpers.
// This does not affect the library in non-test builds.
#[cfg(test)]
#[macro_use]
extern crate std;

pub mod contextual;
pub mod intrinsic;
