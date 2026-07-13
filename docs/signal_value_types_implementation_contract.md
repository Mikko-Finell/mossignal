# `mossignal` Implementation Contract: Fundamental Signal Value Types

**Status:** Ready for implementation-task transfer  
**Repository:** `Mikko-Finell/mossignal`  
**Specification baseline:** `74a63fd436ddad09837972158721bfc7b433cb6a`  
**Task position:** Premium Foundation Roadmap, Part I, Task 3

---

## 1. Objective

Replace the placeholder starter-library surface with the complete foundational signal-value API for `mossignal`'s two closed signal kinds.

The accepted implementation must establish:

- type-level `Level` and `Pulse` markers;
- erased `SignalKind` values;
- a sealed `SignalType` association between the typed and erased representations;
- the complete binary `LogicLevel` value model;
- the initial checked `u64`-backed `PulseCount` value model;
- a dedicated `PulseCountOverflow` error;
- focused tests and compile-fail documentation proving the required positive and negative guarantees.

This task establishes only the foundational signal-value model. It must not begin graph, node, runtime, diagnostics, time, or persistence implementation.

---

## 2. Source of truth

The repository specifications under `docs/specs/` are authoritative. This contract is derived from the following sections.

### Primary API authority

- `docs/specs/concrete_rust_api_surface.md`
  - § **Normative language**
  - § **Closed semantic universe**
  - § **Public module organization**
  - § **Signal-kind markers**
  - § **Logic levels**
  - § **Pulse counts**

This specification governs the concrete public Rust surface, type distinctions, ownership relationships, and failure boundaries.

### Semantic authority

- `docs/specs/api_and_semantics_spec.md`
  - § **Signal kinds**
- `docs/specs/built_in_node_semantics.md`
  - § **Signal kinds**
  - § **Pulse multiplicity**

These sections govern the meaning of levels and pulses, including the rule that `Low` is a real signal value rather than absence and that pulse multiplicity is semantically significant.

### Adjacent constraints

- `docs/specs/testing_and_verification_policy.md`
  - § **Pulse algebra**
- `docs/specs/persistence_canonical_encoding_and_compatibility_spec.md`
  - § **Time, span, pulse count, and revision**
- `docs/specs/exhaustive_diagnostic_code_catalogue.md`
  - § **Runtime values, time, policy, and semantic rejection**
  - § **Required mapping by API family**

These sections constrain the representable pulse-count range, checked overflow behavior, and future mapping of `PulseCountOverflow` to `runtime.pulse_count_overflow`. They do not bring transaction atomicity, canonical encoding, or the diagnostic framework into this task.

If this contract conflicts with an authoritative repository specification at the stated baseline, the specification governs and the conflict must be surfaced rather than silently resolved.

---

## 3. Scope boundary

### Included

- one public `signal` module;
- removal of the placeholder `project_name()` API and its placeholder tests;
- `Level`;
- `Pulse`;
- `SignalKind`;
- sealed `SignalType`;
- `LogicLevel`;
- `PulseCount`;
- `PulseCountOverflow`;
- concise public documentation;
- focused unit tests;
- compile-fail doctests for negative API guarantees.

### Explicitly excluded

- graph representation;
- stable keys;
- ports and endpoints;
- network builders;
- node definitions or node kinds;
- reaction semantics or pulse storage;
- logical-time types;
- validation or compilation;
- machine state or runtime execution;
- transaction atomicity implementation;
- diagnostic codes, `Problem`, reports, or runtime failure enums;
- persistence encoding or serialization derives;
- prelude design;
- `no_std` policy;
- new dependencies;
- speculative abstraction layers or compatibility shims.

---

## 4. Required public module surface

The implementation must expose the following public paths:

```text
mossignal::signal::Level
mossignal::signal::Pulse
mossignal::signal::SignalKind
mossignal::signal::SignalType
mossignal::signal::LogicLevel
mossignal::signal::PulseCount
mossignal::signal::PulseCountOverflow
```

The expected source shape is:

```text
crates/mossignal/src/lib.rs
crates/mossignal/src/signal.rs
```

`lib.rs` must publicly declare the `signal` module and must no longer expose the placeholder `project_name()` API.

Do not create empty future modules, a prelude, or crate-root re-exports merely to anticipate later roadmap tasks.

---

## 5. Obligations

## O1. Establish the closed signal-kind universe

Define the two type-level markers:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Level {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pulse {}
```

Required properties:

- both are uninhabited marker types;
- both are zero-sized;
- neither represents a runtime signal value;
- no third signal-kind marker is introduced;
- no open registration or extension mechanism exists.

A unit struct is not an acceptable substitute because it is constructible as a runtime value.

---

## O2. Provide the erased signal-kind representation

Define:

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SignalKind {
    Level,
    Pulse,
}
```

Required properties:

- `SignalKind::Level` and `SignalKind::Pulse` are distinct erased categories;
- no `Unknown`, custom, or extension variant is added;
- numeric discriminants and declaration-order values are not made semantic public contracts;
- `#[non_exhaustive]` reserves crate evolution but does not make the semantic universe downstream-extensible.

---

## O3. Seal `SignalType`

Provide a public trait equivalent in responsibility to:

```rust
pub trait SignalType: private::Sealed {
    const KIND: SignalKind;
}
```

Implement it only for the two core marker types:

```text
Level::KIND = SignalKind::Level
Pulse::KIND = SignalKind::Pulse
```

Required properties:

- downstream crates cannot implement `SignalType` for another type;
- there is no blanket implementation;
- the trait does not acquire unrelated requirements such as `Default`, serialization, or host-integration traits;
- the sealing mechanism remains private implementation detail.

A compile-fail doctest must demonstrate that an external type cannot implement `SignalType`.

---

## O4. Implement exact binary `LogicLevel` semantics

Define:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LogicLevel {
    Low,
    High,
}
```

Provide:

```rust
impl LogicLevel {
    pub const fn invert(self) -> Self;
    pub const fn is_low(self) -> bool;
    pub const fn is_high(self) -> bool;
}

impl core::ops::Not for LogicLevel {
    type Output = LogicLevel;

    fn not(self) -> LogicLevel;
}
```

Required laws:

```text
Low.invert()  = High
High.invert() = Low
!Low          = High
!High         = Low
x.invert().invert() = x
```

Predicate laws:

| Value | `is_low()` | `is_high()` |
|---|---:|---:|
| `Low` | `true` | `false` |
| `High` | `false` | `true` |

Required semantic boundary:

- `LogicLevel` has exactly two values;
- `Low` is an established signal value, not absence or uninitialized state;
- no `Unknown`, `Unset`, `None`, floating, or third logic value is introduced;
- semantically level-valued APIs in this module use `LogicLevel`, not `bool`.

Deliberately omit from this task:

- `Default`;
- Boolean conversions;
- parsing;
- `Display`;
- numeric conversion;
- serialization.

Those additions are permitted only by future deliberate API work; they are not required for this foundation.

---

## O5. Implement checked pulse multiplicity

Define the initial public representation:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PulseCount(u64);
```

The private representation must be `u64`. The complete valid domain is:

```text
0 ..= u64::MAX
```

Provide:

```rust
impl PulseCount {
    pub const ZERO: Self;
    pub const ONE: Self;

    pub const fn new(value: u64) -> Self;
    pub const fn get(self) -> u64;
    pub const fn is_zero(self) -> bool;
    pub const fn is_positive(self) -> bool;

    pub fn checked_add(
        self,
        other: Self,
    ) -> Result<Self, PulseCountOverflow>;
}
```

Required properties:

- every `u64` value is a valid pulse count;
- zero is valid and represents no occurrence in the relevant batch;
- the constructor is infallible;
- equality and ordering correspond to the wrapped numeric count;
- `checked_add` returns the exact mathematical sum when it is representable;
- `checked_add` returns `PulseCountOverflow` when the sum exceeds `u64::MAX`;
- no operation may silently wrap or saturate.

Do not implement:

- `Add`;
- `AddAssign`;
- `Sum`;
- wrapping or saturating arithmetic;
- subtraction;
- multiplication;
- mutable setters;
- implicit integer conversions;
- serialization.

A compile-fail doctest must demonstrate that ordinary `PulseCount + PulseCount` is unavailable.

---

## O6. Provide a dedicated `PulseCountOverflow` error

`PulseCountOverflow` is required because it appears in the specified `checked_add` signature, but its detailed Rust representation is not fixed by the specifications. For this implementation, settle it as follows.

Define a dedicated public error type broadly equivalent to:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PulseCountOverflow {
    left: PulseCount,
    right: PulseCount,
}
```

The exact private field names and internal constructor shape may differ.

Required properties:

- the error retains both failed operands in its private representation;
- callers cannot construct arbitrary invalid error values through public fields;
- the type implements `Debug`, `Clone`, `Copy`, `PartialEq`, and `Eq`;
- the type implements `Display` with a concise useful description of pulse-count overflow;
- the exact `Display` wording is not a compatibility contract and need not expose the operands;
- the type implements `std::error::Error`;
- no heap allocation is required to construct or return it;
- `checked_add` returns it for every unrepresentable sum.

Do not implement the future diagnostic model in this task. In particular, do not add:

- `DiagnosticCode`;
- `Problem` or typed problem evidence;
- severity or responsibility values;
- runtime-failure wrappers;
- diagnostic rendering infrastructure.

The retained operands must make a later lossless mapping to `runtime.pulse_count_overflow` possible without redesigning `PulseCount` arithmetic. Public operand accessors are not required by this task. Crate-private access may be added later when diagnostic integration is implemented.

---

## O7. Document the public semantic boundary

Every public item introduced by this task must have concise Rust documentation sufficient for `RUSTDOCFLAGS=-Dwarnings cargo doc`.

Documentation must make these facts clear where relevant:

- `Level` and `Pulse` are type markers rather than signal values;
- the signal universe is closed and `SignalType` is sealed;
- `LogicLevel::Low` is not absence;
- a pulse count is a non-negative simultaneous multiplicity;
- pulse-count addition is checked;
- `PulseCountOverflow` means the mathematical sum exceeded the representable `u64` range.

Documentation must not claim that graphs, nodes, transactions, persistence, or runtime execution already exist.

---

## 6. Required verification

Tests must establish behavior and API boundaries rather than merely instantiate the types.

### Marker and trait verification

- `size_of::<Level>() == 0`;
- `size_of::<Pulse>() == 0`;
- the required marker traits compile;
- `Level::KIND == SignalKind::Level`;
- `Pulse::KIND == SignalKind::Pulse`;
- a compile-fail doctest proves that an external type cannot implement `SignalType`.

### `LogicLevel` verification

Exhaustively test both values for:

- `invert`;
- `Not`;
- `is_low`;
- `is_high`;
- double inversion;
- declared ordering consistency (`Low < High`).

The test should enumerate the complete two-value domain rather than sample it.

### `PulseCount` verification

At minimum, exercise:

```text
0
1
2
u64::MAX
```

Verify:

- `ZERO` and `ONE` values;
- `new` and `get` round-trip;
- zero and positive predicates;
- numeric equality and ordering;
- successful addition at ordinary and maximum boundaries;
- overflow in both operand orders where relevant.

Required checked-add cases include:

```text
0 + 0       = 0
0 + MAX     = MAX
MAX + 0     = MAX
1 + 1       = 2
MAX + 1     = overflow
1 + MAX     = overflow
MAX + MAX   = overflow
```

Also require:

- a module-local unit test verifies directly that overflow errors retain the two attempted operands in their private representation; public fields, accessors, and operand-bearing `Display` text are not required;
- a compile-fail doctest proves ordinary `+` is unavailable.

### Repository gates

The completed implementation must pass:

```bash
make check-dev
make check-final
```

A test count is not an acceptance criterion. Complete coverage of the finite laws and boundary behavior is.

---

## 7. Forbidden outcomes

The implementation is noncompliant if it does any of the following:

- makes `Level` or `Pulse` constructible runtime marker values;
- permits downstream implementations of `SignalType`;
- adds a third signal kind or an open signal-kind registry;
- represents `LogicLevel` as a raw public `bool` API;
- adds an unknown or uninitialized logic value;
- treats `Low` as absence;
- represents `PulseCount` as signed, Boolean, arbitrary precision, or saturating;
- permits pulse-count arithmetic to wrap;
- implements ordinary `Add`, `AddAssign`, or another infallible overflowing arithmetic trait;
- replaces `PulseCountOverflow` with an arbitrary string or a generic fallback error;
- begins implementation of diagnostics, persistence, time, graphs, builders, nodes, or runtime machinery;
- adds serialization derives because `serde` happens to be present;
- creates empty future modules or speculative abstractions;
- keeps `project_name()` as an undocumented compatibility shim;
- adds dependencies for functionality available directly from the standard library.

---

## 8. Implementation freedom

The implementer may choose:

- private helper names and layout;
- whether tests are colocated in `signal.rs` or placed in a focused integration-test file;
- exact non-semantic wording of documentation and `Display` output;
- whether eligible methods beyond those explicitly required are `const`, provided this does not enlarge the semantic API or weaken compatibility;
- ordinary formatting and local code organization.

The implementer must not reinterpret explicit public types, derives, methods, sealing, arithmetic behavior, or scope exclusions as private implementation freedom.

---

## 9. Expected change surface

Expected:

```text
crates/mossignal/src/lib.rs
crates/mossignal/src/signal.rs
```

A focused test file is acceptable when useful.

Not expected:

```text
Cargo.toml dependency additions
other source modules
docs/specs changes
workflow or bead-process changes
```

Unrelated repository cleanup should not be mixed into this task.

---

## 10. Completion criteria

The task is complete only when all of the following hold:

1. The placeholder library API has been removed.
2. `mossignal::signal` exposes the complete required public surface.
3. The two-kind signal universe is represented by uninhabited markers and sealed against downstream extension.
4. `LogicLevel` implements the complete specified binary behavior without an absence value.
5. `PulseCount` represents the complete initial `u64` domain.
6. Every pulse-count sum is either exact or rejected with `PulseCountOverflow`; no wrapping or saturation path exists.
7. The overflow error is a dedicated structured Rust error retaining both attempted operands privately.
8. Focused unit tests exhaust the finite level laws and cover pulse-count boundaries.
9. Compile-fail doctests protect the sealed-trait and unavailable-ordinary-addition guarantees.
10. Public documentation is accurate for the implemented foundation and passes strict documentation checks.
11. `make check-dev` and `make check-final` pass without weakening repository gates.
12. No excluded adjacent subsystem has been introduced.

---

## 11. Open decisions

None.

The task is sufficiently specified for implementation and independent review.
