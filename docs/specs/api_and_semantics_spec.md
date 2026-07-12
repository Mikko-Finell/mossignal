# `mossignal` API and Semantics Specification

**Status:** Design specification, version 2  
**Defines:** Public semantics, API responsibilities, diagnostics, inspection, persistence, and reconfiguration  
**Does not define:** Implementation architecture, performance strategy, or testing policy

---

## 1. Purpose

`mossignal` is a deterministic, host-agnostic Rust library for defining, validating, initializing, executing, inspecting, explaining, persisting, and reconfiguring discrete signal networks.

A network consists of typed external inputs, nodes, typed ports, directed connections, mutable machine state, caller-driven temporal behavior, and typed external outputs. The caller supplies topology, authoritative external input, logical time, and an explicit runtime policy. The library validates and executes the network, preserves compatible state across topology revisions, reports observable changes, and exposes structured explanations for current behavior.

The library is designed around a small semantic domain with unusually strong guarantees:

- statically knowable invariants are represented in Rust types;
- dynamically authored definitions receive complete runtime validation;
- a newly spawned machine is explicitly uninitialized until given one complete input snapshot;
- execution is deterministic and glitch-free;
- logical time is exact, discrete, and entirely caller-owned;
- compatible state and pending work survive topology changes;
- every relevant runtime state is inspectable;
- current state, pending work, and output events retain causal provenance;
- persistent diagnostic conditions are represented explicitly rather than repeatedly rediscovered as messages;
- snapshots and replay preserve the complete future-determining semantic state;
- no application-domain concepts enter the evaluator.

`mossignal` is not tied to any particular application or presentation model.

## 2. Normative language

The terms **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.

This specification defines observable behavior and public API responsibilities. It does not prescribe internal algorithms, memory layouts, concurrency strategies, or optimization techniques.

## 3. Scope and non-goals

`mossignal` defines:

- signal kinds and their exact semantics;
- network definitions;
- nodes, ports, connections, and reusable modules;
- typed and dynamic authoring APIs;
- validation and compilation;
- explicit machine initialization;
- deterministic synchronous reactions;
- caller-driven exact logical time;
- stateful and temporal behavior;
- atomic transaction semantics;
- explicit runtime policy and structured budget failure;
- external endpoint bindings;
- runtime inspection;
- causal explanations and provenance checkpoints;
- structured diagnostics and persistent diagnostic episodes;
- state-preserving reconfiguration;
- graph queries;
- scheduling, quiescence, and dormancy;
- semantic change sets for observer layers;
- snapshots, restoration, replay, and hypothetical execution.

`mossignal` MUST NOT:

- own or consult a wall clock;
- sleep, poll, or schedule operating-system work;
- receive arbitrary host state during evaluation;
- invoke host callbacks during propagation;
- transport arbitrary user payloads through signals;
- define domain-specific actions, objects, or entities;
- model physical electricity;
- derive behavior from names, bindings, or diagnostic metadata;
- require a global allocator or singleton identity source;
- make results depend on hash iteration order, insertion order, or thread scheduling;
- treat `Low` as absence, unknown state, or an implicit external-input default;
- make observer subscriptions part of semantic machine state.

Testing policy, model checking, declarative contracts, editor interaction design, and constrained user-defined automata belong in separate specifications.

# Part I — Semantic model

## 4. Networks and regions

A network is a directed graph containing:

- nodes;
- typed input and output ports;
- directed connections;
- typed external inputs;
- typed external outputs;
- optional modules and hierarchy;
- stable structural keys;
- diagnostic metadata.

The caller submits one complete network definition for a caller-chosen scope.

The caller MUST NOT be required to divide a network into independently managed circuits. `mossignal` MUST derive weakly connected regions automatically.

A **region** is a weakly connected component of the graph. Connection direction is ignored only when determining region membership.

Regions are derived views. They do not own separate execution state or separate machine lifecycles. Adding or removing a connection may merge or split regions without changing the identity or state of surviving nodes.

## 5. Signal kinds

The core library defines exactly two signal kinds:

```rust
pub enum Level {}
pub enum Pulse {}
```

These are marker types, not runtime variants of one untyped signal.

### 5.1 Level

A level is persistent state:

```rust
pub enum LogicLevel {
    Low,
    High,
}
```

Once established in a ready machine, a level remains at its current value until a defined event changes it.

Public APIs MUST use `LogicLevel` rather than raw `bool` values wherever the value is semantically a signal level.

### 5.2 Pulse

A pulse is a discrete occurrence at a logical time. Pulse values are reaction-scoped multiplicities, not persistent current port state.

A pulse is not:

- a temporary `High`;
- a third level value;
- a level that implicitly resets;
- an untyped notification.

Pulse multiplicity is part of the semantics:

```rust
pub struct PulseCount(/* non-negative integer */);
```

`PulseCount` MUST support at least `ZERO` and `ONE`.

If several pulses reach one endpoint at one logical time, their multiplicity MUST be preserved unless an explicit node coalesces them.

A pulse-aware node MUST define behavior over the complete simultaneous pulse batch. Results MUST NOT depend on an arbitrary ordering of pulses sharing the same time.

Examples:

- a toggle receiving two pulses returns to its prior state;
- a coalescer converts one or more pulses into exactly one;
- a merge preserves total multiplicity.

### 5.3 Explicit conversion

Conversion between kinds MUST be explicit.

```text
RisingEdge   Level -> Pulse
FallingEdge  Level -> Pulse
AnyEdge      Level -> Pulse
Toggle       Pulse -> Level
SetReset     Pulse or Level inputs -> Level
Sample       Level sampled by Pulse -> Level
```

The library MUST NOT silently interpret a level transition as a pulse or a pulse as a temporary level.

## 6. Nodes

Nodes are classified as combinational, stateful, or temporal, but current-reaction causality is defined per dependency path rather than per whole node.

### 6.1 Combinational nodes

A combinational node’s output is determined entirely by its settled current level inputs and current simultaneous pulse batch. It retains no semantic state between transactions.

Every combinational node kind MUST define its conservative current-input-to-current-output dependency signature.

Examples include inversion, all-of, any-of, parity, exclusive-or, level gates, pulse gates, pulse merge, pulse coalescing, routing, and selection.

### 6.2 Stateful nodes

A stateful node retains semantic state between transactions.

Examples include edge detectors, set/reset latches, toggles, sample-and-hold nodes, and other explicitly specified memory primitives.

Every stateful node kind MUST define:

- declared initial state;
- complete state schema;
- conservative current-reaction dependency signature;
- current output law over previous committed state and settled current inputs;
- proposed successor-state law;
- simultaneous-input semantics;
- conflicting-input semantics;
- inspection representation;
- causal provenance semantics;
- snapshot representation;
- reconfiguration compatibility.

### 6.3 Temporal nodes

A temporal node schedules behavior for a caller-supplied future logical time.

Examples include pulse delay, transport level delay, inertial level delay, periodic emission, timeout, debounce, and minimum-duration hold.

Every temporal node kind MUST define:

- whether zero duration is valid;
- whether values or transitions are delayed;
- conservative current-reaction dependency signature;
- exact-deadline interaction between due obligations and settled current inputs;
- strictly-future scheduling behavior;
- simultaneous-event semantics;
- pending-event inspection;
- reconfiguration behavior;
- parameter-change behavior while work is pending;
- causal propagation across the delay.

## 7. Ports and connections

Canonical port and endpoint keys are:

```rust
pub struct InPortKey<S> { /* opaque */ }
pub struct OutPortKey<S> { /* opaque */ }

pub struct ExternalInputKey<S> { /* opaque */ }
pub struct ExternalOutputKey<S> { /* opaque */ }

pub struct ConnectionKey { /* opaque */ }
```

`S` is `Level` or `Pulse`.

A connection links one output port to one compatible input port.

A connection MUST NOT join incompatible kinds.

An input port MUST have an explicit driver policy. Unless a node kind explicitly states otherwise, an input accepts at most one connection.

Fan-out is intrinsic. One output may connect to multiple compatible inputs.

Fan-in MUST occur through a node with explicit fan-in semantics. Multiple arbitrary drivers MUST NOT be resolved implicitly.

Connections retain stable identity and diagnostic metadata independently from their endpoint nodes.

## 8. Current-reaction causality and cycles

Each node kind defines a conservative **current-reaction dependency signature** describing which current inputs may affect which current outputs during one reaction.

Compilation derives a directed current-reaction dependency graph from:

- external current-input facts;
- temporal obligations due now;
- previous committed state facts;
- node reaction operations;
- current signal outputs.

A dependency path breaks current-reaction causality only when it crosses:

- previous committed state; or
- strictly later logical time.

A whole stateful or temporal node is not automatically a causality barrier. One node may contain both instantaneous and delayed dependency paths.

For example:

- a toggle's current pulse input may affect its current level output immediately;
- its proposed successor state does not feed the same reaction;
- a pulse delay's current input schedules future work but does not affect its current output;
- a due pulse-delay obligation affects its current output;
- a periodic node's current enable level may affect whether a due boundary emits.

Proposed successor state and newly created future obligations are terminal products of the current reaction. They MUST NOT feed current-reaction evaluation.

The current-reaction dependency graph MUST be acyclic. A cycle in this graph is invalid and MUST prevent compilation.

Only settled pre-reaction and post-reaction values are semantically observable. Internal evaluation order MUST NOT expose intermediate transitions. A downstream edge detector compares its remembered prior observation with the complete settled current input, not with evaluator glitches.

The library MUST NOT use unspecified fixed-point iteration, repeated same-time microsteps, or evaluator traversal order to resolve a cycle.

Cycle diagnostics SHOULD identify the cyclic operations, corresponding node-and-port path, dependency contributed by each node kind, and any apparent state or temporal boundary that does not actually break current-reaction causality.

## 9. Quiescence and dormancy

A ready machine is **quiescent** when no immediate propagation work or unpublished current-time change remains.

A ready machine is **dormant** when it is quiescent and has no pending future event.

```rust
pub enum Schedule<D> {
    Dormant,
    WakeAt(Time<D>),
}
```

A dormant machine MUST NOT change unless the caller supplies new external input, a topology transaction, or restoration or migration.

An uninitialized machine is neither quiescent nor dormant. It has no settled runtime valuation and therefore has no valid `Schedule<D>`.

# Part II — Canonical object model

## 10. Lifecycle

The canonical representation lifecycle is:

```text
NetworkBuilder or UncheckedNetwork
                |
                v
         ValidatedNetwork
                |
                v
         CompiledNetwork
                |
                v
             Machine
        AwaitingInitialization
                |
                v
              Ready
```

Canonical types are:

```rust
pub struct NetworkBuilder<D> { /* typed authoring */ }
pub struct UncheckedNetwork<D> { /* dynamic or deserialized */ }
pub struct ValidatedNetwork<D> { /* structurally valid */ }
pub struct CompiledNetwork<D> { /* immutable executable definition */ }
pub struct Machine<D> { /* mutable running instance */ }
```

A compiled network may spawn any number of independent machines.

A newly spawned machine is explicitly `AwaitingInitialization`. It has declared node initial state and a compiled topology, but it has no authoritative external level valuation, current logical time, settled signal valuation, output baseline, pending schedule, active runtime explanation, or runtime diagnostic episodes.

The first successful transaction initializes the machine and moves it to `Ready`. Initialization uses the ordinary complete reaction evaluator rather than a reduced or special execution path.

## 11. `NetworkBuilder<D>`

`NetworkBuilder<D>` is the strongly typed Rust authoring API.

It creates builder-only typed signal handles:

```rust
pub struct Signal<S> { /* builder-scoped, opaque */ }
```

A `Signal<S>` is not a persistent structural key and MUST NOT appear in snapshots or bindings.

Illustrative API:

```rust
let mut net = NetworkBuilder::<Ticks>::new();

let (a_key, a) = net.level_input("a");
let (b_key, b) = net.level_input("b");

let sum = net.xor(a, b);
let carry = net.all([a, b]);

let sum_key = net.level_output("sum", sum);
let carry_key = net.level_output("carry", carry);

let validated = net.finish().require_artifact()?;
```

The typed API SHOULD make these invalid states unrepresentable where practical:

- connecting `Level` to `Pulse`;
- reversing connection direction;
- using the wrong fixed arity;
- using signals from unrelated builders;
- referring to unresolved dynamic elements.

Remaining dynamic invariants still receive structured validation.

## 12. `UncheckedNetwork<D>`

`UncheckedNetwork<D>` is the canonical representation for runtime-authored or deserialized definitions.

It contains stable keys, node definitions, typed ports, connections, external endpoints, modules, parameters, hierarchy, and diagnostic metadata.

It may be malformed and MUST NOT compile or instantiate directly.

```rust
let validated = unchecked.validate().require_artifact()?;
```

Malformed definitions MUST produce structured diagnostics rather than panics.

## 13. `ValidatedNetwork<D>`

`ValidatedNetwork<D>` satisfies all pre-execution structural and semantic requirements.

Only a validated network may compile:

```rust
let compiled = validated.compile().require_artifact()?;
```

Validation is a one-way type transition. Invalid data MUST NOT be representable as `ValidatedNetwork<D>`.

## 14. `CompiledNetwork<D>`

`CompiledNetwork<D>` is an immutable executable specification.

It MUST:

- retain stable identity for nodes, ports, connections, modules, and endpoints;
- retain hierarchy and diagnostic metadata;
- expose graph queries;
- expose a network fingerprint;
- support multiple independent machines;
- contain no mutable runtime state.

```rust
let machine = compiled.spawn();
```

## 15. `Machine<D>`

`Machine<D>` is one mutable execution instance.

It owns:

- lifecycle status;
- current logical time when ready;
- topology revision;
- authoritative external level state when ready;
- current settled level-port state when ready;
- stateful-node state;
- temporal-node state;
- pending events;
- external level-output baselines;
- required causal provenance roots and checkpoints;
- active persistent diagnostic episodes;
- explicit runtime policy identity;
- scheduling state when ready.

```rust
pub enum MachineStatus<D> {
    AwaitingInitialization,
    Ready { now: Time<D> },
}
```

`Low` is a real level value and MUST NOT be used as an implicit external default. The library MUST NOT add a third `LogicLevel` value to represent missing initialization; lifecycle state carries that distinction.

Client-specific subscriptions, delivery cursors, compiled inspection plans, retained observer deltas, and acknowledgement state are not owned by `Machine<D>` and MUST NOT affect its semantic behavior.

# Part III — Identity, bindings, and metadata

## 16. Stable structural keys

Canonical stable keys are:

```rust
pub struct NodeKey { /* opaque */ }
pub struct ConnectionKey { /* opaque */ }
pub struct ModuleInstanceKey { /* opaque */ }

pub struct InPortKey<S> { /* opaque */ }
pub struct OutPortKey<S> { /* opaque */ }

pub struct ExternalInputKey<S> { /* opaque */ }
pub struct ExternalOutputKey<S> { /* opaque */ }
```

Structural keys:

- are unique within one network identity;
- survive compatible revisions when the logical element survives;
- are independent of compiled execution positions;
- require no global allocator;
- carry no semantic meaning through numeric ordering.

A definition may allocate keys locally. A caller rebuilding a definition across revisions MUST be able to preserve or explicitly re-associate keys for surviving elements.

## 17. External bindings

Bindings associate endpoints with opaque caller-owned keys.

```rust
pub struct BindingSet<I, O> { /* opaque */ }
```

```rust
let bindings = BindingSet::builder(&compiled)
    .bind_input(input_key, external_input_id)?
    .bind_output(output_key, external_output_id)?
    .finish()?;
```

Binding keys:

- are supplied and owned by the caller;
- are not interpreted by `mossignal`;
- MUST NOT affect behavior;
- remain separate from structural keys;
- SHOULD identify a specific external port.

Bindings SHOULD support lookup in both directions.

The core evaluator operates only on structural endpoints. An optional ergonomic wrapper may combine a machine and binding set:

```rust
pub struct BoundMachine<D, I, O> { /* opaque */ }
```

`BoundMachine` MUST NOT change evaluator semantics.

## 18. Diagnostic metadata

Human-readable identity is separate from structural identity and bindings.

Every relevant element MAY carry:

```rust
pub struct DiagnosticMeta<Origin = ()> {
    pub name: Option<String>,
    pub description: Option<String>,
    pub path: Option<DiagnosticPath>,
    pub origin: Option<Origin>,
    pub tags: Vec<String>,
}
```

This is a semantic shape, not a mandated storage representation.

Diagnostic metadata MUST NOT affect execution, state compatibility, or semantic fingerprints.

## 19. Revision, fingerprint, digests, and runtime policy

These concepts are distinct:

```rust
pub struct NetworkRevision(/* machine-local topology revision */);
pub struct NetworkFingerprint(/* semantic topology identity */);

pub struct ExecutionStateDigest(/* future-determining state */);
pub struct ObservableStateDigest(/* complete current observable state */);
pub struct SnapshotDigest(/* complete persisted artifact */);

pub struct RuntimePolicyId(/* canonical semantically relevant policy identity */);
```

### 19.1 Topology revision

`NetworkRevision` identifies the topology installed in one machine.

A state-only transaction does not change the topology revision. A committed topology patch advances it. Therefore a successful transaction may have:

```text
before_revision == after_revision
```

### 19.2 Network fingerprint

A fingerprint MUST reflect structural keys, node kinds, typed ports, connections, semantic parameters, state-relevant module structure, and signal-semantics version.

It MUST exclude dense runtime indices, construction order, memory layout, and presentation-only metadata.

### 19.3 Digest scopes

`ExecutionStateDigest` covers state capable of affecting future execution, including topology fingerprint, lifecycle status, logical time, topology revision, external levels, stateful and temporal state, pending events, and active diagnostic episodes.

`ObservableStateDigest` extends execution state with current output baselines, required current provenance roots or checkpoints, current explanation state, and current diagnostic evidence.

`SnapshotDigest` covers the complete canonical persisted snapshot, including optional retained provenance history and persisted checkpoint metadata.

These digest types MUST NOT be interchangeable.

### 19.4 Runtime policy

Semantically relevant execution limits are represented by an explicit immutable runtime policy:

```rust
pub struct RuntimePolicy { /* validated limits */ }
```

Examples include limits on internal reactions, evaluated operations, pending events, events created per transaction, and required provenance growth.

The canonical semantically relevant fields produce a `RuntimePolicyId`.

Budget exhaustion MUST reject the complete transaction atomically. The library MUST NOT skip required work or publish partial settlement.

Exact operational replay requires compatible semantic versions and the same `RuntimePolicyId` in addition to equivalent execution state and transactions.

## 20. Resolved handles

Stable keys support persistence and reconfiguration. Repeated runtime access may use revision-bound handles:

```rust
pub struct ResolvedInput<S> { /* opaque */ }
pub struct ResolvedOutput<S> { /* opaque */ }
pub struct ResolvedNode { /* opaque */ }
```

A resolved handle is valid only for one fingerprint and topology revision.

Using it against another network or stale topology revision MUST produce a structured error identifying the expected revision, actual revision, and stable key to resolve again.

Resolved handles MUST NOT silently refer to a different element after reconfiguration.

---

# Part IV — Logical time

## 21. Caller-owned time

Canonical time types are:

```rust
pub struct Time<D> { /* exact discrete logical instant */ }
pub struct Span<D> { /* non-negative integral tick count */ }
pub struct NonZeroSpan<D> { /* positive integral tick count */ }
```

`D` is a caller-defined time-domain marker. Different domains are type-incompatible. The caller defines what one tick means; `mossignal` does not interpret the unit.

The library MUST NOT read a clock, infer time from execution duration, advance automatically, sleep, or create timer threads.

Time need not begin at zero. A duration that must be positive MUST require `NonZeroSpan<D>`.

Time arithmetic MUST be checked and MUST NOT wrap. Overflow and invalid subtraction produce structured failures.

## 22. Time progression

The first successful transaction establishes an arbitrary initial logical time and initializes the machine.

Every later successful transaction advances the ready machine to one strictly later caller-supplied time.

All external changes intended for one logical time MUST be included in one transaction. A second independent transaction at the same time MUST be rejected, preventing call order from becoming hidden semantics.

The caller may jump directly across distant times. The processor evaluates only meaningful intervening pending deadlines.

Every newly scheduled temporal obligation MUST have a deadline strictly later than the reaction creating it. Immediate behavior is produced directly in the current reaction rather than inserted into the event calendar at the current time.

# Part V — Transactions and execution

## 23. Explicit transaction values

The atomic execution unit is:

```rust
pub struct Transaction<D> { /* opaque */ }
```

```rust
let tx = Transaction::at(time)
    .against(machine.revision())
    .with_input_snapshot(snapshot);

let result = machine.apply(tx)?;
```

A transaction MAY contain:

- expected topology revision;
- optional expected `ExecutionStateDigest` for state-sensitive freshness;
- one prepared topology patch;
- one reconfiguration state-loss policy when a patch is present;
- exactly one `InputSnapshot` or `InputDelta` according to lifecycle rules;
- caller metadata used only for diagnostics and replay.

The first transaction MUST contain a complete `InputSnapshot`; an `InputDelta` is invalid before initialization.

A transaction MUST be:

- atomic;
- deterministic;
- fully validated before commitment;
- all-or-nothing;
- inspectable before execution;
- representable as data.

A closure convenience API MAY exist, but it MUST build the same explicit transaction model.

## 24. Transaction ordering

### 24.1 First transaction

For initialization at time `T0`, the machine MUST:

1. validate lifecycle status, expected revision, runtime policy, complete input snapshot, and transaction structure;
2. finalize any prepared topology patch effective at `T0` against the declared initial state;
3. form one initialization stimulus batch from:
   - the complete external level snapshot;
   - external pulse occurrences at `T0`;
   - topology-induced facts;
4. evaluate the ordinary current-reaction dependency graph;
5. produce current outputs, proposed successor state, future obligations, provenance, and diagnostic-episode state;
6. commit initialization atomically.

There are no prior due events and no prior external output baselines.

### 24.2 Ready-machine transaction

For a transaction at time `T`, the ready machine MUST:

1. validate expected revision, optional expected execution digest, time progression, runtime policy, and transaction structure;
2. process every pending deadline strictly earlier than `T`, in chronological order, each equal-deadline batch as one reaction;
3. finalize and commit the topology patch effective at `T` into candidate state;
4. migrate, transform, cancel, or reject pending events according to the prepared rules, including events due exactly at `T`;
5. form one current-time stimulus batch from:
   - surviving pending events due exactly at `T`;
   - external pulse occurrences at `T`;
   - external level observations or changes at `T`;
   - topology-induced facts;
6. evaluate node reaction semantics over previous committed candidate state;
7. propagate until quiescent;
8. publish one immutable result and one complete new machine state.

Intervening deadline reactions are internal steps of the same outer transaction. They MUST NOT become independently visible or partially committed.

Events before `T` are evaluated under the previous topology. The patch is finalized against the actual candidate state reached immediately before `T`.

A temporal obligation due at `T` is determined from temporal state entering the reaction. Same-time current inputs modify or suppress that obligation only where the node kind explicitly declares an instantaneous dependency.

## 25. Atomic input semantics

All external input at one time is one unordered semantic batch.

Equivalent batches MUST produce equivalent results regardless of insertion order, collection iteration order, connection insertion order, binding order, or diagnostic metadata.

Nodes receiving simultaneous stimuli MUST use documented batch semantics.

## 26. Settlement

Every successful reaction MUST settle all immediate consequences before it can contribute to a committed outer transaction.

A result MUST NOT expose partial propagation, intermediate glitches, or provisional state writes.

Each state cell has one previous committed value and at most one proposed successor value per reaction. Proposed state commits only after the complete reaction succeeds.

If any internal deadline reaction, current-time reaction, migration, provenance construction, diagnostic update, digest computation, or budget check fails, the complete outer transaction MUST fail atomically and leave the published machine unchanged.

## 27. Canonical machine API

```rust
impl<D> Machine<D> {
    pub fn apply(
        &mut self,
        transaction: Transaction<D>,
    ) -> Result<TransactionResult<D>, RuntimeFailure>;

    pub fn forecast(
        &self,
        transaction: Transaction<D>,
    ) -> Result<ForecastResult<D>, RuntimeFailure>;

    pub fn status(&self) -> MachineStatus<D>;
    pub fn revision(&self) -> NetworkRevision;
    pub fn now(&self) -> Option<Time<D>>;

    pub fn schedule(&self)
        -> Result<Schedule<D>, LifecycleFailure>;

    pub fn next_deadline(&self)
        -> Result<Option<Time<D>>, LifecycleFailure>;

    pub fn runtime_policy_id(&self) -> RuntimePolicyId;
    pub fn execution_state_digest(&self) -> ExecutionStateDigest;
    pub fn observable_state_digest(&self) -> ObservableStateDigest;
}
```

`CompiledNetwork::spawn` MUST associate the machine with an explicit validated `RuntimePolicy`. An ergonomic named policy constructor MAY exist, but policy identity MUST remain observable.

Exact borrowing, generic details, and convenience methods may differ, but these responsibilities MUST remain.

# Part VI — Inputs and outputs

## 28. `InputSnapshot`

`InputSnapshot` is the complete authoritative state of all external level inputs at one time, plus pulse occurrences at that time.

```rust
pub struct InputSnapshot { /* network-bound, opaque */ }
```

A valid snapshot MUST:

- contain one level for every required external level input;
- reject unknown inputs;
- reject duplicate or conflicting observations;
- belong to the correct network identity or binding projection.

Absence of a pulse input means zero occurrences.

```rust
let snapshot = compiled.input_snapshot()
    .set(level_input, LogicLevel::High)?
    .pulse(pulse_input, PulseCount::new(2)?)?
    .finish()?;
```

The first successful transaction MUST use an `InputSnapshot`. It establishes the complete authoritative external valuation; no omitted level is inferred as `Low`.

## 29. `InputDelta`

`InputDelta` expresses changes relative to the external level valuation already held by a ready machine.

```rust
pub struct InputDelta { /* network-bound, opaque */ }
```

A delta may contain level changes and pulse occurrences. Unmentioned levels retain their previous values.

An `InputDelta` MUST be rejected while the machine is `AwaitingInitialization` because no previous authoritative external valuation exists.

Snapshots and deltas MUST be distinct, non-interchangeable types.

## 30. Input projection

The library SHOULD support prevalidated projection from caller-owned binding keys:

```rust
let projector = bindings.input_projector(&compiled)?;
let snapshot = projector.snapshot_from(observations)?;
```

Projection MUST diagnose missing, duplicate, unknown, ambiguous, wrong-kind, wrong-network, and stale-revision observations.

The projector MUST NOT interpret the caller’s object model.

## 31. `TransactionResult<D>` and semantic change sets

Every successful transaction produces one immutable semantic change set regardless of whether any observer or subscription exists.

```rust
pub struct SemanticChangeSet<D> {
    pub processed_times: Vec<Time<D>>,
    pub output_events: Vec<OutputEvent<D>>,
    pub state_changes: Vec<StateChange<D>>,
    pub topology_changes: Vec<TopologyChange>,
    pub diagnostic_episode_changes: Vec<DiagnosticEpisodeChange<D>>,
    pub provenance_root_changes: Vec<ProvenanceRootChange<D>>,
}
```

`processed_times` contains the chronological logical reaction times processed by the outer transaction, including intervening deadlines and the requested final time.

The successful result SHOULD expose:

```rust
pub struct TransactionResult<D> {
    pub requested_time: Time<D>,
    pub before_revision: NetworkRevision,
    pub after_revision: NetworkRevision,
    pub changes: SemanticChangeSet<D>,
    pub migration: Option<MigrationReport<D>>,
    pub diagnostics: DiagnosticSet,
    pub schedule: Schedule<D>,
    pub before_execution_digest: ExecutionStateDigest,
    pub after_execution_digest: ExecutionStateDigest,
    pub after_observable_digest: ObservableStateDigest,
    pub runtime_policy_id: RuntimePolicyId,
}
```

Concrete collection forms may differ. Convenience accessors MAY expose output events, state changes, or diagnostics directly.

Warnings and informational diagnostic events may appear in successful results. Active persistent diagnostic conditions remain separately inspectable.

## 32. `OutputEvent<D>`

```rust
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

Before initialization, a level output has no observable baseline. The first committed value therefore emits `LevelEstablished`, not a fabricated change from `Low` or from an unknown third level.

A newly added external level output likewise emits `LevelEstablished` when its first value settles.

A preserved level output emits `LevelChanged` only when its settled value differs from its prior published baseline.

Removing an output is reported as a topology consequence, not as a signal transition.

Pulse outputs preserve multiplicity, including pulses produced during initialization.

Events MUST be deterministically ordered as one flat network-wide chronological stream. Events sharing one time use a canonical representation order that MUST NOT be interpreted as pulse causality.

## 33. State-change reporting

`StateChange<D>` SHOULD cover:

- node internal state;
- level-port establishment and change;
- pulse activity for each completed reaction;
- pending-event creation, migration, transformation, cancellation, and firing;
- next-deadline changes;
- active diagnostic-episode changes;
- provenance-root changes;
- module aggregate state;
- region changes where relevant.

Each change SHOULD identify structural subject, prior state where applicable, new state, logical time, cause, and topology revision.

Pulse activity is reaction-scoped. An old pulse count MUST NOT be presented through later inspection as a persistent current signal value. Optional retained recent pulse history must be labeled as history and report its retention boundary.

# Part VII — Stateful and temporal semantics

## 34. Explicit memory primitives

The core MUST avoid vaguely specified generic memory nodes.

The built-in node specification defines the initial stateful catalogue. Primitive forms SHOULD have fixed, explicit port shapes and policies rather than optional reset ports or underspecified generic behavior.

The initial stateful language includes explicit edge-detector initialization, toggle state, pulse-controlled and level-controlled set/reset latches, and sample-and-hold behavior.

Each kind MUST define simultaneous-input behavior, initialization, current output law, proposed successor-state law, inspection, causality, persistence, and reconfiguration.

A conflict-rejecting or conflict-diagnosing latch MUST state exactly whether conflict rejects the transaction or retains prior state while maintaining a persistent diagnostic episode.

## 35. Edge detection

```text
RisingEdge   Low -> High emits pulse
FallingEdge  High -> Low emits pulse
AnyEdge      either transition emits pulse
```

Repeating the same level emits nothing.

Initial observation semantics MUST explicitly choose between baseline establishment and comparison against a configured initial level.

## 36. Pulse multiplicity

Every pulse-consuming node MUST define whether it preserves, coalesces, consumes, bounds, or transforms multiplicity.

No node may silently discard multiplicity.

## 37. Pulse delay

A pulse delay schedules every occurrence for a future time.

Pending pulse state MUST retain deadline, multiplicity, originating cause, originating subject, scheduling node, and revision context.

## 38. Level delay

The library MUST distinguish:

- **transport delay**, which reproduces every transition after a duration;
- **inertial delay**, which reproduces a transition only if the new level remains stable for the duration.

These MUST be distinct node kinds or explicit visible policies.

## 39. Periodic behavior

Periodic nodes MUST define period, enabling condition, first-emission phase, disable behavior, re-enable behavior, phase preservation, large-jump behavior, and multiplicity when several periods elapse.

Standard policies SHOULD include:

```text
Immediate
AfterFirstPeriod
PreservePhase
RestartPhase
```

## 40. Large time jumps

Advancing directly from `A` to `B` MUST be observationally equivalent to processing every intervening deadline chronologically.

The caller need not invoke empty intermediate times.

---

# Part VIII — Diagnostics

## 41. Structured diagnostics

Diagnostics MUST be first-class structured data:

```rust
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,
    pub summary: DiagnosticMessage,
    pub primary: SubjectRef,
    pub related: Vec<RelatedSubject>,
    pub evidence: Vec<Evidence>,
    pub suggestions: Vec<Suggestion>,
}
```

Rendered text is not authoritative.

The diagnostic model distinguishes:

- one-time validation or runtime findings;
- emitted diagnostic events;
- persistent active diagnostic conditions.

A persistent condition is represented as a semantic episode rather than emitted again on every unrelated transaction.

## 42. Stable codes

Every category MUST have a stable code suitable for documentation, filtering, automated handling, regression tracking, editor integration, and support reports.

Codes MUST remain stable when wording changes.

## 43. Subjects and evidence

A subject MUST be able to identify a network, region, module, node, port, connection, endpoint, binding, pending event, snapshot, transaction, revision, or resolved handle.

Evidence MAY include signal kinds, directions, parameters, graph paths, cycle witnesses, times, revisions, fingerprints, pending-event details, and migration consequences.

## 44. Suggestions

Machine-readable suggestions MAY be included only when a correction is unambiguous.

The library MUST NOT invent speculative fixes.

## 45. Reports

Validation, compilation, binding, and structural patch preparation SHOULD use:

```rust
pub struct Report<T> {
    pub artifact: Option<T>,
    pub diagnostics: DiagnosticSet,
}
```

Independent findings SHOULD be collected where safe.

The artifact MUST be absent when blocking diagnostics remain.

Diagnostic ordering MUST be deterministic.

## 46. Required validation coverage

Validation MUST cover:

- unknown or duplicate keys;
- missing nodes and ports;
- invalid direction;
- signal-kind mismatch;
- unsupported multiple drivers;
- missing required inputs;
- invalid arity;
- invalid module interface conformance;
- cycles in the current-reaction dependency graph;
- invalid or incomplete dependency signatures;
- invalid timing parameters;
- invalid initial state;
- duplicate or ambiguous bindings;
- malformed hierarchy;
- incompatible network references;
- incompatible state schema.

Current-reaction cycle diagnostics SHOULD include a cycle witness expressed through structural nodes, ports, and per-node dependency edges.

Non-blocking diagnostics SHOULD cover unreachable outputs, unused inputs, isolated nodes, constant outputs, redundant connections, empty modules, and deprecated node forms.

## 47. Lifecycle-wide quality

The same diagnostic model MUST apply to authoring, validation, compilation, binding, input projection, initialization, execution, inspection, explanation, snapshots, restoration, replay, patch preparation, patch finalization, stale handles, forecasts, runtime-policy failures, and observer resynchronization.

Runtime failures MUST NOT degrade into unstructured strings.

### 47.1 Persistent diagnostic episodes

A persistent diagnostic condition has stable semantic identity derived from facts such as:

```text
diagnostic code
primary structural subject
condition discriminator
```

Conceptually an episode is:

```text
Inactive

Active {
    began_at,
    current_evidence,
    last_material_change,
}
```

A diagnostic event is emitted when an episode begins, materially changes, and optionally when it resolves.

Active episode state affects future diagnostic emission and therefore MUST participate in snapshots, restoration, replay, execution-state digests, inspection, and reconfiguration.

An episode MUST NOT attach to a different subject because a dense runtime slot was reused.

# Part IX — Inspection and explanation

## 48. Inspection is core

Every semantically relevant element MUST be inspectable.

Structural inspection is available before initialization and includes topology, node definitions, declared initial state, metadata, modules, regions, and graph structure.

Current runtime inspection is available only for a ready machine. Before initialization, requests for current port values, current outputs, pending events, schedules, active runtime diagnostics, or current explanations MUST return a structured not-initialized failure.

Inspection MUST support networks, regions, modules, nodes, ports, connections, endpoints, bindings, pending events, active diagnostic episodes, provenance checkpoints, and transaction results.

## 49. Direct inspection

Canonical methods SHOULD include:

```rust
impl<D> Machine<D> {
    pub fn inspect_node(
        &self,
        node: NodeKey,
    ) -> Result<NodeInspection<D>, InspectionFailure>;

    pub fn inspect_output<S>(
        &self,
        output: ExternalOutputKey<S>,
    ) -> Result<OutputInspection<S, D>, InspectionFailure>;

    pub fn inspect_pending(
        &self,
        event: PendingEventKey,
    ) -> Result<PendingEventInspection<D>, InspectionFailure>;
}
```

## 50. Node inspection

A ready-machine node inspection MUST expose, where applicable:

- key and kind;
- current level input and output values;
- pulse activity in the most recent retained reaction, explicitly labeled as history;
- internal and declared initial state;
- last committed state transition;
- pending events;
- next deadline;
- current causal justification;
- active persistent diagnostic conditions;
- provenance checkpoint boundary;
- metadata;
- containing module and region;
- topology revision and logical time.

Built-in node families SHOULD have typed inspection variants. A schema-driven dynamic view MAY also exist, but must preserve equivalent information.

Inspection MUST distinguish current port values, stored state, pending work, and retained history. In particular, a pulse has no persistent current value after its reaction completes.

## 51. Stable inspection queries and compiled plans

The library SHOULD distinguish stable inspection intent from revision-bound dense access:

```rust
pub struct InspectionQuery<D> { /* stable structural keys and requested fields */ }
pub struct InspectionPlan<D> { /* compiled revision-bound access paths */ }
```

```rust
let query = InspectionQuery::builder()
    .node(latch_key)
    .output(output_key)
    .pending_for(delay_key)
    .explanation(output_key)
    .finish()?;

let plan = compiled.compile_inspection(&query)?;
let snapshot = machine.inspect(&plan)?;
```

A stable query MAY survive topology revisions and be recompiled.

A compiled plan is tied to a fingerprint and topology revision. It MUST fail structurally when stale and MUST NOT silently retarget another element.

## 52. Inspection subscriptions and observer state

Subscriptions and their delivery state live outside semantic `Machine` state.

An observer layer MAY own:

- stable inspection queries;
- compiled inspection plans;
- last delivered projections;
- delivery cursors based on committed execution-state identity;
- retained semantic change sets;
- acknowledgement state;
- delivery queues;
- resynchronization state.

Creating, removing, delaying, or failing a subscription MUST NOT alter machine execution, scheduling, migration, fingerprints, semantic snapshots, semantic digests, or provenance.

Incremental subscriptions consume committed semantic change sets and return only changes relevant to their query.

They MUST:

- preserve deterministic order;
- detect incompatible revisions;
- detect missed or expired retained history;
- support complete resynchronization from a fresh inspection projection;
- track explanation dependencies when explanation data is requested, even if the observed signal value remains unchanged.

Observer delivery occurs after semantic commit. Delivery failure MUST NOT roll back the machine.

## 53. Causal provenance

Every externally observable establishment or change MUST have a cause.

```rust
pub struct CauseRef { /* opaque */ }
```

A cause SHOULD identify originating transaction, originating external input or timer or topology event, immediate predecessors, logical time, and topology revision.

Causal provenance is a structured acyclic derivation graph, not merely an execution log. Joint support MAY be represented by grouped or hyperedge-like derivations where a result depends on several unordered facts.

Current combinational support is distinct from the cause of the most recent transition. Current support may be reconstructed from compiled reaction structure and current settled values. Explicit retained provenance is required where the current fact depends on history that cannot be reconstructed.

The machine MUST retain enough provenance to explain:

- current stateful state;
- current external level outputs;
- every pending event;
- every output event;
- each inspectable signal's latest retained transition;
- migration and reset consequences;
- active persistent diagnostic evidence where causally relevant.

### 53.1 Authoritative roots

An explanation may terminate at an authoritative root such as:

- declared initial state;
- external input observation;
- committed caller transaction;
- retained temporal origin;
- topology migration or reset;
- snapshot checkpoint;
- explicit provenance checkpoint.

An unexplained previous value is not a valid terminal root.

### 53.2 Provenance checkpoints

Older ancestry MAY be compacted into an explicit checkpoint establishing semantic facts at a logical time and topology revision.

Checkpointing may reduce historical depth but MUST NOT leave current facts unexplained.

Migration provenance SHOULD derive a new fact from the old fact and explicit migration rule. Reset state receives an explicit reset or initialization cause.

## 54. Explanation API

```rust
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

```rust
pub fn explain(
    &self,
    request: Explain<D>,
) -> Result<Explanation<D>, ExplanationFailure>;
```

## 55. Explanation result

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

```rust
pub enum RetentionStatus<D> {
    CompleteFromInitialization,
    CompleteFromCheckpoint {
        checkpoint_time: Time<D>,
        checkpoint_revision: NetworkRevision,
    },
    IncompleteForRequestedHistory {
        retained_since: Time<D>,
    },
}
```

Explanation data is structured and independent from prose rendering.

Combinational explanations derive results from current upstream state.

Stateful explanations distinguish the event establishing state, conditions preserving it, conflicting or rejected inputs, and reset or overwrite policy.

Temporal explanations expose deadline, remaining span, originating cause, scheduling node, cancellation conditions, and coalescing or replacement policy.

“Why not?” explanations SHOULD identify unsatisfied prerequisites, blockers, pending conditions, conflicts, disconnections, disabled conditions, and nearest relevant paths.

A historical request such as `MissingPulse { since }` may claim complete non-occurrence only over retained history. The result MUST expose any checkpoint or retention boundary.

The library MUST NOT claim a cause, non-occurrence, or correction it cannot derive.

# Part X — Reconfiguration

## 56. `NetworkPatch<D>`

Topology changes are explicit values:

```rust
pub struct NetworkPatch<D> { /* opaque */ }
```

A patch may add or remove nodes, connections, modules, and endpoints; change semantic parameters; and modify hierarchy or metadata.

It MUST identify its expected base revision.

## 57. Structural patch preparation

Patch handling is divided into reusable structural preparation and state-dependent transaction-time finalization.

```rust
pub struct PreparedPatch<D> { /* topology-bound migration program */ }

let prepared = machine
    .prepare_patch(patch)
    .require_artifact()?;
```

Structural preparation MUST:

- validate the graph rewrite;
- compile the proposed topology;
- establish stable-key correspondence;
- validate the new current-reaction dependency graph;
- perform static compatibility analysis;
- construct state migration functions;
- construct pending-event migration rules;
- classify unavoidable and conditional semantic loss;
- identify resolved handles and compiled inspection plans requiring re-resolution.

A prepared patch SHOULD expose:

- base topology revision;
- proposed topology revision;
- resulting fingerprint;
- preserved structural subjects;
- static `Preserve`, `Migrate`, `Reset`, or `Reject` classifications where decidable;
- conditional compatibility and state-loss predicates;
- pending-event migration rules;
- region merges and splits;
- invalidated resolved artifacts;
- diagnostics.

Structural preparation MUST NOT claim exact future state values, exact pending-event outcomes, or exact output changes when those depend on runtime state at the patch's effective time.

## 58. Prepared-patch freshness and exact preview

A prepared patch is bound to its exact base topology revision and semantic definition.

Ordinary state-only transactions do not invalidate it. A topology revision change does.

At effective time `T`, the outer transaction processes all earlier deadlines under the old topology, obtains the actual candidate state immediately before `T`, and only then finalizes migration and state-loss outcomes.

An exact preview of a patch at a future time is a `forecast`, not structural preparation.

An exact patch forecast is bound to:

- base topology revision;
- base `ExecutionStateDigest`;
- effective time;
- `RuntimePolicyId`.

Committing later requires a new explicit transaction against the then-current revision and, where exact freshness is required, the expected execution digest.

## 59. State compatibility

Every stateful and temporal kind MUST define compatibility across revisions.

```rust
pub enum Compatibility {
    Preserve,
    Migrate,
    Reset,
    Reject,
}
```

Compatibility may depend on kind, state schema, semantic parameters, module identity, port shape, bounds, timing policy, current value, and pending-event payload.

Structural preparation MUST determine the compatibility rule and every possible outcome. State-dependent actual outcomes are finalized at the patch effective time.

## 60. Preservation rules

When a stable node key survives and definitions are compatible, state MUST be preserved.

Region merge or split MUST NOT reset state by itself.

Connection changes preserve unrelated state.

New nodes receive initial state.

Removed nodes lose state only through an explicitly reported consequence.

## 61. Pending-event migration

Every temporal kind MUST define pending-event behavior when the node is removed, duration changes, connections change, semantic kind changes, compatible identity survives, or containing module changes.

Every pending event receives one explicit outcome:

```text
PreserveDeadline
RecomputeDeadline
TransformPayload
Cancel
Reject
```

No pending event may disappear silently.

Static preparation defines the migration rule. Transaction-time finalization applies it to the actual event set present at the effective time, including events due exactly then.

## 62. State-loss policy

```rust
pub enum ReconfigurationPolicy {
    RejectStateLoss,
    AllowReportedStateLoss,
}
```

`RejectStateLoss` prevents commitment if the finalized semantic loss set is non-empty.

Semantic loss may include removed or reset stored state, canceled pending work, terminated required provenance ancestry, and removed output baselines.

`AllowReportedStateLoss` permits commitment only when every possible loss is classified during structural preparation and every actual loss appears in the committed result.

## 63. Atomic commitment

A prepared patch commits only through a transaction:

```rust
let tx = Transaction::at(time)
    .against(machine.revision())
    .with_patch(prepared)
    .with_reconfiguration_policy(
        ReconfigurationPolicy::RejectStateLoss,
    )
    .with_input_delta(delta);

let result = machine.apply(tx)?;
```

The transaction finalizes actual state and event migration against the state reached at the patch time, installs the candidate topology, settles topology-induced reaction changes, and publishes one atomic result.

A topology patch is itself a cause. It MUST NOT be represented as a synthetic pulse or level.

For a preserved level output, the new settled value is compared with its old published baseline. A newly added level output emits `LevelEstablished`. A removed output produces a topology consequence.

A failed transaction leaves topology, revision, logical time, external levels, node state, temporal state, pending events, output baselines, provenance roots, diagnostic episodes, and execution digest unchanged.

# Part XI — Modules and graph queries

## 64. Modules

```rust
pub struct ModuleDef<D> { /* opaque */ }
pub struct ModuleInstanceKey { /* opaque */ }
```

A module has typed public inputs and outputs, private internal structure, parameters, metadata, a module fingerprint, and compatibility rules.

Modules may be nested.

Internal keys MUST remain stable across compatible module revisions.

Private internals MUST NOT be directly connectable from outside unless exported. Encapsulation affects authoring access, not diagnostics or explanations.

Elements SHOULD have hierarchical diagnostic paths. Paths are metadata, not structural identity.

Updating a module definition MUST produce structural patch preparation for affected instances, including state-compatibility rules, interface and binding consequences, pending-event migration rules, and diagnostic-path consequences. Exact runtime outcomes require forecast or commitment at an effective time.

## 65. Read-only graph view

```rust
pub struct GraphView<'a, D> { /* immutable borrowed view */ }
```

It preserves keys and hierarchy and supports visualization, diagnostics, navigation, and dependency analysis.

It is not an independently mutable copied graph.

## 66. Region and dependency queries

Canonical queries SHOULD include:

```rust
compiled.regions();
compiled.region_containing(node_key);
compiled.region_containing_output(output_key);

compiled.slice_affecting(output_key);
compiled.slice_affected_by(input_key);
compiled.slice_between(source, destination);
```

The graph API SHOULD answer which inputs can affect an output, which outputs an input can affect, whether an output is reachable, and which stateful, temporal, and module elements lie on relevant paths.

These are structural possibility queries, not claims about current activation.

---

# Part XII — Scheduling, forecasting, persistence, and replay

## 67. Scheduling

Every successful transaction on a ready machine exposes `Schedule::Dormant` or `Schedule::WakeAt(time)`.

`WakeAt` is the earliest non-canceled pending deadline capable of changing state without new external input.

`next_deadline()` and `schedule()` MUST NOT mutate state.

Calling them before initialization MUST return a structured not-initialized failure rather than `Dormant`.

The caller need not invoke the machine at every integer time step. Invoking only on external input, topology changes, or reported deadlines remains semantically correct.

## 68. Forecasting

```rust
let forecast = machine.forecast(transaction)?;
```

A forecast executes the same deterministic transition function as `apply` on unpublished candidate state.

It MUST use the same:

- initialization rules;
- reaction evaluation;
- deadline processing;
- patch finalization;
- migration;
- provenance;
- diagnostic episodes;
- runtime policy and budget behavior;
- failure semantics.

It does not mutate the original machine and returns equivalent result information.

A forecast is explicitly hypothetical and cannot silently become real state.

It is bound to the base topology revision, base `ExecutionStateDigest`, requested time, and `RuntimePolicyId`. Equivalent commitment requires a new explicit transaction whose preconditions still hold.

## 69. Snapshots

```rust
pub struct MachineSnapshot<D> { /* complete semantic state */ }
```

A snapshot MUST contain the complete semantic state sufficient to determine future behavior, including:

- lifecycle status;
- logical time when ready;
- topology revision;
- network fingerprint;
- semantic and snapshot-schema versions;
- authoritative external levels when ready;
- current settled level values when ready;
- stateful-node state;
- temporal-node state;
- pending event calendar;
- external level-output baselines and establishment state;
- required causal roots or provenance checkpoints;
- active persistent diagnostic episodes;
- `ExecutionStateDigest`;
- `ObservableStateDigest`;
- `RuntimePolicyId` where exact operational restoration requires it;
- an attached `SnapshotDigest` computed over the canonical snapshot payload, excluding the digest field itself.

An uninitialized machine snapshot MUST be representable. It contains declared stored state and lifecycle status but no fabricated current time, external valuation, output baseline, pending schedule, or current runtime explanation.

Snapshots MUST NOT depend on resolved handles, dense indices, heap positions, worklists, compiled inspection plans, or observer delivery state.

## 70. Restoration

```rust
let machine = compiled.restore(snapshot, runtime_policy)?;
```

Restoration is validation, not deserialization alone.

It MUST validate:

- lifecycle consistency;
- fingerprint and topology revision compatibility;
- state schema and semantic versions;
- stable subject existence;
- external valuation completeness where ready;
- pending-event ownership and strictly future deadlines;
- output baseline consistency;
- required provenance roots, checkpoints, and acyclicity;
- active diagnostic episodes;
- runtime-policy compatibility where exact operational restoration is requested;
- canonical digest consistency.

Ordinary restoration MUST NOT silently migrate topology, reset incompatible state, infer missing inputs as `Low`, or trust persisted dense runtime indices.

Incompatible restoration MUST fail precisely and leave no partially constructed published machine.

## 71. Replay

```rust
pub struct ReplayFrame<D> {
    pub expected_previous_execution_digest: ExecutionStateDigest,
    pub expected_revision: NetworkRevision,
    pub runtime_policy_id: RuntimePolicyId,
    pub transaction: Transaction<D>,
    pub resulting_execution_digest: ExecutionStateDigest,
}
```

A replay frame contains enough information to reproduce execution without hidden caller state.

Replay is repeated application of the same deterministic transition function used by `apply` and `forecast`.

The same compatible snapshot, semantic versions, runtime policy, and replay sequence MUST produce equivalent:

- output events, including level establishments;
- state changes;
- topology revisions;
- diagnostic events and active episodes;
- provenance roots and checkpoints;
- schedules;
- final execution and observable digests.

Replay compatibility diagnostics MUST distinguish wrong network, stale topology revision, wrong semantic version, wrong runtime policy, missing element, incompatible schema, invalid pending event, corrupted snapshot, stale transaction, and digest mismatch.

Replay concatenation MUST hold wherever every transaction remains valid at its application point:

```text
Replay(M, A ++ B) == Replay(Replay(M, A), B)
```

Canonical digest encoding MUST be independent of dense-index assignment, allocation order, hash iteration, heap shape, and presentation metadata.

# Part XIII — Extensibility boundaries

## 72. Closed evaluator boundary

The core MUST NOT expose unrestricted callback-based custom nodes.

```rust
trait CustomNode {
    fn evaluate(&mut self, host: &mut dyn Any);
}
```

The above semantic shape is forbidden.

Evaluation must remain deterministic, host-independent, inspectable, persistable, explainable, reconfigurable, and replayable.

## 73. No arbitrary payloads

Signals carry only `Level` or `Pulse`.

Application-specific information belongs in external bindings or metadata.

## 74. Composition before primitives

New behavior SHOULD first be expressed through existing primitives and modules.

A new primitive is justified only when composition cannot provide precise atomic semantics, correct migration, adequate inspection, useful explanation, stable persistence, or clear diagnostics.

## 75. Requirements for new node kinds

Every new node kind MUST define:

- typed ports;
- semantic parameters;
- declared initial state;
- current-reaction dependency signature;
- current output law over previous state, settled current inputs, and due obligations;
- proposed successor-state law;
- simultaneous-input semantics;
- pulse multiplicity semantics;
- exact-deadline semantics where temporal;
- strictly-future scheduling behavior;
- inspection schema;
- current-support and why-not explanation semantics;
- transition and migration causality;
- snapshot schema;
- state compatibility;
- pending-event migration;
- persistent diagnostic-condition behavior;
- diagnostics.

A kind unable to satisfy these requirements MUST NOT enter the core.

# Part XIV — End-to-end API example

```rust
struct Ticks;

let mut net = NetworkBuilder::<Ticks>::new();

let (set_key, set) = net.pulse_input("set");
let (reset_key, reset) = net.pulse_input("reset");

let stored = net.reset_dominant_latch(
    set,
    reset,
    LogicLevel::Low,
);

let state_key = net.level_output("state", stored);

let validated = net.finish().require_artifact()?;
let compiled = validated.compile().require_artifact()?;

let policy = RuntimePolicy::builder().finish()?;
let mut machine = compiled.spawn(policy.clone());

assert!(matches!(
    machine.status(),
    MachineStatus::AwaitingInitialization,
));

let initial = compiled
    .input_snapshot()
    .pulse(set_key, PulseCount::ONE)?
    .finish()?;

let init_tx = Transaction::at(Time::new(100))
    .against(machine.revision())
    .with_input_snapshot(initial);

let init_result = machine.apply(init_tx)?;

assert!(init_result.changes.output_events.iter().any(|event| {
    matches!(
        event,
        OutputEvent::LevelEstablished {
            output,
            value: LogicLevel::High,
            ..
        } if *output == state_key
    )
}));

let delta = compiled
    .input_delta()
    .pulse(reset_key, PulseCount::ONE)?
    .finish()?;

let tx = Transaction::at(Time::new(101))
    .against(machine.revision())
    .with_input_delta(delta);

let result = machine.apply(tx)?;

assert!(result.changes.output_events.iter().any(|event| {
    matches!(
        event,
        OutputEvent::LevelChanged {
            output,
            from: LogicLevel::High,
            to: LogicLevel::Low,
            ..
        } if *output == state_key
    )
}));

let inspection = machine.inspect_output(state_key)?;
assert_eq!(inspection.level(), LogicLevel::Low);

let explanation = machine.explain(
    Explain::CurrentOutput(state_key.into()),
)?;

let execution_digest = machine.execution_state_digest();
let observable_digest = machine.observable_state_digest();
let snapshot = machine.snapshot();
let restored = compiled.restore(snapshot, policy)?;

assert_eq!(
    execution_digest,
    restored.execution_state_digest(),
);

assert_eq!(
    observable_digest,
    restored.observable_state_digest(),
);
```

Bindings remain separate:

```rust
let bindings = BindingSet::builder(&compiled)
    .bind_input(set_key, ExternalInputId::Set)?
    .bind_input(reset_key, ExternalInputId::Reset)?
    .bind_output(state_key, ExternalOutputId::StoredState)?
    .finish()?;
```

The evaluator does not inspect the external key types.

# Part XV — Required quality properties

## 76. Determinism

Equivalent network, lifecycle state, logical times, topology transactions, external input transactions, runtime policy, and semantic versions MUST produce equivalent observable results.

Any valid topological evaluation order of the acyclic current-reaction dependency graph MUST produce equivalent settled semantics.

## 77. Strong typing

The typed API MUST encode signal kind, endpoint direction, and time domain wherever statically knowable.

Dynamic data receives equivalent runtime validation.

Types exist to encode meaningful invariants, not to compress syntax.

## 78. Precise time and initialization semantics

All temporal behavior is expressed in exact discrete caller-owned logical time.

No behavior depends on real execution speed.

A machine has an explicit uninitialized phase. Its first transaction requires one complete authoritative input snapshot and establishes output baselines through `LevelEstablished` events.

## 79. State preservation

Compatible state survives topology revisions.

Incompatible changes are planned, reported, and policy-controlled.

No semantic state or pending work is lost silently.

## 80. Complete inspectability

Every stateful and temporal behavior is inspectable.

A caller can determine current value, internal state, latest transition, pending work, next deadline, and current cause.

## 81. Causal explainability

The library explains current output and stored state through structured causal information.

It SHOULD explain why a desired result does not currently hold.

## 82. Diagnostic quality

Diagnostics are structured, stable, deterministic, attached to precise subjects, usable by non-textual tooling, and available throughout the lifecycle.

Persistent conditions are represented as inspectable semantic episodes and do not emit unchanged warnings repeatedly.

## 83. Transactional consistency

Initialization, chronological deadline advancement, topology finalization, input update, state migration, reaction evaluation, provenance, diagnostic episodes, digest computation, and publication follow one atomic transaction model.

Failure at any preparation stage leaves the published machine exactly unchanged.

## 84. Domain neutrality

The evaluator knows nothing about caller objects, actions, or vocabulary.

External meaning exists only through opaque bindings and diagnostic metadata.

## 85. Replayability

A compatible snapshot, runtime policy, and transaction history reproduce execution exactly.

Execution, observable, and snapshot digest scopes remain distinct and are used only for their defined purposes.

## 86. No hidden global state

Identity allocation, time, scheduling, and execution do not depend on global mutable state.

## 87. API coherence

Initialization, execution, inspection, explanation, reconfiguration, persistence, graph queries, diagnostics, and observer change sets describe one shared semantic model.

They MUST NOT become adjacent subsystems with incompatible identity, lifecycle, state, causality, or retention concepts.

# Part XVI — Deferred specifications

The following are intentionally deferred:

- testing and verification policy;
- exhaustive and property-based test requirements;
- reference evaluator requirements;
- fuzzing policy;
- model checking;
- declarative temporal contracts;
- constrained user-defined automata;
- editor-specific interaction design;
- visualization formats;
- implementation architecture;
- performance targets;
- memory layout;
- parallel execution;
- platform and feature-flag policy.

Any future addition must preserve the semantics and boundaries defined here.

---

# Summary

`mossignal` is an inspectable, causally explainable, state-preserving signal machine with an explicit lifecycle.

Its defining properties are:

```text
typed Level and Pulse signals
explicit uninitialized and ready phases
complete first-snapshot initialization
LevelEstablished output semantics
caller-owned exact discrete logical time
acyclic current-reaction dependencies
dependency-specific state and temporal barriers
explicit atomic transactions
deterministic glitch-free settlement
chronological internal deadline reactions
automatic region discovery
stable structural keys
separate external bindings
revision-safe resolved handles
two-stage topology patch preparation and finalization
state-preserving reconfiguration
structured runtime inspection
stable queries and revision-bound inspection plans
first-class why and why-not explanations
authoritative provenance roots and checkpoints
persistent diagnostic episodes
observer-state separation
explicit dormancy and scheduling
explicit runtime policy and atomic budget failure
execution, observable, and snapshot digest scopes
snapshots and deterministic replay
strict domain neutrality
```

The library does not merely calculate outputs. It maintains a coherent account of:

- whether the machine has been initialized;
- what the network currently believes;
- what was established or changed;
- when each reaction occurred;
- why current state and pending work exist;
- why an expected result did not occur;
- what is scheduled to happen;
- what a topology edit can preserve structurally;
- what it actually preserves when committed at a particular state and time;
- where retained causal history begins;
- how execution can be reproduced exactly.

Every public feature must reinforce that single semantic model.
