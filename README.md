# witnessed

[![crates.io](https://img.shields.io/crates/v/witnessed.svg)](https://crates.io/crates/witnessed)

A small Rust pattern for carrying boundary-established facts through the type system.

`witnessed` separates:

* **witness production at boundaries**
* **trusted use inside the program**

A value becomes `Witnessed<T, W>` only after a witness token `W` has been produced for it.

This helps prevent “unchecked value slipped into internal logic” bugs across decoupled modules.

---

## Core idea

`Witnessed<T, W>` is a transparent wrapper over `T`.

The type parameter `W` is a **witness token type**. It represents a fact that has been established for the wrapped value.

`W` is not required to implement a trait.

Instead, a value is witnessed through a proof-producing function or closure:

```rust
use witnessed::WitnessExt;

let witnessed = value.witness().by(W::prove)?;
```

Conceptually:

```text
witness this value by producing W
```

The witness token `W` is used only at the construction boundary. It is not stored at runtime.

---

## Example

```rust
use witnessed::{WitnessExt, Witnessed};

#[derive(Debug, PartialEq, Eq)]
enum IdxErr {
    OutOfRange { idx: usize },
}

struct IdxLt3;

impl IdxLt3 {
    fn prove(idx: &usize) -> Result<Self, IdxErr> {
        (*idx < 3)
            .then_some(Self)
            .ok_or(IdxErr::OutOfRange { idx: *idx })
    }
}

fn pick(xs: &[i32; 3], idx: Witnessed<usize, IdxLt3>) -> i32 {
    xs[*idx]
}

fn main() -> Result<(), IdxErr> {
    let idx = 2usize.witness().by(IdxLt3::prove)?;

    assert_eq!(pick(&[10, 20, 30], idx), 30);

    Ok(())
}
```

The internal function `pick` does not accept a raw `usize`. It requires `Witnessed<usize, IdxLt3>`, so callers must cross the witnessing boundary first.

---

## API shape

The primary construction path is:

```rust
value.witness().by(prove)?
```

where `prove` has this shape:

```rust
FnOnce(&T) -> Result<W, E>
```

For example:

```rust
let x = raw.witness().by(MyWitness::prove)?;
```

or with a closure:

```rust
let x = raw.witness().by(|value| MyWitness::prove_with(&env, value))?;
```

The result is:

```rust
Result<Witnessed<T, W>, E>
```

---

## Context-dependent witnesses

Context-dependent facts do not require a special wrapper type.

Capture the context in the proof-producing closure:

```rust
use witnessed::WitnessExt;

struct InRange;

#[derive(Debug, PartialEq, Eq)]
enum RangeErr {
    OutOfRange { value: i32 },
}

struct Range {
    start: i32,
    end_excl: i32,
}

impl InRange {
    fn prove_in(range: &Range, value: &i32) -> Result<Self, RangeErr> {
        (range.start <= *value && *value < range.end_excl)
            .then_some(Self)
            .ok_or(RangeErr::OutOfRange { value: *value })
    }
}

fn main() -> Result<(), RangeErr> {
    let range = Range {
        start: 10,
        end_excl: 20,
    };

    let x = 15.witness().by(|value| InRange::prove_in(&range, value))?;

    assert_eq!(*x, 15);

    Ok(())
}
```

The resulting type records only:

```rust
Witnessed<i32, InRange>
```

It does not store `range`, and it does not encode the lifetime of `range`.

This crate models a witnessed value as:

> a value that crossed a witness-producing boundary

not as:

> a value that remains dynamically tied to an environment after construction

---

## Unsafe witnessing

Sometimes a value is derived from already-witnessed inputs, and rechecking would be redundant.

For those cases, use the explicit unsafe boundary:

```rust
unsafe {
    value.witness().by_unchecked::<W>()
}
```

Example:

```rust
use witnessed::{WitnessExt, Witnessed};

struct ZeroOne;

fn mul01(
    a: Witnessed<f32, ZeroOne>,
    b: Witnessed<f32, ZeroOne>,
) -> Witnessed<f32, ZeroOne> {
    let out = *a * *b;

    // Safety:
    // If a and b are both in [0, 1], then a * b is also in [0, 1].
    unsafe { out.witness().by_unchecked::<ZeroOne>() }
}
```

`by_unchecked` does not run a proof function.

The caller must guarantee that the value satisfies the fact represented by `W`.

---

## Dropping the witness

Use `into_inner()` when intentionally returning to the raw value:

```rust
let raw = witnessed.into_inner();
```

After extraction, the witness guarantee is no longer represented in the type system.

---

## Representation

`Witnessed<T, W>` is `repr(transparent)` over `T`.

The witness token type `W` participates only in the type system. It is not stored at runtime.

This means:

* no runtime witness field
* no environment field
* no extra pointer
* no additional size overhead
* auto-traits are driven by `T`, not by `W`

---

## `no_std`

This crate supports `#![no_std]`.

The core API depends only on `core`.

Tests use `std` under `#[cfg(test)]`.

---

## Pattern

Typical usage:

1. Accept raw values at boundaries.
2. Produce witness tokens with `value.witness().by(...)`.
3. Require `Witnessed<T, W>` in internal APIs.
4. Use `into_inner()` only when intentionally dropping the type-level fact.
5. Use `unsafe { value.witness().by_unchecked::<W>() }` only for audited trusted derivations.

---

## Design note

`W` is a witness token type, not a validator trait.

This keeps the API small:

```rust
value.witness().by(W::prove)?
value.witness().by(|value| W::prove_with(&env, value))?
unsafe { value.witness().by_unchecked::<W>() }
```

The crate does not prescribe how witness tokens are produced.

They may come from:

* associated functions
* free functions
* closures
* context-capturing closures
* audited unsafe derivation rules

The only thing `Witnessed<T, W>` records is that construction crossed a boundary that established `W` for `T`.
