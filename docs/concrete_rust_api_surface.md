# `mossignal` Concrete Rust API Surface

**Status:** Design specification, version 1  
**Defines:** Concrete public Rust types, ownership model, construction APIs, lifecycle transitions, runtime transaction APIs, inspection, explanation, bindings, snapshots, replay, and reconfiguration entry points  
**Does not define:** Processor internals, serialized wire encodings, the exhaustive diagnostic-code catalogue, the exhaustive topology-patch operation language, performance targets, standard-module catalogue, or application integration

---

## 1. Purpose

This specification translates the semantic and architectural model of `mossignal` into a coherent public Rust API.

It defines:

- the public type hierarchy;
- which invariants are represented statically;
- which invariants remain dynamically validated;
- ownership and borrowing rules;
- stable and revision-bound identity;
- typed and dynamic network authoring;
- validation and compilation transitions;
- machine creation and lifecycle access;
- input, transaction, forecast, and result construction;
- inspection, explanation, graph, binding, persistence, replay, and reconfiguration entry points;
- error and diagnostic return shapes;
- the intended allocation boundary of the public API.

The API must make ordinary correct use straightforward while keeping dynamically authored, persisted, reconfigured, and inspected networks fully representable.

The API must not encode every semantic fact into Rust typestate or generative lifetimes. Static typing is used where it prevents common category errors without making the library difficult to compose, store, serialize, inspect, or use from tooling.

---

## 2. Normative language

The terms **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.

Code shown in this specification is representative of the required public shape. Exact private fields, derive lists, module paths, and names of narrowly scoped helper types may differ where the same guarantees and ergonomics are preserved.

Public responsibilities, ownership relationships, type distinctions, and failure boundaries are normative.

Where earlier specifications contain illustrative Rust signatures, this specification governs the concrete Rust surface. The API and node specifications remain authoritative for semantics, the processor specification remains authoritative for internal invariants, and the Exhaustive Diagnostic Code Catalogue remains authoritative for problem-code identity, severity, responsibility, evidence, delivery, ordering, and persistent-episode rules.

---

# Part I — API design principles

## 3. Static versus dynamic invariants

The API MUST represent the following statically where practical:

```text
signal kind: Level versus Pulse
port direction: input versus output
time domain
validated versus unchecked network
compiled topology versus authored topology
complete initialization snapshot versus ready-machine delta
typed versus erased endpoint access
snapshot, forecast, patch, and replay artifact categories
```

The API MAY validate the following dynamically:

```text
signals belonging to the same builder
stable-key uniqueness
revision freshness
network and input-schema identity
complete external level coverage
current-reaction acyclicity
state-schema compatibility
patch migration compatibility
snapshot compatibility
runtime-policy budgets
machine lifecycle legality
```

The library MUST NOT introduce lifetime-heavy or typestate-heavy APIs merely to make rare invalid operations unrepresentable when a precise structured runtime failure is simpler and more composable.

## 4. Closed semantic universe

The core signal and built-in-node universe is closed.

The public API MUST use:

- marker types for `Level` and `Pulse`;
- closed enums for erased signal kinds, node kinds, ports, subjects, observations, and events;
- concrete configuration values for built-in nodes.

The core MUST NOT expose unrestricted evaluator trait objects or callback-defined nodes.

## 5. Owned semantic artifacts

The canonical public forms of the following MUST be owned values:

```text
UncheckedNetwork
ValidatedNetwork
CompiledNetwork
Transaction
InputSnapshot
InputDelta
TransactionResult
ForecastResult
MachineSnapshot
ReplayFrame
NetworkPatch
PreparedPatch
InspectionQuery
InspectionSnapshot
Explanation
Problem
Diagnostic
DiagnosticOccurrence
DiagnosticEpisode
InternalDefect
```

This permits storage, replay, persistence, transfer between subsystems, and deterministic testing without borrowing a live builder or machine.

Borrowed views MAY exist as optional performance conveniences, but they MUST NOT be the only public representation of semantic information.

## 6. Opaque representation

Types that rely on validated internal invariants MUST have private fields.

At minimum, the following MUST be opaque:

```text
Signal<S>
all stable key types
all resolved handle types
ValidatedNetwork<D>
CompiledNetwork<D>
Machine<D>
InputSnapshot<D>
InputDelta<D>
Transaction<D>
PreparedPatch<D>
MachineSnapshot<D>
CauseRef
PendingEventKey
all digest and fingerprint types
```

Callers may inspect these types only through their documented methods or structured projections.

## 7. Allocation boundary

The public API permits ordinary heap allocation during:

- authoring;
- validation;
- compilation;
- patch preparation;
- snapshots and replay construction;
- inspection and explanation;
- creation of committed semantic artifacts such as events, provenance, and transaction results.

The public API MUST NOT require one heap allocation or dynamic dispatch object per runtime node evaluation.

Names and other strings MUST NOT participate in evaluator semantics. Moving an owned public artifact MUST move ownership rather than clone contained strings unless the caller explicitly clones the artifact.

---

# Part II — Crate organization and prelude

## 8. Public module organization

The initial crate SHOULD expose a module organization broadly equivalent to:

```rust
pub mod signal;
pub mod time;
pub mod key;
pub mod metadata;
pub mod definition;
pub mod builder;
pub mod diagnostic;
pub mod compile;
pub mod runtime;
pub mod inspect;
pub mod explain;
pub mod graph;
pub mod binding;
pub mod reconfigure;
pub mod persistence;
pub mod replay;
```

The crate SHOULD provide a curated prelude:

```rust
pub mod prelude {
    pub use crate::builder::NetworkBuilder;
    pub use crate::compile::{CompiledNetwork, ValidatedNetwork};
    pub use crate::runtime::{
        InputDelta, InputSnapshot, Machine, RuntimePolicy,
        Transaction, TransactionResult,
    };
    pub use crate::signal::{Level, LogicLevel, Pulse, PulseCount};
    pub use crate::time::{NonZeroSpan, Span, Time};
}
```

The prelude SHOULD contain common authoring and execution types only. It SHOULD NOT glob-export the complete diagnostic, inspection, graph, persistence, or reconfiguration surface.

## 9. One cohesive crate

The initial public API belongs to one cohesive library crate.

Public module boundaries MUST NOT imply independent semantic runtimes, separate identity spaces, or separately versioned execution engines.

A future crate split may occur only for a genuine dependency, platform, release, or reuse boundary.

---

# Part III — Fundamental signal types

## 10. Signal-kind markers

The signal kinds are zero-sized markers:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Level {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pulse {}
```

They MUST NOT be instantiated as runtime signal values.

The erased signal kind is:

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SignalKind {
    Level,
    Pulse,
}
```

An internal sealed trait MAY associate marker types with their erased kind:

```rust
pub trait SignalType: private::Sealed {
    const KIND: SignalKind;
}
```

The trait MUST remain sealed so downstream crates cannot introduce new core signal kinds.

## 11. Logic levels

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LogicLevel {
    Low,
    High,
}
```

`LogicLevel` SHOULD provide:

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

The core signal API MUST NOT use `bool` where the value semantically denotes a signal level.

Conversions from and to `bool` MAY be provided as explicit conveniences.

## 12. Pulse counts

The initial public representation is a checked fixed-width unsigned count:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PulseCount(u64);
```

It MUST provide:

```rust
impl PulseCount {
    pub const ZERO: Self;
    pub const ONE: Self;

    pub const fn new(value: u64) -> Self;
    pub const fn get(self) -> u64;
    pub const fn is_zero(self) -> bool;
    pub const fn is_positive(self) -> bool;

    pub fn checked_add(self, other: Self)
        -> Result<Self, PulseCountOverflow>;
}
```

Ordinary arithmetic traits that can silently wrap MUST NOT be implemented.

The use of `u64` is part of the initial concrete API. A later incompatible change to arbitrary-precision counts requires an explicit semantic and persistence version change.

---

# Part IV — Exact logical time

## 13. Time-domain markers

A caller defines a zero-sized marker type:

```rust
#[derive(Debug, Clone, Copy)]
pub struct SimulationTicks;
```

The marker has no required trait implementation beyond those naturally induced by its use in `PhantomData`.

Different domain markers produce statically incompatible time values.

## 14. Time types

The initial public representation is:

```rust
#[repr(transparent)]
pub struct Time<D> {
    ticks: u64,
    marker: core::marker::PhantomData<fn() -> D>,
}

#[repr(transparent)]
pub struct Span<D> {
    ticks: u64,
    marker: core::marker::PhantomData<fn() -> D>,
}

#[repr(transparent)]
pub struct NonZeroSpan<D> {
    ticks: core::num::NonZeroU64,
    marker: core::marker::PhantomData<fn() -> D>,
}
```

These types MUST implement value semantics independent of `D`:

```text
Clone
Copy
Eq
Ord
Hash
Debug
```

Their trait implementations MUST NOT require corresponding traits on `D`.

## 15. Time constructors and accessors

```rust
impl<D> Time<D> {
    pub const fn from_ticks(ticks: u64) -> Self;
    pub const fn ticks(self) -> u64;

    pub fn checked_add(self, span: Span<D>)
        -> Result<Self, TimeArithmeticError>;

    pub fn checked_add_nonzero(self, span: NonZeroSpan<D>)
        -> Result<Self, TimeArithmeticError>;

    pub fn checked_duration_since(self, earlier: Self)
        -> Result<Span<D>, TimeArithmeticError>;
}

impl<D> Span<D> {
    pub const ZERO: Self;
    pub const fn from_ticks(ticks: u64) -> Self;
    pub const fn ticks(self) -> u64;
    pub const fn is_zero(self) -> bool;

    pub fn checked_add(self, other: Self)
        -> Result<Self, TimeArithmeticError>;

    pub fn try_nonzero(self)
        -> Result<NonZeroSpan<D>, ZeroSpanError>;
}

impl<D> NonZeroSpan<D> {
    pub fn from_ticks(ticks: u64)
        -> Result<Self, ZeroSpanError>;

    pub const fn ticks(self) -> u64;
    pub const fn get(self) -> core::num::NonZeroU64;
    pub const fn as_span(self) -> Span<D>;
}
```

Infallible `Add` or `Sub` implementations that could overflow or subtract in the wrong direction MUST NOT be provided.

---

# Part V — Stable identity

## 16. Stable keys

Canonical stable keys include:

```rust
pub struct NetworkKey(/* opaque */);
pub struct NodeKey(/* opaque */);
pub struct ConnectionKey(/* opaque */);
pub struct ModuleInstanceKey(/* opaque */);
pub struct ModuleInputKey<S>(/* opaque */);
pub struct ModuleOutputKey<S>(/* opaque */);
pub struct InPortKey<S>(/* opaque */);
pub struct OutPortKey<S>(/* opaque */);
pub struct ExternalInputKey<S>(/* opaque */);
pub struct ExternalOutputKey<S>(/* opaque */);
```

Every key MUST:

- be an owned copyable value;
- carry no borrow of a builder or network;
- preserve signal kind where applicable;
- have deterministic equality, hashing, and canonical ordering;
- expose no semantic meaning through numeric ordering;
- be constructible without global mutable state.

The initial key payload SHOULD be large enough to support caller-generated durable identities without collision-prone process-global allocation. A 128-bit opaque value is recommended.

## 17. Key construction

The API MUST support both local allocation and explicit durable construction.

Representative constructors are:

```rust
impl NodeKey {
    pub fn from_u128(value: u128) -> Self;
    pub fn as_u128(self) -> u128;
}
```

Equivalent constructors SHOULD exist for every stable key type.

A caller-scoped allocator MAY be provided:

```rust
pub struct KeyAllocator { /* opaque */ }

impl KeyAllocator {
    pub fn new(namespace: u64) -> Self;
    pub fn node(&mut self) -> NodeKey;
    pub fn connection(&mut self) -> ConnectionKey;
    pub fn module_instance(&mut self) -> ModuleInstanceKey;
}
```

The allocator MUST be an ordinary caller-owned value. The crate MUST NOT require a global allocator or hidden singleton counter.

Keys allocated implicitly by a fresh builder are convenient local identities. They are not guaranteed to match keys produced by a separately rebuilt definition after unrelated insertions or deletions. Callers that require persistence or state-preserving reconfiguration across independent rebuilds MUST supply or deliberately retain stable keys.

## 18. Erased stable keys

Heterogeneous tooling uses closed erased enums:

```rust
#[non_exhaustive]
pub enum AnyInPortKey {
    Level(InPortKey<Level>),
    Pulse(InPortKey<Pulse>),
}

#[non_exhaustive]
pub enum AnyOutPortKey {
    Level(OutPortKey<Level>),
    Pulse(OutPortKey<Pulse>),
}

#[non_exhaustive]
pub enum AnyExternalInputKey {
    Level(ExternalInputKey<Level>),
    Pulse(ExternalInputKey<Pulse>),
}

#[non_exhaustive]
pub enum AnyExternalOutputKey {
    Level(ExternalOutputKey<Level>),
    Pulse(ExternalOutputKey<Pulse>),
}

#[non_exhaustive]
pub enum AnyModuleInputKey {
    Level(ModuleInputKey<Level>),
    Pulse(ModuleInputKey<Pulse>),
}

#[non_exhaustive]
pub enum AnyModuleOutputKey {
    Level(ModuleOutputKey<Level>),
    Pulse(ModuleOutputKey<Pulse>),
}

#[non_exhaustive]
pub enum AnySignalSourceKey {
    Level(SignalSourceKey<Level>),
    Pulse(SignalSourceKey<Pulse>),
}
```

Typed keys and typed signal-source keys MUST provide explicit conversion into their erased forms.

Fallible conversion from erased to typed form MUST return a structured kind mismatch.

## 19. Subject references

Diagnostics, graph results, migration reports, and explanations use:

```rust
#[non_exhaustive]
pub enum SubjectRef {
    Network(NetworkKey),
    Module(ModuleInstanceKey),
    ModuleInput(AnyModuleInputKey),
    ModuleOutput(AnyModuleOutputKey),
    Node(NodeKey),
    InPort(AnyInPortKey),
    OutPort(AnyOutPortKey),
    Connection(ConnectionKey),
    ExternalInput(AnyExternalInputKey),
    ExternalOutput(AnyExternalOutputKey),
    PendingEvent(PendingEventKey),
    Revision(NetworkRevision),
    Snapshot(SnapshotDigest),
}
```

`SubjectRef` is semantic identity, not a rendered label.

---

# Part VI — Metadata and human-readable identity

## 20. Diagnostic metadata

The standard owned metadata type is:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagnosticMeta {
    pub name: Option<String>,
    pub description: Option<String>,
    pub path: Option<DiagnosticPath>,
    pub origin: Option<OriginRef>,
    pub tags: Vec<String>,
}
```

The initial public runtime types MUST NOT become generic over arbitrary metadata payloads.

Callers needing richer data SHOULD retain it in their own tables keyed by stable structural keys.

`OriginRef` SHOULD be a small standardized owned value suitable for source correlation, for example:

```rust
#[non_exhaustive]
pub enum OriginRef {
    Text(String),
    SourceLocation {
        source: String,
        line: u32,
        column: u32,
    },
}
```

Metadata MUST NOT affect execution or semantic fingerprints.

## 21. Metadata inputs

Builder APIs SHOULD accept borrowed or owned text through `impl Into<String>` or `DiagnosticMeta` values.

A string supplied by the caller SHOULD be allocated at most once when converted into owned metadata.

The API MUST NOT require `'static` strings.

---

# Part VII — Typed network authoring

## 22. `NetworkBuilder<D>`

```rust
pub struct NetworkBuilder<D> { /* opaque */ }
```

A builder owns:

- one network identity;
- a caller-local stable-key allocator;
- authored nodes, ports, endpoints, modules, and connections;
- builder-scoped signal handles;
- authoring diagnostics.

Constructors:

```rust
impl<D> NetworkBuilder<D> {
    pub fn new() -> Self;
    pub fn with_network_key(key: NetworkKey) -> Self;
    pub fn network_key(&self) -> NetworkKey;
}
```

`NetworkBuilder<D>` MUST NOT borrow external storage.

## 23. Builder-scoped signals

```rust
pub struct Signal<S> { /* opaque */ }
```

`Signal<S>` SHOULD implement `Clone` and `Copy`.

It carries:

- signal kind through `S`;
- an internal builder identity;
- a source reference inside that builder.

Passing a signal from another builder MUST return `AuthoringFailure::ForeignSignal` rather than panic or silently connect to an unrelated source.

The API intentionally does not use branded lifetimes or generative closure scopes for builder isolation.

A signal MAY expose its stable source identity:

```rust
pub enum SignalSourceKey<S> {
    ExternalInput(ExternalInputKey<S>),
    NodeOutput(OutPortKey<S>),
}

impl<S: SignalType> Signal<S> {
    pub fn source_key(self) -> SignalSourceKey<S>;
}
```

`Signal<S>` itself is builder-only and MUST NOT appear in snapshots, bindings, replay frames, or persisted definitions.

## 24. External input authoring

Convenience construction allocates stable identity locally:

```rust
impl<D> NetworkBuilder<D> {
    pub fn level_input(
        &mut self,
        name: impl Into<String>,
    ) -> (ExternalInputKey<Level>, Signal<Level>);

    pub fn pulse_input(
        &mut self,
        name: impl Into<String>,
    ) -> (ExternalInputKey<Pulse>, Signal<Pulse>);
}
```

Explicit durable identity uses metadata-bearing forms:

```rust
impl<D> NetworkBuilder<D> {
    pub fn add_level_input(
        &mut self,
        key: ExternalInputKey<Level>,
        meta: DiagnosticMeta,
    ) -> Result<Signal<Level>, AuthoringFailure>;

    pub fn add_pulse_input(
        &mut self,
        key: ExternalInputKey<Pulse>,
        meta: DiagnosticMeta,
    ) -> Result<Signal<Pulse>, AuthoringFailure>;
}
```

The explicit forms MUST reject duplicate keys immediately where determinable.

## 25. External output authoring

Convenience forms:

```rust
impl<D> NetworkBuilder<D> {
    pub fn level_output(
        &mut self,
        name: impl Into<String>,
        source: Signal<Level>,
    ) -> Result<ExternalOutputKey<Level>, AuthoringFailure>;

    pub fn pulse_output(
        &mut self,
        name: impl Into<String>,
        source: Signal<Pulse>,
    ) -> Result<ExternalOutputKey<Pulse>, AuthoringFailure>;
}
```

Explicit forms:

```rust
impl<D> NetworkBuilder<D> {
    pub fn add_level_output(
        &mut self,
        key: ExternalOutputKey<Level>,
        source: Signal<Level>,
        meta: DiagnosticMeta,
    ) -> Result<(), AuthoringFailure>;

    pub fn add_pulse_output(
        &mut self,
        key: ExternalOutputKey<Pulse>,
        source: Signal<Pulse>,
        meta: DiagnosticMeta,
    ) -> Result<(), AuthoringFailure>;
}
```

## 26. Added-node handles

Explicit node construction returns:

```rust
pub struct AddedNode<O> {
    key: NodeKey,
    outputs: O,
}

impl<O> AddedNode<O> {
    pub fn key(&self) -> NodeKey;
    pub fn outputs(&self) -> &O;
    pub fn into_outputs(self) -> O;
    pub fn into_parts(self) -> (NodeKey, O);
}
```

For a one-output node, `O` MAY be `Signal<S>`.

For multiple outputs, node-specific output structs are used:

```rust
pub struct PulseRouteOutputs {
    pub when_low: Signal<Pulse>,
    pub when_high: Signal<Pulse>,
}
```

Ordinary expression-oriented convenience methods MAY return only their signal output. Every primitive MUST also have an explicit form that exposes its `NodeKey`.

## 27. Primitive construction style

Simple nodes SHOULD use direct arguments:

```rust
impl<D> NetworkBuilder<D> {
    pub fn not(
        &mut self,
        input: Signal<Level>,
    ) -> Result<Signal<Level>, AuthoringFailure>;

    pub fn select(
        &mut self,
        selector: Signal<Level>,
        when_low: Signal<Level>,
        when_high: Signal<Level>,
    ) -> Result<Signal<Level>, AuthoringFailure>;

    pub fn coalesce(
        &mut self,
        input: Signal<Pulse>,
    ) -> Result<Signal<Pulse>, AuthoringFailure>;
}
```

Variadic nodes accept `IntoIterator`:

```rust
impl<D> NetworkBuilder<D> {
    pub fn all<I>(
        &mut self,
        inputs: I,
    ) -> Result<Signal<Level>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Level>>;

    pub fn merge<I>(
        &mut self,
        inputs: I,
    ) -> Result<Signal<Pulse>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Pulse>>;
}
```

The iterator is consumed immediately. The returned network MUST NOT borrow it.

Policy-heavy nodes use explicit configuration value types rather than long positional argument lists. The complete initial configuration family is defined below.

## 28. Explicit keyed primitive construction

Every primitive MUST have an explicit keyed form broadly equivalent to:

```rust
impl<D> NetworkBuilder<D> {
    pub fn add_toggle(
        &mut self,
        key: NodeKey,
        input: Signal<Pulse>,
        config: ToggleConfig,
        meta: DiagnosticMeta,
    ) -> Result<AddedNode<Signal<Level>>, AuthoringFailure>;
}
```

Port keys for fixed-shape nodes MAY be derived deterministically from the node key or allocated as part of the node definition, provided they remain stable and inspectable.

For imported or explicitly authored definitions, callers MUST be able to supply exact port keys.

## 29. Built-in configuration values

The initial public configuration types are:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConstantConfig {
    pub value: LogicLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtLeastConfig {
    pub threshold: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeInitialization {
    Baseline,
    Assume(LogicLevel),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeConfig {
    pub initialization: EdgeInitialization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConflictPolicy {
    SetDominant,
    ResetDominant,
    RetainAndDiagnose,
    RejectTransaction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToggleConfig {
    pub initial: LogicLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PulseSetResetConfig {
    pub initial: LogicLevel,
    pub conflict: ConflictPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LevelSetResetConfig {
    pub initial: LogicLevel,
    pub conflict: ConflictPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SampleHoldConfig {
    pub initial: LogicLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PulseDelayConfig<D> {
    pub delay: NonZeroSpan<D>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransportDelayConfig<D> {
    pub delay: NonZeroSpan<D>,
    pub initial: LogicLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InertialDelayConfig<D> {
    pub delay: NonZeroSpan<D>,
    pub initial: LogicLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FirstEmissionPolicy {
    Immediate,
    AfterFirstPeriod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReenablePhasePolicy {
    RestartPhase,
    PreservePhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PeriodicConfig<D> {
    pub period: NonZeroSpan<D>,
    pub first_emission: FirstEmissionPolicy,
    pub reenable_phase: ReenablePhasePolicy,
}
```

Configuration structs SHOULD remain small value types. They MUST contain semantic policy only, not diagnostic metadata or runtime state.

## 30. Complete primitive constructor family

The typed builder MUST expose the complete initial primitive family with names broadly equivalent to:

```rust
impl<D> NetworkBuilder<D> {
    // Level combinational
    pub fn constant(
        &mut self,
        value: LogicLevel,
    ) -> Signal<Level>;

    pub fn low(&mut self) -> Signal<Level>;
    pub fn high(&mut self) -> Signal<Level>;

    pub fn not(
        &mut self,
        input: Signal<Level>,
    ) -> Result<Signal<Level>, AuthoringFailure>;

    pub fn all<I>(
        &mut self,
        inputs: I,
    ) -> Result<Signal<Level>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Level>>;

    pub fn any<I>(
        &mut self,
        inputs: I,
    ) -> Result<Signal<Level>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Level>>;

    pub fn parity<I>(
        &mut self,
        inputs: I,
    ) -> Result<Signal<Level>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Level>>;

    pub fn at_least<I>(
        &mut self,
        threshold: u64,
        inputs: I,
    ) -> Result<Signal<Level>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Level>>;

    pub fn select(
        &mut self,
        selector: Signal<Level>,
        when_low: Signal<Level>,
        when_high: Signal<Level>,
    ) -> Result<Signal<Level>, AuthoringFailure>;

    // Pulse combinational
    pub fn merge<I>(
        &mut self,
        inputs: I,
    ) -> Result<Signal<Pulse>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Pulse>>;

    pub fn coalesce(
        &mut self,
        input: Signal<Pulse>,
    ) -> Result<Signal<Pulse>, AuthoringFailure>;

    pub fn zip<I>(
        &mut self,
        inputs: I,
    ) -> Result<Signal<Pulse>, AuthoringFailure>
    where
        I: IntoIterator<Item = Signal<Pulse>>;

    // Level-controlled pulse flow
    pub fn pulse_gate(
        &mut self,
        pulses: Signal<Pulse>,
        enable: Signal<Level>,
    ) -> Result<Signal<Pulse>, AuthoringFailure>;

    pub fn pulse_select(
        &mut self,
        selector: Signal<Level>,
        when_low: Signal<Pulse>,
        when_high: Signal<Pulse>,
    ) -> Result<Signal<Pulse>, AuthoringFailure>;

    pub fn pulse_route(
        &mut self,
        selector: Signal<Level>,
        pulses: Signal<Pulse>,
    ) -> Result<PulseRouteOutputs, AuthoringFailure>;

    // Transition-sensitive and stateful
    pub fn rising_edge(
        &mut self,
        input: Signal<Level>,
        config: EdgeConfig,
    ) -> Result<Signal<Pulse>, AuthoringFailure>;

    pub fn falling_edge(
        &mut self,
        input: Signal<Level>,
        config: EdgeConfig,
    ) -> Result<Signal<Pulse>, AuthoringFailure>;

    pub fn any_edge(
        &mut self,
        input: Signal<Level>,
        config: EdgeConfig,
    ) -> Result<Signal<Pulse>, AuthoringFailure>;

    pub fn toggle(
        &mut self,
        input: Signal<Pulse>,
        config: ToggleConfig,
    ) -> Result<Signal<Level>, AuthoringFailure>;

    pub fn pulse_set_reset_latch(
        &mut self,
        set: Signal<Pulse>,
        reset: Signal<Pulse>,
        config: PulseSetResetConfig,
    ) -> Result<Signal<Level>, AuthoringFailure>;

    pub fn level_set_reset_latch(
        &mut self,
        set: Signal<Level>,
        reset: Signal<Level>,
        config: LevelSetResetConfig,
    ) -> Result<Signal<Level>, AuthoringFailure>;

    pub fn sample_hold(
        &mut self,
        value: Signal<Level>,
        sample: Signal<Pulse>,
        config: SampleHoldConfig,
    ) -> Result<Signal<Level>, AuthoringFailure>;

    // Temporal
    pub fn pulse_delay(
        &mut self,
        input: Signal<Pulse>,
        config: PulseDelayConfig<D>,
    ) -> Result<Signal<Pulse>, AuthoringFailure>;

    pub fn transport_delay(
        &mut self,
        input: Signal<Level>,
        config: TransportDelayConfig<D>,
    ) -> Result<Signal<Level>, AuthoringFailure>;

    pub fn inertial_delay(
        &mut self,
        input: Signal<Level>,
        config: InertialDelayConfig<D>,
    ) -> Result<Signal<Level>, AuthoringFailure>;

    pub fn periodic(
        &mut self,
        enable: Signal<Level>,
        config: PeriodicConfig<D>,
    ) -> Result<Signal<Pulse>, AuthoringFailure>;
}
```

`constant`, `low`, and `high` are infallible because they consume no foreign signal handles. Each call creates one distinct `Constant` primitive with its own stable identity; the builder MUST NOT silently merge separately authored constants. All constructors accepting signals return `Result` so cross-builder misuse remains structured.

For every method above, an explicit keyed form named `add_<method>` MUST exist. It receives `NodeKey`, semantic inputs, semantic configuration where applicable, and `DiagnosticMeta`, and returns `AddedNode<O>`.

For variadic explicit forms, callers MAY additionally provide stable variadic input-port keys. If omitted, the builder allocates them locally in iterator order. Durable reconfiguration that depends on preserving individual variadic ports SHOULD supply or retain those keys explicitly.

## 31. Convenience aliases

A primitive alias such as `xor` or `debounce` MAY have a concise method:

```rust
pub fn xor(
    &mut self,
    a: Signal<Level>,
    b: Signal<Level>,
) -> Result<Signal<Level>, AuthoringFailure>;
```

The resulting canonical node kind remains `Parity` or `InertialDelay` respectively.

Named standard-module conveniences belong to the future standard-module catalogue and MUST remain distinguishable from primitive aliases.

## 32. Signal and port metadata

Intermediate signals may be named or annotated without inserting a semantic identity node.

```rust
impl<D> NetworkBuilder<D> {
    pub fn annotate_signal<S: SignalType>(
        &mut self,
        signal: Signal<S>,
        meta: DiagnosticMeta,
    ) -> Result<Signal<S>, AuthoringFailure>;
}
```

The method updates metadata associated with the signal source and returns the same semantic signal handle.

It MUST NOT create a new node, connection, signal value, causality barrier, fingerprint contribution, or runtime state merely to carry a name.

Where one source is exposed through several differently named contexts, metadata SHOULD be attached to the relevant port, endpoint, module export, or graph-view alias rather than represented as a signal-processing node.

## 33. Reusable module definitions

Reusable modules use the typed stable `ModuleInputKey<S>` and `ModuleOutputKey<S>` interface keys defined with the other structural identities. Their primary module artifacts are:

```rust
pub struct UncheckedModule<D> { /* owned dynamic module definition */ }
pub struct ModuleDef<D> { /* validated immutable module definition */ }
pub struct ModuleBuilder<D> { /* owned typed authoring builder */ }
pub struct ModuleFingerprint(/* opaque fixed digest */);
```

A module builder follows the same typed signal rules as `NetworkBuilder<D>` but exposes a module interface instead of application-level external endpoints:

```rust
impl<D> ModuleBuilder<D> {
    pub fn new() -> Self;

    pub fn level_input(
        &mut self,
        name: impl Into<String>,
    ) -> (ModuleInputKey<Level>, Signal<Level>);

    pub fn pulse_input(
        &mut self,
        name: impl Into<String>,
    ) -> (ModuleInputKey<Pulse>, Signal<Pulse>);

    pub fn level_output(
        &mut self,
        name: impl Into<String>,
        source: Signal<Level>,
    ) -> Result<ModuleOutputKey<Level>, AuthoringFailure>;

    pub fn pulse_output(
        &mut self,
        name: impl Into<String>,
        source: Signal<Pulse>,
    ) -> Result<ModuleOutputKey<Pulse>, AuthoringFailure>;

    pub fn finish(self) -> Report<ModuleDef<D>, D>;
    pub fn into_unchecked(self) -> UncheckedModule<D>;
}

impl<D> UncheckedModule<D> {
    pub fn validate(self) -> Report<ModuleDef<D>, D>;
    pub fn validate_ref(&self) -> Report<ModuleDef<D>, D>;
}
```

The module builder MUST support the same built-in primitive constructor family and explicit internal stable keys as the network builder.

`ModuleDef<D>` SHOULD be cheaply cloneable through immutable shared ownership and MUST expose:

```rust
impl<D> ModuleDef<D> {
    pub fn fingerprint(&self) -> ModuleFingerprint;
    pub fn inputs(&self) -> ModuleInputIter<'_, D>;
    pub fn outputs(&self) -> ModuleOutputIter<'_, D>;
    pub fn graph(&self) -> DefinitionGraphView<'_, D>;
}
```

## 34. Module instantiation

A module instance is identified by `ModuleInstanceKey` and binds module inputs to builder signals.

```rust
pub struct ModuleInstanceBuilder<'a, D> { /* borrows network builder */ }
pub struct AddedModuleInstance { /* builder-scoped instance outputs */ }
```

Construction:

```rust
impl<D> NetworkBuilder<D> {
    pub fn instantiate<'a>(
        &'a mut self,
        module: &'a ModuleDef<D>,
        key: ModuleInstanceKey,
        meta: DiagnosticMeta,
    ) -> Result<ModuleInstanceBuilder<'a, D>, AuthoringFailure>;
}
```

Input binding and completion:

```rust
impl<'a, D> ModuleInstanceBuilder<'a, D> {
    pub fn bind_level(
        self,
        input: ModuleInputKey<Level>,
        source: Signal<Level>,
    ) -> Result<Self, AuthoringFailure>;

    pub fn bind_pulse(
        self,
        input: ModuleInputKey<Pulse>,
        source: Signal<Pulse>,
    ) -> Result<Self, AuthoringFailure>;

    pub fn finish(self)
        -> Result<AddedModuleInstance, AuthoringFailure>;
}

impl AddedModuleInstance {
    pub fn key(&self) -> ModuleInstanceKey;

    pub fn level_output(
        &self,
        output: ModuleOutputKey<Level>,
    ) -> Result<Signal<Level>, AuthoringFailure>;

    pub fn pulse_output(
        &self,
        output: ModuleOutputKey<Pulse>,
    ) -> Result<Signal<Pulse>, AuthoringFailure>;
}
```

`finish` MUST require every module input exactly once and MUST reject unknown, duplicate, wrong-kind, or foreign-builder bindings.

Module internals retain stable identity logically as:

```text
(ModuleInstanceKey, module-internal stable key)
```

The compiled network MAY derive network-global private or public structural keys from that pair, but compatible module revisions MUST preserve identity according to the pair rather than insertion order or dense position.

Nested module instantiation is permitted. The module mechanism MUST preserve hierarchy for inspection, diagnostics, explanation, and reconfiguration even if execution is internally flattened.

## 35. Builder completion

```rust
impl<D> NetworkBuilder<D> {
    pub fn finish(self) -> Report<ValidatedNetwork<D>, D>;
    pub fn into_unchecked(self) -> UncheckedNetwork<D>;
}
```

`finish` consumes the builder, materializes the canonical dynamic definition, runs complete validation, and returns a report.

A builder MUST NOT be reusable after completion.

---

# Part VIII — Dynamic network definitions

## 36. `UncheckedNetwork<D>`

```rust
pub struct UncheckedNetwork<D> { /* owned dynamic definition */ }
```

It is the canonical public form for:

- deserialized network definitions;
- editor-authored definitions;
- generated definitions;
- explicit low-level network construction;
- fuzzing malformed structures.

It MUST be possible to construct an invalid `UncheckedNetwork<D>` without unsafe code.

## 37. Dynamic node definitions

The closed node definition surface SHOULD be equivalent to:

```rust
#[non_exhaustive]
pub struct NodeDef<D> {
    pub key: NodeKey,
    pub kind: NodeKind<D>,
    pub ports: NodePorts,
    pub meta: DiagnosticMeta,
    pub module: Option<ModuleInstanceKey>,
}

#[non_exhaustive]
pub enum NodeKind<D> {
    Constant(ConstantConfig),
    Not,
    All,
    Any,
    Parity,
    AtLeast(AtLeastConfig),
    Select,
    Merge,
    Coalesce,
    Zip,
    PulseGate,
    PulseSelect,
    PulseRoute,
    RisingEdge(EdgeConfig),
    FallingEdge(EdgeConfig),
    AnyEdge(EdgeConfig),
    Toggle(ToggleConfig),
    PulseSetResetLatch(PulseSetResetConfig),
    LevelSetResetLatch(LevelSetResetConfig),
    SampleHold(SampleHoldConfig),
    PulseDelay(PulseDelayConfig<D>),
    TransportDelay(TransportDelayConfig<D>),
    InertialDelay(InertialDelayConfig<D>),
    Periodic(PeriodicConfig<D>),
}
```

`NodeKind<D>` MUST remain closed and non-callback-based.

## 38. Dynamic ports and connections

Dynamic definitions use stable typed keys where the kind is known and erased forms where heterogeneous storage is necessary.

Representative forms:

```rust
pub struct ConnectionDef {
    pub key: ConnectionKey,
    pub from: AnySignalSourceKey,
    pub to: AnyInPortKey,
    pub meta: DiagnosticMeta,
}

pub struct ExternalInputDef {
    pub key: AnyExternalInputKey,
    pub meta: DiagnosticMeta,
}

pub struct ExternalOutputDef {
    pub key: AnyExternalOutputKey,
    pub source: AnySignalSourceKey,
    pub meta: DiagnosticMeta,
}
```

The dynamic representation MAY contain kind mismatches and dangling references. Validation is responsible for rejecting them.

## 39. Validation

```rust
impl<D> UncheckedNetwork<D> {
    pub fn validate(self) -> Report<ValidatedNetwork<D>, D>;
    pub fn validate_ref(&self) -> Report<ValidatedNetwork<D>, D>;
}
```

The consuming form SHOULD be preferred when the caller no longer needs the unchecked definition.

Validation MUST collect independent diagnostics where safe and MUST omit the artifact when blocking diagnostics remain.

---

# Part IX — Reports and diagnostics

## 40. `Report<T, D>`

```rust
pub struct Report<T, D> {
    artifact: Option<T>,
    diagnostics: DiagnosticSet<D>,
}
```

Required methods:

```rust
impl<T, D> Report<T, D> {
    pub fn artifact(&self) -> Option<&T>;
    pub fn diagnostics(&self) -> &DiagnosticSet<D>;
    pub fn has_errors(&self) -> bool;
    pub fn has_warnings(&self) -> bool;

    pub fn into_parts(self) -> (Option<T>, DiagnosticSet<D>);

    pub fn require_artifact(self)
        -> Result<T, ReportFailure<D>>;
}
```

`ReportFailure<D>` MUST retain the complete diagnostic set. Calling `require_artifact` introduces no new problem code merely because the caller selected the artifact-requiring convenience.

## 41. Unified problem records and report findings

The common owned problem record is:

```rust
pub struct Problem<D> { /* private owned fields */ }

pub enum Severity {
    Info,
    Warning,
    Error,
}

pub enum Responsibility {
    Advisory,
    CallerInput,
    SemanticRejection,
    Compatibility,
    ResourceLimit,
    CorruptData,
    UnsupportedFeature,
    ExternalIntegration,
    LibraryDefect,
}

#[non_exhaustive]
pub enum ProblemEvidence<D> {
    /* exactly one code-specific variant per catalogue entry */
}
```

`Problem<D>` MUST expose accessors broadly equivalent to:

```rust
impl<D> Problem<D> {
    pub fn code(&self) -> DiagnosticCode;
    pub fn severity(&self) -> Severity;
    pub fn responsibility(&self) -> Responsibility;
    pub fn primary(&self) -> &SubjectRef;
    pub fn related(&self) -> &[RelatedSubject];
    pub fn evidence(&self) -> &ProblemEvidence<D>;
    pub fn suggestions(&self) -> &[Suggestion];
}
```

Its fields SHOULD remain private so arbitrary code-to-evidence, severity, responsibility, and delivery combinations cannot be constructed accidentally. A checked dynamic decoder MAY construct a problem only after validating the combination against the diagnostic catalogue.

`DiagnosticCode` MUST be a catalogue-backed structured identifier, not an arbitrary string selected at emission time. Its compatibility status follows the diagnostic schema: codes may change through intentional schema revision while the schema is experimental, and acquire permanence only after an explicit future freeze.

Severity and responsibility are fixed by the code. Emitters MUST NOT choose them dynamically.

`ProblemEvidence<D>` is a closed typed family. Each catalogue code has one exact code-specific variant, even when several variants wrap the same reusable payload record. An untyped map such as `HashMap<String, String>` MUST NOT be the canonical evidence representation.

A report finding is a delivery-specific wrapper:

```rust
pub struct Diagnostic<D> {
    problem: Problem<D>,
}

impl<D> Diagnostic<D> {
    pub fn problem(&self) -> &Problem<D>;
    pub fn into_problem(self) -> Problem<D>;
}
```

`Diagnostic<D>` may contain only codes whose catalogue entry permits `ReportFinding` delivery.

Rendered summaries, localized messages, and `Display` output are derived presentation. They MUST NOT be stored as authoritative semantic fields and MAY evolve without changing the code or evidence.

Transient successful-runtime findings use a distinct owned delivery form:

```rust
pub struct DiagnosticOccurrence<D> {
    problem: Problem<D>,
    at: Time<D>,
    revision: NetworkRevision,
}

impl<D> DiagnosticOccurrence<D> {
    pub fn problem(&self) -> &Problem<D>;
    pub fn at(&self) -> Time<D>;
    pub fn revision(&self) -> NetworkRevision;
}
```

A `DiagnosticOccurrence<D>` may contain only codes whose catalogue entry permits `RuntimeOccurrence` delivery.

## 42. Diagnostic collections

```rust
pub struct DiagnosticSet<D> { /* deterministic owned collection */ }
```

It SHOULD provide iteration over `&Diagnostic<D>` in the catalogue's canonical order, filtering by severity or code, and ordinary collection accessors such as `len` and `is_empty`.

One set contains at most one finding for each catalogue-defined condition key. Repeated detection MUST merge according to the code-defined evidence merge law rather than whichever validator ran first.

Callers MUST NOT need to parse rendered prose to identify, order, deduplicate, or handle known conditions.

---

# Part X — Validation and compilation artifacts

## 43. `ValidatedNetwork<D>`

```rust
pub struct ValidatedNetwork<D> { /* opaque */ }
```

It MUST be constructible only by successful validation or an internal trusted test path.

It SHOULD expose structural read-only access:

```rust
impl<D> ValidatedNetwork<D> {
    pub fn network_key(&self) -> NetworkKey;
    pub fn graph(&self) -> DefinitionGraphView<'_, D>;
    pub fn into_unchecked(self) -> UncheckedNetwork<D>;
    pub fn compile(self) -> Report<CompiledNetwork<D>, D>;
    pub fn compile_ref(&self) -> Report<CompiledNetwork<D>, D>;
}
```

The consuming compile form SHOULD be preferred when practical.

## 44. `CompiledNetwork<D>`

```rust
pub struct CompiledNetwork<D> { /* cheap immutable shared handle */ }
```

It SHOULD implement cheap `Clone` through hidden immutable shared ownership.

The public API MUST NOT expose internal dense indices, adjacency storage, arenas, or worklists.

Required accessors include:

```rust
impl<D> CompiledNetwork<D> {
    pub fn network_key(&self) -> NetworkKey;
    pub fn fingerprint(&self) -> NetworkFingerprint;
    pub fn graph(&self) -> GraphView<'_, D>;

    pub fn spawn(&self, policy: RuntimePolicy) -> Machine<D>;

    pub fn input_snapshot(&self) -> InputSnapshotBuilder<D>;
    pub fn input_delta(&self) -> InputDeltaBuilder<D>;

    pub fn patch(
        &self,
        base_revision: NetworkRevision,
    ) -> NetworkPatchBuilder<D>;

    pub fn prepare_patch(
        &self,
        patch: NetworkPatch<D>,
    ) -> Report<PreparedPatch<D>, D>;

    pub fn restore(
        &self,
        snapshot: MachineSnapshot<D>,
        policy: RuntimePolicy,
    ) -> Result<Machine<D>, RestoreFailure>;
}
```

`spawn` MUST require an explicit validated `RuntimePolicy`.

## 45. Revision and fingerprints

```rust
pub struct NetworkRevision(/* opaque */);
pub struct NetworkFingerprint(/* opaque fixed digest */);
pub struct ExecutionStateDigest(/* opaque fixed digest */);
pub struct ObservableStateDigest(/* opaque fixed digest */);
pub struct SnapshotDigest(/* opaque fixed digest */);
pub struct RuntimePolicyId(/* opaque fixed digest */);
```

These types MUST be distinct and non-interchangeable.

They SHOULD provide:

```text
Clone
Copy
Eq
Hash
Ord
Debug
Display using a stable hexadecimal representation
```

The public API MUST NOT expose the selected digest algorithm as part of ordinary semantic use.

---

# Part XI — Resolved handles

## 46. Revision-bound resolution

Stable keys remain the canonical public identity. Repeated runtime access MAY use:

```rust
pub struct ResolvedNode { /* opaque */ }
pub struct ResolvedInput<S> { /* opaque */ }
pub struct ResolvedOutput<S> { /* opaque */ }
pub struct ResolvedInPort<S> { /* opaque */ }
pub struct ResolvedOutPort<S> { /* opaque */ }
```

Each handle is bound to:

- network fingerprint;
- topology revision;
- stable key;
- private dense access data.

## 47. Resolution APIs

```rust
impl<D> Machine<D> {
    pub fn resolve_node(
        &self,
        key: NodeKey,
    ) -> Result<ResolvedNode, ResolveFailure>;

    pub fn resolve_output<S: SignalType>(
        &self,
        key: ExternalOutputKey<S>,
    ) -> Result<ResolvedOutput<S>, ResolveFailure>;
}
```

Resolution belongs to `Machine<D>` because topology revision is machine-local. The returned handle captures the machine's current fingerprint and revision.

A stale or foreign handle MUST fail structurally when used against a machine.

A handle MUST NOT silently retarget after topology revision change, even if the same private dense slot is reused.

---

# Part XII — Runtime policy

## 48. `RuntimePolicy`

```rust
pub struct RuntimePolicy { /* validated immutable policy */ }
```

Construction uses a builder:

```rust
pub struct RuntimePolicyBuilder { /* opaque */ }

impl RuntimePolicy {
    pub fn builder() -> RuntimePolicyBuilder;
    pub fn id(&self) -> RuntimePolicyId;
}

impl RuntimePolicyBuilder {
    pub fn max_internal_reactions(self, value: u64) -> Self;
    pub fn max_evaluated_operations(self, value: u64) -> Self;
    pub fn max_pending_events(self, value: u64) -> Self;
    pub fn max_events_created_per_transaction(self, value: u64) -> Self;
    pub fn max_required_provenance_growth(self, value: u64) -> Self;

    pub fn build(self) -> Result<RuntimePolicy, PolicyFailure>;
}
```

The plain builder MUST require every semantically relevant limit to be set or must reject `build` with a missing-field failure. Named constructors such as `RuntimePolicy::conservative()` or `RuntimePolicy::development()` MAY provide explicit documented bundles, but the resulting exact policy and `RuntimePolicyId` MUST remain inspectable.

There MUST NOT be an invisible process-global runtime policy.

---

# Part XIII — Machine lifecycle and access

## 49. `Machine<D>`

```rust
pub struct Machine<D> { /* opaque mutable semantic machine */ }
```

The canonical machine uses runtime lifecycle state rather than a typestate parameter.

This permits:

- snapshots of either lifecycle state;
- restoration of either lifecycle state;
- homogeneous containers;
- dynamic tools;
- replay and failure testing;
- explicit structured lifecycle failures.

## 50. Machine status

```rust
#[non_exhaustive]
pub enum MachineStatus<D> {
    AwaitingInitialization,
    Ready { now: Time<D> },
}
```

Required accessors:

```rust
impl<D> Machine<D> {
    pub fn status(&self) -> MachineStatus<D>;
    pub fn is_initialized(&self) -> bool;
    pub fn now(&self) -> Option<Time<D>>;

    pub fn revision(&self) -> NetworkRevision;
    pub fn fingerprint(&self) -> NetworkFingerprint;
    pub fn compiled(&self) -> &CompiledNetwork<D>;

    pub fn runtime_policy(&self) -> &RuntimePolicy;
    pub fn runtime_policy_id(&self) -> RuntimePolicyId;

    pub fn execution_state_digest(&self) -> ExecutionStateDigest;
    pub fn observable_state_digest(&self) -> ObservableStateDigest;
}
```

## 51. Schedule access

```rust
#[non_exhaustive]
pub enum Schedule<D> {
    Dormant,
    WakeAt(Time<D>),
}

impl<D> Machine<D> {
    pub fn schedule(&self)
        -> Result<Schedule<D>, LifecycleFailure>;

    pub fn next_deadline(&self)
        -> Result<Option<Time<D>>, LifecycleFailure>;
}
```

An uninitialized machine MUST return a not-initialized failure rather than `Dormant`.

## 52. No lifecycle typestate requirement

The canonical API MUST NOT require:

```rust
Machine<D, AwaitingInitialization>
Machine<D, Ready>
```

Optional borrowed wrappers MAY be added later:

```rust
pub struct ReadyMachineRef<'a, D> { /* read-only ready view */ }
```

Such wrappers MUST remain convenience views over the same canonical `Machine<D>`.

---

# Part XIV — Input schemas and values

## 53. Network-bound input artifacts

`InputSnapshot<D>` and `InputDelta<D>` are owned, opaque, and bound to an exact expected input schema.

The schema identity includes enough information to distinguish:

- the current compiled topology;
- a prepared patch’s target topology;
- input kind and stable identity;
- whether a level input is newly introduced and requires establishment.

These artifacts MUST NOT borrow a compiled network or prepared patch.

## 54. `InputSnapshot<D>`

```rust
pub struct InputSnapshot<D> { /* opaque */ }
pub struct InputSnapshotBuilder<D> { /* opaque */ }
```

Representative API:

```rust
impl<D> InputSnapshotBuilder<D> {
    pub fn set(
        self,
        input: ExternalInputKey<Level>,
        value: LogicLevel,
    ) -> Result<Self, InputBuildFailure>;

    pub fn pulse(
        self,
        input: ExternalInputKey<Pulse>,
        count: PulseCount,
    ) -> Result<Self, InputBuildFailure>;

    pub fn finish(self)
        -> Result<InputSnapshot<D>, InputBuildFailure>;
}
```

The builder MUST diagnose duplicate, unknown, wrong-kind, foreign-schema, and missing required level observations.

Absent pulse input means zero.

## 55. `InputDelta<D>`

```rust
pub struct InputDelta<D> { /* opaque */ }
pub struct InputDeltaBuilder<D> { /* opaque */ }
```

Representative API:

```rust
impl<D> InputDeltaBuilder<D> {
    pub fn set(
        self,
        input: ExternalInputKey<Level>,
        value: LogicLevel,
    ) -> Result<Self, InputBuildFailure>;

    pub fn establish(
        self,
        input: ExternalInputKey<Level>,
        value: LogicLevel,
    ) -> Result<Self, InputBuildFailure>;

    pub fn pulse(
        self,
        input: ExternalInputKey<Pulse>,
        count: PulseCount,
    ) -> Result<Self, InputBuildFailure>;

    pub fn finish(self)
        -> Result<InputDelta<D>, InputBuildFailure>;
}
```

For an ordinary current-topology delta:

- `set` changes or reasserts existing external levels;
- `establish` is invalid because no input is new.

For a prepared-patch target delta:

- `set` applies to preserved target inputs;
- `establish` applies exactly to newly introduced target level inputs;
- every newly introduced required level input MUST be established before `finish` succeeds;
- removed old inputs are not part of the target schema and MUST be rejected if referenced.

This avoids requiring a complete replacement snapshot merely because a patch adds one external level input.

## 56. Prepared-patch input builders

```rust
impl<D> PreparedPatch<D> {
    pub fn input_snapshot(&self) -> InputSnapshotBuilder<D>;
    pub fn input_delta(&self) -> InputDeltaBuilder<D>;
}
```

The snapshot builder targets the resulting topology and is used when the patch participates in machine initialization.

The delta builder targets the resulting topology and carries required establishment obligations for newly added external level inputs.

---

# Part XV — Transactions

## 57. Owned explicit transaction values

```rust
pub struct Transaction<D> { /* opaque owned value */ }
```

A transaction is representable as data. It MUST NOT primarily be a closure that mutates hidden transaction state.

## 58. Distinct constructors

```rust
impl<D> Transaction<D> {
    pub fn initialize(
        at: Time<D>,
        expected_revision: NetworkRevision,
        input: InputSnapshot<D>,
    ) -> Self;

    pub fn advance(
        at: Time<D>,
        expected_revision: NetworkRevision,
        input: InputDelta<D>,
    ) -> Self;
}
```

These constructors make the ordinary lifecycle distinction visible without splitting the machine into typestates.

A caller may still construct a lifecycle-incompatible transaction and receive a structured runtime failure when applying it to the wrong machine state.

## 59. Transaction options

```rust
impl<D> Transaction<D> {
    pub fn expect_execution_state(
        self,
        digest: ExecutionStateDigest,
    ) -> Self;

    pub fn with_patch(
        self,
        prepared: PreparedPatch<D>,
        policy: ReconfigurationPolicy,
    ) -> Result<Self, TransactionBuildFailure>;

    pub fn with_meta(
        self,
        meta: TransactionMeta,
    ) -> Self;

    pub fn requested_time(&self) -> Time<D>;
    pub fn expected_revision(&self) -> NetworkRevision;
}
```

When a patch is attached:

- the transaction input MUST be bound to the prepared patch’s target input schema;
- the prepared patch MUST be bound to the transaction’s expected base revision;
- a ready-machine patch transaction uses a target-bound `InputDelta`;
- an initialization patch transaction uses a target-bound `InputSnapshot`.

`with_patch` MUST reject schema mismatch before runtime application where determinable.

## 60. Transaction metadata

Transaction metadata MUST remain non-semantic:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransactionMeta {
    pub label: Option<String>,
    pub correlation: Option<String>,
}
```

It MAY appear in diagnostics, provenance roots, and replay artifacts but MUST NOT affect evaluator behavior.

The core transaction type MUST NOT be generic over arbitrary caller metadata.

---

# Part XVI — Apply and forecast

## 61. Applying transactions

```rust
impl<D> Machine<D> {
    pub fn apply(
        &mut self,
        transaction: Transaction<D>,
    ) -> Result<TransactionResult<D>, RuntimeFailure>;
}
```

On success:

- the complete candidate machine becomes current;
- one immutable result is returned.

On structured failure:

- the transaction is consumed;
- the published machine remains semantically unchanged;
- a structured `RuntimeFailure` is returned.

The API MUST NOT expose partial reaction results during application.

## 62. Forecasting

```rust
impl<D> Machine<D> {
    pub fn forecast(
        &self,
        transaction: Transaction<D>,
    ) -> Result<ForecastResult<D>, RuntimeFailure>;
}
```

A forecast consumes its transaction and leaves the original machine unchanged.

## 63. Forecast result and hypothetical state

```rust
pub struct ForecastResult<D> {
    result: TransactionResult<D>,
    state: ForecastState<D>,
    basis: ForecastBasis<D>,
}

pub struct ForecastState<D> { /* read-only unpublished candidate */ }

pub struct ForecastBasis<D> {
    pub revision: NetworkRevision,
    pub execution_digest: ExecutionStateDigest,
    pub requested_time: Time<D>,
    pub runtime_policy_id: RuntimePolicyId,
}
```

Required accessors:

```rust
impl<D> ForecastResult<D> {
    pub fn result(&self) -> &TransactionResult<D>;
    pub fn state(&self) -> &ForecastState<D>;
    pub fn basis(&self) -> &ForecastBasis<D>;
    pub fn into_parts(self)
        -> (TransactionResult<D>, ForecastState<D>, ForecastBasis<D>);
}
```

`ForecastState<D>` SHOULD support the same read-only inspection, explanation, schedule, digest, and snapshot projections as a committed machine.

It MUST NOT provide `apply`, mutation, or implicit commit.

The caller may explicitly persist or restore a forecast snapshot through ordinary APIs, but that is a new explicit operation rather than conversion of the forecast into committed machine state.

---

# Part XVII — Transaction results and changes

## 64. `TransactionResult<D>`

```rust
pub struct TransactionResult<D> {
    pub requested_time: Time<D>,
    pub before_revision: NetworkRevision,
    pub after_revision: NetworkRevision,
    pub changes: SemanticChangeSet<D>,
    pub migration: Option<MigrationReport<D>>,
    pub occurrences: Vec<DiagnosticOccurrence<D>>,
    pub schedule: Schedule<D>,
    pub before_execution_digest: ExecutionStateDigest,
    pub after_execution_digest: ExecutionStateDigest,
    pub after_observable_digest: ObservableStateDigest,
    pub runtime_policy_id: RuntimePolicyId,
    pub provenance: ProvenanceView<D>,
}
```

Every `CauseRef` contained in the result's change set MUST resolve through the result's `ProvenanceView<D>`, even if the originating machine later compacts optional provenance. Retaining a transaction result may therefore retain shared immutable causal records associated with that result.

Fields MAY become private if equivalent accessors are provided. The result MUST remain an owned immutable semantic artifact.

## 65. Semantic change set

```rust
pub struct SemanticChangeSet<D> {
    pub processed_times: Vec<Time<D>>,
    pub output_events: Vec<OutputEvent<D>>,
    pub state_changes: Vec<StateChange<D>>,
    pub topology_changes: Vec<TopologyChange>,
    pub pending_event_changes: Vec<PendingEventChange<D>>,
    pub diagnostic_episode_changes: Vec<DiagnosticEpisodeChange<D>>,
    pub provenance_root_changes: Vec<ProvenanceRootChange<D>>,
    pub region_changes: Vec<RegionChange>,
}
```

The collection order MUST be deterministic.

## 66. Output events

```rust
#[non_exhaustive]
pub enum OutputEvent<D> {
    LevelEstablished {
        output: ExternalOutputKey<Level>,
        value: LogicLevel,
        at: Time<D>,
        cause: CauseRef,
        revision: NetworkRevision,
    },
    LevelChanged {
        output: ExternalOutputKey<Level>,
        from: LogicLevel,
        to: LogicLevel,
        at: Time<D>,
        cause: CauseRef,
        revision: NetworkRevision,
    },
    Pulsed {
        output: ExternalOutputKey<Pulse>,
        count: PulseCount,
        at: Time<D>,
        cause: CauseRef,
        revision: NetworkRevision,
    },
}
```

`OutputEvent<D>` MUST remain one flat chronological network-wide event stream.

## 67. Change typing

Heterogeneous state and topology changes SHOULD use closed enums with typed variants rather than unstructured key-value maps.

A dynamic schema view MAY accompany them for generic tooling.

---

# Part XVIII — Runtime failures

## 68. Failure layering

Runtime execution uses:

```rust
Result<TransactionResult<D>, RuntimeFailure>
```

The public failure hierarchy SHOULD be layered:

```rust
#[non_exhaustive]
pub enum RuntimeFailure {
    Lifecycle(LifecycleFailure),
    StaleRevision(StaleRevisionFailure),
    StaleExecutionState(StaleExecutionStateFailure),
    Time(TimeFailure),
    Input(InputFailure),
    Reconfiguration(ReconfigurationFailure),
    SemanticRejection(SemanticRejection),
    Budget(BudgetFailure),
}
```

Wrapper variants are ergonomic categories, not additional semantic conditions. Every leaf failure variant MUST map one-to-one to one catalogue code and its exact typed evidence variant, and MUST expose the complete `Problem<D>` or an exact lossless typed projection of it.

Failure types MUST provide common access to code, severity, responsibility, primary subject, related subjects, and suggestions without requiring `Display` or `Debug` parsing. Variant matching MAY provide the code-specific evidence payload.

Catch-all leaf variants such as `Other(String)` or `UnknownFailure { message: String }` MUST NOT be used for conditions that belong to the catalogue.

## 69. Internal defects

Internal invariant violations MUST NOT be represented as ordinary `RuntimeFailure` values intended to blame caller-controlled data.

The structured defect form is:

```rust
pub struct DefectContext<D> { /* standard non-semantic reproduction envelope */ }

pub struct InternalDefect<D> {
    problem: Problem<D>,
    context: DefectContext<D>,
}

impl<D> InternalDefect<D> {
    pub fn problem(&self) -> &Problem<D>;
    pub fn context(&self) -> &DefectContext<D>;
}
```

The contained problem MUST use an `internal.*` code with `LibraryDefect` responsibility and `InternalDefect` delivery. `DefectContext<D>` MAY carry the standard non-semantic reproduction envelope defined by the diagnostic catalogue.

An implementation MAY expose internal defects through a dedicated verification result, test or debug API, or structured panic payload according to the processor defect policy. In every case, the complete `InternalDefect<D>` MUST be constructed before delivery and made available to tests and support tooling.

Bare assertions, arbitrary panic strings, or ordinary runtime-failure variants are insufficient for named catalogue invariants.

The public API guarantees atomicity for structured operation failures. It does not promise arbitrary recovery from an internal panic or process corruption.

## 70. Non-exhaustive public enums

Public failure, problem-evidence, inspection, explanation, and change enums SHOULD be marked `#[non_exhaustive]` unless exhaustive matching is an intentional long-term compatibility promise.

Core semantic enums whose closed set is itself part of the model, such as `LogicLevel`, MAY remain exhaustive.

---

# Part XIX — Inspection

## 71. Direct inspection

```rust
impl<D> Machine<D> {
    pub fn inspect_node(
        &self,
        node: NodeKey,
    ) -> Result<NodeInspection<D>, InspectionFailure>;

    pub fn inspect_output<S: SignalType>(
        &self,
        output: ExternalOutputKey<S>,
    ) -> Result<OutputInspection<S, D>, InspectionFailure>;

    pub fn inspect_pending(
        &self,
        event: PendingEventKey,
    ) -> Result<PendingEventInspection<D>, InspectionFailure>;
}
```

Equivalent methods SHOULD exist on `ForecastState<D>`.

## 72. Owned inspection values

Canonical inspection results are owned:

```rust
pub struct NodeInspection<D> { /* owned structured result */ }
pub struct OutputInspection<S, D> { /* owned structured result */ }
pub struct PendingEventInspection<D> { /* owned structured result */ }
```

They MUST identify the observed revision and logical time where applicable.

They MUST distinguish:

```text
current persistent levels
reaction-scoped retained pulse history
internal stored state
pending future work
active diagnostic episodes
current causal support
latest transition cause
```

## 73. Stable queries

```rust
pub struct InspectionQuery<D> { /* stable-keyed query */ }
pub struct InspectionQueryBuilder<D> { /* opaque */ }

impl<D> InspectionQuery<D> {
    pub fn builder() -> InspectionQueryBuilder<D>;
}
```

Representative builder methods:

```rust
impl<D> InspectionQueryBuilder<D> {
    pub fn node(self, node: NodeKey) -> Self;
    pub fn output(self, output: AnyExternalOutputKey) -> Self;
    pub fn pending_for(self, node: NodeKey) -> Self;
    pub fn explanation(self, output: AnyExternalOutputKey) -> Self;
    pub fn finish(self) -> InspectionQuery<D>;
}
```

## 74. Compiled plans

```rust
pub struct InspectionPlan<D> { /* revision-bound opaque plan */ }
```

Plan compilation and execution belong to the machine because topology revision is machine-local:

```rust
impl<D> Machine<D> {
    pub fn compile_inspection(
        &self,
        query: &InspectionQuery<D>,
    ) -> Result<InspectionPlan<D>, InspectionPlanFailure>;

    pub fn inspect(
        &self,
        plan: &InspectionPlan<D>,
    ) -> Result<InspectionSnapshot<D>, InspectionFailure>;
}
```

The plan MUST fail structurally when its fingerprint or revision is stale.

It MUST NOT silently retarget surviving or newly introduced subjects.

## 75. Structural inspection before initialization

Structural graph and definition inspection belongs to `CompiledNetwork<D>` and remains available before machine initialization.

Runtime inspection methods requiring settled state MUST return `InspectionFailure::NotInitialized` on an uninitialized machine.

---

# Part XX — Explanation and provenance

## 76. Cause references

```rust
pub struct CauseRef(/* opaque committed provenance reference */);
pub struct ProvenanceView<D> { /* immutable shared causal records */ }
```

A cause reference is meaningful only together with the machine or owned artifact that retains its provenance. It MUST NOT expose a private arena index as durable external identity.

`TransactionResult<D>` owns or shares a `ProvenanceView<D>` sufficient to resolve every cause reference in that result. Owned inspection artifacts containing cause references MUST follow the same rule. A machine may compact its later optional history without invalidating already returned artifacts that retain their own shared provenance view.

Representative access is:

```rust
impl<D> ProvenanceView<D> {
    pub fn inspect(
        &self,
        cause: CauseRef,
    ) -> Result<CauseInspection<D>, ExplanationFailure>;
}
```

## 77. Explanation requests

```rust
#[non_exhaustive]
pub enum Explain<D> {
    CurrentNode(NodeKey),
    CurrentOutput(AnyExternalOutputKey),
    Pending(PendingEventKey),
    DesiredLevel {
        output: ExternalOutputKey<Level>,
        desired: LogicLevel,
    },
    MissingPulse {
        output: ExternalOutputKey<Pulse>,
        since: Time<D>,
    },
}
```

Execution:

```rust
impl<D> Machine<D> {
    pub fn explain(
        &self,
        request: Explain<D>,
    ) -> Result<Explanation<D>, ExplanationFailure>;
}
```

Equivalent read-only execution SHOULD exist on `ForecastState<D>`.

## 78. Explanation values

```rust
pub struct Explanation<D> {
    pub subject: SubjectRef,
    pub observed: ObservedState<D>,
    pub conclusion: ExplanationConclusion,
    pub supporting_causes: Vec<ExplanationEdge<D>>,
    pub blockers: Vec<Blocker<D>>,
    pub pending_conditions: Vec<PendingCondition<D>>,
    pub paths: Vec<NetworkPath>,
    pub relevant_times: Vec<Time<D>>,
    pub retention: RetentionStatus<D>,
}
```

The explanation API MUST return structured data independent from rendered prose.

Provenance supporters that are semantically unordered MUST remain unordered semantically even if stored in canonical representation order.

---

# Part XXI — Graph access

## 79. Graph views

```rust
pub struct GraphView<'a, D> { /* immutable compiled structural view */ }
pub struct DefinitionGraphView<'a, D> { /* immutable authored view */ }
```

These views borrow immutable artifacts and MUST NOT permit mutation.

They MAY provide iterators borrowing the underlying artifact to avoid copying complete graph structures.

## 80. Graph queries

Representative compiled queries:

```rust
impl<D> CompiledNetwork<D> {
    pub fn regions(&self) -> RegionIter<'_, D>;

    pub fn region_containing(
        &self,
        node: NodeKey,
    ) -> Result<RegionRef<'_, D>, GraphQueryFailure>;

    pub fn slice_affecting(
        &self,
        output: AnyExternalOutputKey,
    ) -> Result<NetworkSlice, GraphQueryFailure>;

    pub fn slice_affected_by(
        &self,
        input: AnyExternalInputKey,
    ) -> Result<NetworkSlice, GraphQueryFailure>;

    pub fn slice_between(
        &self,
        source: SubjectRef,
        destination: SubjectRef,
    ) -> Result<NetworkSlice, GraphQueryFailure>;
}
```

`NetworkSlice` SHOULD be an owned stable-keyed result suitable for later use after the borrowed graph view is dropped.

---

# Part XXII — External bindings

## 81. Binding sets

```rust
pub struct BindingSet<I, O> { /* immutable external adapter */ }
pub struct BindingSetBuilder<'a, D, I, O> { /* borrows compiled network */ }
```

Construction:

```rust
impl<I, O> BindingSet<I, O>
where
    I: Eq + core::hash::Hash + Clone,
    O: Eq + core::hash::Hash + Clone,
{
    pub fn builder<D>(
        compiled: &CompiledNetwork<D>,
    ) -> BindingSetBuilder<'_, D, I, O>;
}
```

Representative methods:

```rust
impl<'a, D, I, O> BindingSetBuilder<'a, D, I, O> {
    pub fn bind_input<S: SignalType>(
        self,
        endpoint: ExternalInputKey<S>,
        external: I,
    ) -> Result<Self, BindingFailure>;

    pub fn bind_output<S: SignalType>(
        self,
        endpoint: ExternalOutputKey<S>,
        external: O,
    ) -> Result<Self, BindingFailure>;

    pub fn finish(self)
        -> Result<BindingSet<I, O>, BindingFailure>;
}
```

Bindings are outside semantic machine state.

## 82. Input projection

```rust
pub struct InputProjector<D, I> { /* network-bound immutable adapter */ }
```

The binding set SHOULD provide:

```rust
impl<I, O> BindingSet<I, O> {
    pub fn input_projector<D>(
        &self,
        compiled: &CompiledNetwork<D>,
    ) -> Result<InputProjector<D, I>, BindingFailure>;
}
```

The projector converts caller-owned observations into `InputSnapshot<D>` or `InputDelta<D>` while diagnosing missing, duplicate, unknown, ambiguous, wrong-kind, and stale-schema observations.

## 83. Optional bound-machine façade

An ergonomic façade MAY exist:

```rust
pub struct BoundMachine<D, I, O> {
    machine: Machine<D>,
    bindings: BindingSet<I, O>,
}
```

It MUST delegate to the same core machine semantics and MUST NOT introduce callbacks into propagation.

The bound façade MUST NOT become the only way to use the machine.

---

# Part XXIII — Reconfiguration

## 84. Patch values

```rust
pub struct NetworkPatch<D> { /* owned stable-keyed graph rewrite */ }
pub struct NetworkPatchBuilder<D> { /* opaque */ }
```

The exact exhaustive patch-operation catalogue is defined separately.

This API specification requires that the builder support the operation families:

```text
add or remove node
add or remove connection
add or remove external endpoint
change built-in semantic parameters
change module membership or hierarchy
change diagnostic metadata
explicitly preserve or reassociate stable identity where allowed
select node-specific pending-event migration policy
```

## 85. Patch construction

A patch records both the base topology fingerprint and an explicit machine-local base revision. A low-level builder is created from compiled topology plus that revision:

```rust
let builder = compiled.patch(base_revision);
```

A machine convenience seeds the current revision automatically:

```rust
impl<D> Machine<D> {
    pub fn patch(&self) -> NetworkPatchBuilder<D> {
        self.compiled().patch(self.revision())
    }
}
```

The builder exposes:

```rust
impl<D> NetworkPatchBuilder<D> {
    pub fn base_revision(&self) -> NetworkRevision;
    pub fn base_fingerprint(&self) -> NetworkFingerprint;
    pub fn finish(self) -> NetworkPatch<D>;
}
```

Representative methods MAY include:

```rust
pub fn add_node(self, node: NodeDef<D>) -> Result<Self, PatchBuildFailure>;
pub fn remove_node(self, node: NodeKey) -> Result<Self, PatchBuildFailure>;
pub fn add_connection(self, connection: ConnectionDef) -> Result<Self, PatchBuildFailure>;
pub fn remove_connection(self, connection: ConnectionKey) -> Result<Self, PatchBuildFailure>;
```

Patch construction SHOULD diagnose immediate internal contradictions but full semantic validation occurs during preparation. `CompiledNetwork::prepare_patch` MUST reject a patch whose base fingerprint does not match the compiled topology. `Machine::prepare_patch` MUST additionally reject a patch whose base revision does not equal the machine's current revision.

## 86. Structural patch preparation

```rust
pub struct PreparedPatch<D> { /* immutable target topology and migration program */ }
```

Required accessors:

```rust
impl<D> PreparedPatch<D> {
    pub fn base_revision(&self) -> NetworkRevision;
    pub fn proposed_revision(&self) -> NetworkRevision;
    pub fn resulting_fingerprint(&self) -> NetworkFingerprint;
    pub fn resulting_compiled(&self) -> &CompiledNetwork<D>;
    pub fn static_plan(&self) -> &StaticMigrationPlan<D>;
}
```

`PreparedPatch<D>` SHOULD be cheaply cloneable through immutable shared ownership.

It is bound to one exact base topology revision but not to one exact execution-state digest.

## 87. Preparation placement

The canonical preparation method belongs to `CompiledNetwork<D>` because preparation is topology-dependent and not state-dependent.

A convenience method MAY exist:

```rust
impl<D> Machine<D> {
    pub fn prepare_patch(
        &self,
        patch: NetworkPatch<D>,
    ) -> Report<PreparedPatch<D>, D> {
        self.compiled().prepare_patch(patch)
    }
}
```

The convenience MUST NOT imply that current runtime state participates in structural preparation.

## 88. Reconfiguration policy

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReconfigurationPolicy {
    RejectStateLoss,
    AllowReportedStateLoss,
}
```

Node-specific migration-policy values are carried in the patch or preparation artifact, not inferred from implementation convenience at application time.

---

# Part XXIV — Snapshots and restoration

## 89. Machine snapshots

```rust
pub struct MachineSnapshot<D> { /* opaque complete semantic state */ }
```

Snapshot creation:

```rust
impl<D> Machine<D> {
    pub fn snapshot(&self) -> MachineSnapshot<D>;
}

impl<D> ForecastState<D> {
    pub fn snapshot(&self) -> MachineSnapshot<D>;
}
```

The canonical snapshot is owned and stable-keyed. It MUST NOT borrow the machine.

## 90. Snapshot access

A snapshot SHOULD expose summary information without exposing mutable internals:

```rust
impl<D> MachineSnapshot<D> {
    pub fn status(&self) -> MachineStatus<D>;
    pub fn revision(&self) -> NetworkRevision;
    pub fn fingerprint(&self) -> NetworkFingerprint;
    pub fn execution_state_digest(&self) -> ExecutionStateDigest;
    pub fn observable_state_digest(&self) -> ObservableStateDigest;
    pub fn snapshot_digest(&self) -> SnapshotDigest;
    pub fn runtime_policy_id(&self) -> RuntimePolicyId;
}
```

Serialized encoding is defined separately.

## 91. Restoration

Restoration remains:

```rust
CompiledNetwork::restore(snapshot, policy)
    -> Result<Machine<D>, RestoreFailure>
```

Restoration MUST return a complete machine or a structured failure. It MUST NOT return a partially restored machine accompanied by warnings.

Warnings about accepted compatible metadata MAY be returned through a distinct restoration report only if they do not weaken the all-or-nothing machine artifact boundary.

---

# Part XXV — Replay

## 92. Replay frames

```rust
pub struct ReplayFrame<D> {
    pub expected_previous_execution_digest: ExecutionStateDigest,
    pub expected_revision: NetworkRevision,
    pub runtime_policy_id: RuntimePolicyId,
    pub transaction: Transaction<D>,
    pub resulting_execution_digest: ExecutionStateDigest,
}
```

A frame is an owned value.

## 93. Frame creation

A successful transaction result SHOULD support explicit frame construction:

```rust
impl<D> ReplayFrame<D> {
    pub fn from_success(
        transaction: Transaction<D>,
        result: &TransactionResult<D>,
    ) -> Result<Self, ReplayFrameFailure>;
}
```

Because `Machine::apply` consumes the transaction, callers wishing to retain it for replay MUST either clone it before application or use a helper that records execution:

```rust
impl<D> Machine<D> {
    pub fn apply_recorded(
        &mut self,
        transaction: Transaction<D>,
    ) -> Result<RecordedTransaction<D>, RuntimeFailure>;
}

pub struct RecordedTransaction<D> {
    pub result: TransactionResult<D>,
    pub frame: ReplayFrame<D>,
}
```

`apply_recorded` MUST use the same transition semantics as `apply`.

## 94. Replay execution

```rust
pub fn replay<D, I>(
    machine: &mut Machine<D>,
    frames: I,
) -> Result<ReplayResult<D>, ReplayFailure>
where
    I: IntoIterator<Item = ReplayFrame<D>>;
```

Replay MUST stop at the first incompatible frame and report the exact frame position and structured reason.

The machine MUST retain the state reached by all prior successfully applied frames unless the replay API explicitly promises whole-sequence atomicity.

A separate atomic replay convenience MAY clone the machine and publish only on complete success.

---

# Part XXVI — Persistent diagnostic episodes and observers

## 95. Active diagnostic episodes

Inspection exposes active episodes through owned records:

```rust
pub struct DiagnosticConditionKey { /* opaque canonical condition identity */ }

pub struct DiagnosticEpisode<D> {
    pub identity: DiagnosticEpisodeId,
    pub condition: DiagnosticConditionKey,
    pub current: Problem<D>,
    pub began_at: Time<D>,
    pub last_material_change: Time<D>,
}

#[non_exhaustive]
pub enum DiagnosticEpisodeChangeKind {
    Began,
    Changed,
    Resolved,
    Terminated,
}

pub struct DiagnosticEpisodeChange<D> {
    pub identity: DiagnosticEpisodeId,
    pub kind: DiagnosticEpisodeChangeKind,
    pub at: Time<D>,
    pub before: Option<Problem<D>>,
    pub after: Option<Problem<D>>,
}
```

The condition key contains the code, primary subject, and catalogue-defined condition discriminator. The `current` problem MUST agree with that code and primary subject and may contain only a code permitting `PersistentEpisode` delivery.

The code remains the underlying condition code across begin, change, resolution, and termination. Those are change kinds, not separate diagnostic codes.

Episode identity MUST be opaque and stable across compatible machine state transitions. An unchanged active condition MUST NOT emit a repeated change on an unrelated transaction.

## 96. Observer separation

Observer subscriptions are not part of `Machine<D>`.

A future observer layer MAY define:

```rust
pub struct Observer<D> { /* external delivery state */ }
pub struct ObserverCursor { /* opaque */ }
```

The machine’s stable public integration point is the committed `SemanticChangeSet<D>` plus fresh inspection.

The initial concrete core API does not require subscriptions to be implemented before ordinary transaction results and inspection are available.

---

# Part XXVII — Ownership, cloning, and thread boundaries

## 97. Cheaply cloneable immutable artifacts

The following SHOULD be cheaply cloneable through immutable shared ownership:

```text
CompiledNetwork<D>
PreparedPatch<D>
InspectionPlan<D>
RuntimePolicy
```

Cloning these values SHOULD NOT duplicate full topology graphs or migration programs.

The exact use of `Arc` or another mechanism is private.

## 98. Expensive semantic clones

The following MAY perform work proportional to semantic content when cloned:

```text
UncheckedNetwork<D>
ValidatedNetwork<D>
Transaction<D>
InputSnapshot<D>
InputDelta<D>
TransactionResult<D>
MachineSnapshot<D>
Explanation<D>
```

The API documentation SHOULD avoid promising cheap clone for these types.

`Machine<D>` SHOULD NOT implement `Clone` as an ordinary convenience unless the cost and semantic meaning are explicit. Internal forecast and reference execution may use a private clone path.

A public explicit method MAY exist:

```rust
pub fn fork(&self) -> Machine<D>;
```

if independent machine branching is considered a supported use case. The method name SHOULD make the semantic duplication clear.

## 99. Mutation and concurrency

All semantic mutation occurs through `&mut Machine<D>`.

The core API does not provide concurrent mutation of one machine.

Read-only inspection uses `&Machine<D>`.

Auto-trait guarantees such as `Send` and `Sync` SHOULD follow naturally from private representation, but the initial specification does not require concurrent lock-free inspection during mutation.

Host synchronization remains the caller’s responsibility.

---

# Part XXVIII — Allocation and string behavior

## 100. Authoring and compilation allocations

Authoring and compilation MAY allocate:

- vectors and maps for graph structure;
- strings for metadata;
- diagnostics and evidence;
- validation and compilation workspaces;
- compiled immutable arrays and lookup tables.

These stages prioritize clarity and correctness over allocation avoidance.

## 101. Runtime storage

Spawning a machine MAY allocate persistent runtime storage proportional to compiled topology and policy.

Ordinary runtime evaluation SHOULD reuse:

- level arrays;
- pulse-generation arrays;
- state-family storage;
- dirty worklists;
- transaction staging buffers;
- event arenas where possible.

The public API does not guarantee zero allocations per transaction because genuine semantic changes may create:

- pending events;
- provenance records;
- diagnostics;
- output and state-change records;
- migration reports.

## 102. String movement and cloning

Metadata strings are owned by definitions and artifacts.

Moving a network, compiled handle, transaction, result, or snapshot MUST move ownership rather than clone all string buffers.

The library SHOULD avoid cloning metadata into every runtime event. Runtime events SHOULD primarily carry stable keys and structured codes, with text resolved or shared where needed.

String interning is not part of the initial public API.

`Arc<str>` or internal interning MAY be introduced privately if profiling demonstrates benefit without changing semantics or persistence.

---

# Part XXIX — End-to-end example

## 103. Typed authoring and initialization

```rust
use mossignal::prelude::*;

#[derive(Debug, Clone, Copy)]
struct Ticks;

let mut net = NetworkBuilder::<Ticks>::new();

let (set_key, set) = net.pulse_input("set");
let (reset_key, reset) = net.pulse_input("reset");

let stored = net
    .pulse_set_reset_latch(
        set,
        reset,
        PulseSetResetConfig {
            initial: LogicLevel::Low,
            conflict: ConflictPolicy::ResetDominant,
        },
    )?;

let state_key = net.level_output("state", stored)?;

let validated = net.finish().require_artifact()?;
let compiled = validated.compile().require_artifact()?;

let policy = RuntimePolicy::builder()
    .max_internal_reactions(10_000)
    .max_evaluated_operations(1_000_000)
    .max_pending_events(100_000)
    .max_events_created_per_transaction(100_000)
    .max_required_provenance_growth(1_000_000)
    .build()?;

let mut machine = compiled.spawn(policy);

let initial = compiled
    .input_snapshot()
    .pulse(set_key, PulseCount::ONE)?
    .finish()?;

let init = Transaction::initialize(
    Time::from_ticks(100),
    machine.revision(),
    initial,
);

let result = machine.apply(init)?;

assert!(result.changes.output_events.iter().any(|event| {
    matches!(
        event,
        OutputEvent::LevelEstablished {
            output,
            value: LogicLevel::High,
            ..
        } if *output == state_key
    )
}));
```

## 104. Ready-machine transaction

```rust
let delta = compiled
    .input_delta()
    .pulse(reset_key, PulseCount::ONE)?
    .finish()?;

let tx = Transaction::advance(
    Time::from_ticks(101),
    machine.revision(),
    delta,
);

let result = machine.apply(tx)?;
```

## 105. Forecast and inspection

```rust
let delta = machine
    .compiled()
    .input_delta()
    .pulse(set_key, PulseCount::ONE)?
    .finish()?;

let forecast = machine.forecast(Transaction::advance(
    Time::from_ticks(102),
    machine.revision(),
    delta,
))?;

let hypothetical = forecast
    .state()
    .inspect_output(state_key)?;

let current = machine.inspect_output(state_key)?;

assert_ne!(hypothetical.level(), current.level());
```

## 106. Patch adding a new external level input

```rust
let new_input_key = ExternalInputKey::<Level>::from_u128(0x1000);

let patch = machine
    .patch()
    .add_external_level_input(
        new_input_key,
        DiagnosticMeta {
            name: Some("enable".into()),
            ..Default::default()
        },
    )?
    .finish();

let prepared = machine
    .prepare_patch(patch)
    .require_artifact()?;

let target_input = prepared
    .input_delta()
    .establish(new_input_key, LogicLevel::High)?
    .finish()?;

let tx = Transaction::advance(
    Time::from_ticks(103),
    machine.revision(),
    target_input,
)
.with_patch(
    prepared,
    ReconfigurationPolicy::RejectStateLoss,
)?;

let result = machine.apply(tx)?;
```

The new level input is established explicitly. Existing preserved inputs retain their previous authoritative values unless changed through `set`.

---

# Part XXX — Public API coherence requirements

## 107. Shared identity model

Builder, dynamic definition, validation, compilation, runtime, diagnostics, inspection, explanation, snapshots, replay, bindings, and reconfiguration MUST use the same stable structural keys.

No subsystem may invent an incompatible parallel identity model.

## 108. Shared lifecycle model

All public APIs MUST agree that:

- a newly spawned machine is uninitialized;
- initialization requires a complete target-topology `InputSnapshot`;
- ready-machine advancement uses a target-topology `InputDelta`;
- schedule and current runtime inspection are unavailable before initialization;
- `Low` is not an uninitialized marker.

## 109. Shared revision model

Resolved handles, inspection plans, prepared patches, input artifacts, transactions, forecasts, snapshots, and replay frames MUST expose or internally retain their revision and topology binding where needed.

Stale use MUST fail structurally rather than retarget.

## 110. Shared failure model

Public stages use:

```text
Report<T, D>
```

when independent report findings can safely accumulate, and:

```text
Result<T, Failure>
```

when the operation has one atomic success or failure outcome.

Successful runtime occurrences, persistent episode changes, and implementation defects remain distinct delivery forms:

```text
DiagnosticOccurrence<D>
DiagnosticEpisodeChange<D>
InternalDefect<D>
```

All forms share the catalogue-backed `Problem<D>` model. The same condition MUST NOT acquire a different severity or responsibility merely because another API surface detected it.

## 111. No semantic callbacks

The evaluator, patch finalizer, snapshot restorer, replay engine, and provenance builder MUST NOT call arbitrary host callbacks as part of semantic execution.

Host-visible behavior is returned as owned committed results.

## 112. No hidden defaults

The concrete API MUST NOT hide:

- initialization level values;
- positive temporal spans;
- runtime policy;
- conflict policy;
- re-enable phase policy;
- state-loss policy;
- migration policy where work is pending;
- newly added external level establishment.

Named convenience constructors MAY provide explicit documented policy bundles, but the selected values MUST remain inspectable.

---

# Part XXXI — Deliberately deferred details

## 113. Deferred exact surfaces

This specification does not freeze:

- the generated Rust spelling and module placement of exhaustive catalogue code constants;
- the generated spelling of exhaustive code-specific evidence and suggestion variants;
- the complete topology-patch operation enum;
- standard-module constructor catalogue;
- serialized encodings and serde feature shape;
- observer subscription types;
- exact async integration helpers;
- exact iterator concrete types;
- internal sharing mechanism;
- exact digest byte width or algorithm;
- optional borrowed zero-copy inspection views;
- optional public machine-forking API;
- feature flags and platform support.

These additions must preserve the ownership, identity, lifecycle, and failure boundaries defined here.

The common `Problem<D>`, `Diagnostic<D>`, `DiagnosticOccurrence<D>`, `DiagnosticEpisode<D>`, and `InternalDefect<D>` distinctions are not deferred.

## 114. Permitted ergonomic additions

The implementation MAY add:

- convenience aliases;
- method chaining;
- borrowed metadata setters;
- iterators and collection adapters;
- `TryFrom` conversions;
- display and rendering helpers;
- typed wrappers over common inspection results;
- macro helpers for explicit stable keys;
- optional serde support;
- optional bound-machine façades.

Such additions MUST delegate to the canonical owned semantic values and MUST NOT create alternate semantics.

---

# Part XXXII — Required concrete API properties

## 115. Required guarantees

The concrete Rust API must preserve:

```text
Level and Pulse are statically distinct
logical time domains are statically distinct
positive spans are statically distinct from arbitrary spans
checked arithmetic never wraps
stable structural keys are opaque and caller-preservable
builder signals cannot silently cross builder boundaries
Signal<S> remains builder-only
ordinary and explicit-key authoring are both supported
typed builder and dynamic definition produce one semantic model
unchecked, validated, and compiled artifacts are distinct types
compiled topology is immutable and cheaply shareable
machine owns its current topology and runtime policy
machine lifecycle is explicit without mandatory typestate
initialization and advancement have distinct transaction constructors
snapshots and deltas are distinct owned types
input artifacts are bound to exact target topology schemas
patch-time new level inputs require explicit establishment
transactions are owned inspectable data
apply mutates only on complete success
forecast exposes read-only hypothetical state without implicit commit
results and change sets are owned deterministic artifacts
stable keys and revision-bound resolved handles remain distinct
stale handles and plans fail without retargeting
reports accumulate structural diagnostics
problem evidence is code-specific and typed
report findings, runtime occurrences, persistent episodes, and internal defects remain distinct
runtime failures remain structured and atomic
inspection and explanations are owned semantic projections
bindings remain outside machine semantics
patch preparation is topology-dependent and reusable
snapshots are complete owned semantic artifacts
replay uses ordinary transaction semantics
metadata strings do not participate in evaluation
runtime does not require per-node heap objects or callbacks
```

---

# Summary

The initial concrete Rust API is deliberately data-oriented and moderately typed.

Its normal flow is:

```text
NetworkBuilder<D>
        ↓ finish
Report<ValidatedNetwork<D>, D>
        ↓ compile
Report<CompiledNetwork<D>, D>
        ↓ spawn(RuntimePolicy)
Machine<D>
        ↓ apply(Transaction<D>)
TransactionResult<D>
```

The static type system distinguishes the categories that callers encounter constantly:

```text
Level versus Pulse
input versus output
time domain
positive versus arbitrary duration
unchecked versus validated versus compiled
snapshot versus delta
stable versus resolved identity
```

Dynamic validation handles facts that depend on authored structure, revisions, runtime lifecycle, migration, or persistence:

```text
builder ownership
key uniqueness
input completeness
reaction acyclicity
revision freshness
schema compatibility
state migration
snapshot restoration
budget success
```

The API avoids two extremes:

- it does not collapse the library into untyped maps, strings, and callbacks;
- it does not force ordinary users through generative lifetimes, machine typestates, or deeply generic metadata plumbing.

Authoring and inspection use ordinary owned Rust collections and strings. Compilation converts structure into immutable executable topology. A machine owns dense reusable runtime state. Transactions, patches, snapshots, forecasts, and replay remain explicit owned values. Strings and metadata stay outside evaluator semantics.

The result is an API intended to be precise enough for persistence, reconfiguration, tooling, fuzzing, and causal inspection while remaining usable as an ordinary Rust library.
