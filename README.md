# Mossignal

Mossignal is a deterministic, host-agnostic Rust library for authoring,
validating, compiling, executing, inspecting, explaining, and eventually
persisting discrete signal networks.

This README is an orientation document for agents and maintainers. It describes
the architecture that exists in the repository today, where to find it, the
semantic boundaries that must be preserved, and the parts that are specified
but not implemented yet. It is deliberately project-specific; repository-wide
workflow and agent policy live in [`AGENTS.md`](AGENTS.md).

## The shortest useful summary

Mossignal models a typed graph of two closed signal kinds:

- `Level`: persistent binary state, represented by `LogicLevel::Low` or
  `LogicLevel::High`.
- `Pulse`: a reaction-scoped occurrence with an exact non-negative
  multiplicity, represented by `PulseCount`.

The caller owns topology, input observations, logical time, and runtime policy.
Mossignal owns validation, deterministic synchronous settlement, stateful-node
state, temporal obligations, causal provenance, structured diagnostics, and
transactional publication.

The primary lifecycle is:

```text
NetworkBuilder / ModuleBuilder
        |
        v
lossless unchecked authored definition
        |
        v
validated definition
  - structural diagnostics
  - current-reaction dependency graph
  - cycle rejection
  - semantic fingerprints
        |
        v
immutable CompiledNetwork
  - dense runtime indices
  - topological operation plan
  - state layouts
  - temporal descriptors
  - retained module correspondence
        |
        v
mutable Machine
  - lifecycle and logical time
  - external levels
  - stateful and temporal state
  - pending event calendar
  - provenance and diagnostic episodes
```

The most important implementation rule is that authored stable identities and
compiled dense positions are different things. Stable keys are used for public
identity, diagnostics, bindings, fingerprints, and future persistence. Dense
indices are private, revision-local execution machinery.

## Repository map

### Product code

The crate is [`crates/mossignal`](crates/mossignal) in a one-member Cargo
workspace.

| Path | Responsibility |
| --- | --- |
| [`src/lib.rs`](crates/mossignal/src/lib.rs) | Module declarations and the public re-export surface. Several implementation modules are private but their API types are re-exported here. |
| [`src/signal.rs`](crates/mossignal/src/signal.rs) | Closed `Level`/`Pulse` type universe, `LogicLevel`, `PulseCount`, and checked pulse arithmetic. |
| [`src/key.rs`](crates/mossignal/src/key.rs) | Strongly typed stable keys for networks, nodes, connections, ports, external endpoints, module interfaces, instances, and signal sources. |
| [`src/time.rs`](crates/mossignal/src/time.rs) | Caller-defined logical time domains, checked `Time`, `Span`, and `NonZeroSpan` arithmetic. |
| [`src/metadata.rs`](crates/mossignal/src/metadata.rs) | Human-readable metadata and caller-owned source correlation. Metadata is presentation data, not semantic identity. |
| [`src/authored.rs`](crates/mossignal/src/authored.rs) | Lossless unchecked network/module definitions, node kinds, port roles, endpoints, connections, and module bindings. |
| [`src/builder.rs`](crates/mossignal/src/builder.rs) | Typed `NetworkBuilder` and `ModuleBuilder`, builder-scoped signals, primitive constructors, module instantiation, standard-module conveniences, and authoring failures. |
| [`src/node_schema.rs`](crates/mossignal/src/node_schema.rs) | Private semantic schema for every primitive: arity, port roles, signal kinds, current-reaction dependencies, state families, and temporal families. |
| [`src/validation.rs`](crates/mossignal/src/validation.rs) | Structural validation, module validation support, current-reaction graph construction, SCC cycle detection, deterministic witnesses, and normalized validated definitions. |
| [`src/module.rs`](crates/mossignal/src/module.rs) | Immutable validated `ModuleDef`, module origins, public interfaces, graph views, and qualified identities for nested contents. |
| [`src/identity.rs`](crates/mossignal/src/identity.rs) | Canonical semantic encodings and BLAKE3 fingerprints for networks, input schemas, modules, and their standard variants. |
| [`src/compile.rs`](crates/mossignal/src/compile.rs) | Dense immutable executable topology, module lowering, operation descriptors, adjacency, initial state layout, and the full reference evaluator. |
| [`src/input.rs`](crates/mossignal/src/input.rs) | Network-bound complete `InputSnapshot` and partial `InputDelta` builders. |
| [`src/policy.rs`](crates/mossignal/src/policy.rs) | Required runtime budgets and canonical `RuntimePolicyId`. |
| [`src/machine.rs`](crates/mossignal/src/machine.rs) | Mutable machine lifecycle, semantic store, scheduling, output/state/module inspections, and inspection failures. |
| [`src/transaction.rs`](crates/mossignal/src/transaction.rs) | Initialization and advancement transactions, temporal execution, output events, provenance, budget enforcement, runtime failures, and publication. |
| [`src/episode.rs`](crates/mossignal/src/episode.rs) | Persistent diagnostic episode identity and begin/change/resolve transitions for retained level-latch conflicts. |
| [`src/diagnostics.rs`](crates/mossignal/src/diagnostics.rs) | The catalogue-backed diagnostic kernel: codes, severity, responsibility, evidence schemas, problems, reports, occurrences, and deterministic collections. |
| [`src/standard.rs`](crates/mossignal/src/standard.rs) | The current standard-module catalogue, canonical primitive expansions, descriptor identities, standard inspections, and explanations. |
| [`src/binding.rs`](crates/mossignal/src/binding.rs) | Application-facing identifier bindings, input projection, output projection, and the delegating `BoundMachine` façade. |

### Specifications and planning

Use [`AGENTS.md`](AGENTS.md) for repository authority and workflow policy. The
files below are the project-specific places to research semantics, architecture,
verification, and planned work.

| Path | Use |
| --- | --- |
| [`docs/specs/api_and_semantics_spec.md`](docs/specs/api_and_semantics_spec.md) | Public semantic contract: signal meaning, lifecycle, transactions, diagnostics, inspection, bindings, persistence boundaries, and reconfiguration API intent. |
| [`docs/specs/built_in_node_semantics.md`](docs/specs/built_in_node_semantics.md) | Laws and causal semantics of built-in primitives. |
| [`docs/specs/processor_and_runtime_architecture.md`](docs/specs/processor_and_runtime_architecture.md) | Internal architecture: compiled program versus mutable store, reaction evaluation, temporal execution, atomicity, provenance, diagnostics, persistence, and future optimization boundaries. |
| [`docs/specs/concrete_rust_api_surface.md`](docs/specs/concrete_rust_api_surface.md) | Intended Rust API shapes and public type responsibilities. Some later sections describe future APIs not present in the current source. |
| [`docs/specs/persistence_canonical_encoding_and_compatibility_spec.md`](docs/specs/persistence_canonical_encoding_and_compatibility_spec.md) | Canonical encoding, digest, snapshot, and compatibility requirements for future persistence work. |
| [`docs/specs/reconfiguration_and_topology_patch_spec.md`](docs/specs/reconfiguration_and_topology_patch_spec.md) | Future topology-patch, correspondence, migration, and atomic replacement semantics. |
| [`docs/specs/standard_module_catalogue_spec.md`](docs/specs/standard_module_catalogue_spec.md) | Standard catalogue boundaries, identity, expansion, inspection, migration, and future catalogue requirements. |
| [`docs/specs/contracts/`](docs/specs/contracts) | Compact reviewed contract records for reusable specification-backed rules. |
| [`docs/premium_continuation_roadmap.md`](docs/premium_continuation_roadmap.md) | Active implementation roadmap and links to earlier roadmap context. |
| [`.beads/`](.beads/) | Tracked implementation tasks, dependencies, and completion records; inspect with `br`. |
| [`docs/testing_and_verification_policy.md`](docs/testing_and_verification_policy.md) | Required verification depth, reference semantics, differential testing, atomicity, invariant, and regression obligations. |

### Verification code

| Path | Responsibility |
| --- | --- |
| [`crates/mossignal/tests/`](crates/mossignal/tests) | Public-API integration tests grouped by semantic family. |
| [`crates/mossignal/tests/support/`](crates/mossignal/tests/support) | Shared compiled-circuit fixtures, policy construction, behavior traces, and equivalence helpers. |
| [`crates/mossignal/tests/golden/`](crates/mossignal/tests/golden) | Diagnostic catalogue, public failure inventory, and canonical projection fixtures. |
| [`scripts/check_static_guardrails.py`](scripts/check_static_guardrails.py) | Static guardrail checks used by final verification. |
| [`scripts/contracts.py`](scripts/contracts.py) | Contract tooling and coverage checks. |
| [`scripts/check_acceptance_record.py`](scripts/check_acceptance_record.py) | Acceptance-record consistency checks. |
| [`Makefile`](Makefile) | Development and final verification gates. |

## Public API and authoring paths

There are two intended authoring levels.

### Typed authoring

`NetworkBuilder<D>` and `ModuleBuilder<D>` are the ergonomic path. A typed
`Signal<S>` is scoped to the builder that created it, and `S` is sealed to
`Level` or `Pulse`. This gives compile-time signal-kind separation and runtime
builder-scope protection. A signal from another live builder is rejected before
the receiving builder is mutated.

The builders support:

- explicit or locally allocated stable keys;
- external level and pulse inputs and outputs;
- constants, inversion, variadic level operators, selection, and thresholding;
- pulse merge, coalesce, zip, gate, select, and route;
- rising, falling, and any-edge detectors;
- toggle, pulse/level set-reset latches, and sample-hold;
- pulse delay and transport delay;
- reusable module construction and nested module instantiation;
- standard catalogue construction and ordinary convenience aliases.

Convenience methods lower to the same ordinary primitive representation. They do
not create a second evaluator or hidden semantic layer.

### Dynamic/lossless authoring

`UncheckedNetwork<D>` and `UncheckedModule<D>` are the direct representation
path. They intentionally retain duplicate keys, malformed endpoints, wrong
directions, wrong signal kinds, duplicate mappings, missing mappings, and caller
insertion order. This is necessary so validation can report complete structured
evidence instead of silently normalizing away the defect.

The normal transitions are:

```text
UncheckedNetwork::validate()
    -> Report<ValidatedNetwork<D>, D>

ValidatedNetwork::compile() / compile_ref()
    -> Report<CompiledNetwork<D>, D>

CompiledNetwork::spawn(RuntimePolicy)
    -> Machine<D>
```

`UncheckedModule::validate()` and `validate_ref()` produce immutable validated
`ModuleDef<D>` artifacts. A `Report` may contain warnings and an artifact; any
remaining error suppresses the artifact. Use `require_artifact()` when a caller
needs the successful artifact or the complete diagnostic set on failure.

## Validation and compilation

Validation is more than shape checking.

1. Structural validation collects independent findings: duplicate identities,
   missing endpoints, invalid directions, signal-kind mismatches, unsupported
   multiple drivers, missing required drivers, fixed/variadic arity defects,
   malformed module bindings, and hierarchy defects.
2. Node schemas define which input roles can affect which current outputs.
3. Validation builds a current-reaction dependency graph over external inputs,
   module boundaries, node operations, node outputs, and external outputs.
4. Strongly connected components with a self-cycle or multi-member cycle are
   rejected with deterministic members and a cycle witness.
5. Temporal barriers are represented in the dependency signature. A delay can
   break an instantaneous cycle because its input is not a same-reaction output
   dependency, while ordinary combinational/stateful dependencies remain
   visible.
6. Acyclic definitions are normalized by stable keys and fingerprinted from a
   canonical semantic encoding.

The compiled representation is immutable and shareable. It contains private
dense indices, operation descriptors, predecessor/successor adjacency,
stable-key lookups, state slots, initial observations, temporal descriptors,
external endpoint tables, and retained module correspondence. It contains no
mutable machine state.

When module instances exist, compilation uses a private module lowerer to
flatten nested definitions into an executable primitive topology. The lowerer
also records mappings from flattened runtime subjects back to qualified
module-instance paths. Execution is flattened; public graph views, diagnostics,
provenance, and inspection remain hierarchy-aware.

## Runtime semantics

### Lifecycle

A newly spawned machine is explicitly `AwaitingInitialization`. It has no
current time, settled output baselines, or fabricated default level values.

The first successful transaction must be an `InputSnapshot` containing every
external level input exactly once. Pulse inputs are optional in the snapshot and
default to zero when omitted.

After initialization the machine is `Ready { now }`. Ready transactions use an
`InputDelta`:

- supplied levels replace authoritative current values;
- omitted levels retain their previous values;
- pulse inputs apply only to the current reaction and never persist;
- the requested time must be strictly greater than the current time;
- the transaction must carry the current network revision and matching network
  and input-schema identities.

### One reaction and output publication

The evaluator runs the compiled operations once in deterministic topological
order. Stateful operations read the complete previous state vector and stage
successors; they do not mutate committed state while evaluation is in progress.

The result contains:

- level output establishment/change events;
- positive pulse output events with exact multiplicity;
- committed transient diagnostic occurrences;
- diagnostic episode changes;
- the next `Dormant` or `WakeAt(time)` schedule;
- an immutable provenance view for causes.

Level output values are retained as current machine state. Pulse activity is
result-owned and reaction-scoped; it is not retained as a current output value.

### Temporal execution

Pending work is stored in a `BTreeMap<Time<D>, Vec<PendingEvent<D>>>`.

For an advancement to target time `T`:

1. Pending deadlines strictly earlier than `T` are processed chronologically as
   internal reactions.
2. Each deadline batch is aggregated deterministically. Pulse delays sum counts
   with checked arithmetic; transport delays resolve simultaneous candidates by
   deterministic origin/target rules.
3. External target-time level inputs become authoritative after earlier internal
   deadlines have settled.
4. Events due exactly at `T` are combined with the target-time external inputs
   and pulse batch in one final reaction.
5. New delay obligations are scheduled from the actual reaction time that
   created them.

The current implementation includes `PulseDelay` and `TransportDelay`. The
public schedule exposes only the earliest pending wakeup, not operating-system
timers. Mossignal never sleeps, polls, owns a wall clock, or invokes host
callbacks.

### Atomicity and budgets

Transactions use a clone-and-publish strategy. The candidate includes the full
runtime store: lifecycle, time, levels, settled values, state slots, output
baselines, pending events, provenance, diagnostic episodes, and cause roots.
Every fallible evaluation, arithmetic operation, budget check, event schedule,
and diagnostic/provenance construction happens before publication. A failure at
any point discards the entire candidate, including earlier internal deadlines
from the same outer transaction.

`RuntimePolicy` requires explicit limits for:

- internal reactions;
- evaluated operations;
- pending events;
- events created per transaction;
- required provenance growth.

The policy has its own semantic identity, separate from network and input-schema
identity.

## Modules and hierarchy

`ModuleDef<D>` is an immutable validated reusable definition backed by shared
ownership. It retains:

- typed public input and output keys;
- private primitive structure;
- public interface mappings;
- nested module instances;
- explicit origin (`User` or a standard declaration);
- a semantic `ModuleFingerprint`.

Instantiation validates every public binding exactly once. Missing, duplicate,
unknown, wrong-kind, foreign-builder, and foreign-source bindings are rejected.
Nested instances retain explicit containment paths. Runtime flattening allocates
private dense identities, but public qualified identities use instance keys plus
module-local stable keys.

Module identity is sensitive to semantic structure, interface mappings, nested
instances, node configuration, port roles, and connections. It is insensitive to
metadata and caller claim order.

## Identity and canonicalization

The following identities have distinct meanings:

| Identity | Meaning |
| --- | --- |
| `NetworkKey`, `NodeKey`, port keys, endpoint keys, module keys | Stable authored structural identities. |
| `TimeDomainId` | Caller-owned identity for what one logical tick means. |
| `NetworkFingerprint` | Canonical semantic identity of a validated network. |
| `InputSchemaFingerprint` | Canonical identity of the complete typed external-input schema. |
| `ModuleFingerprint` | Canonical semantic identity of a reusable module. |
| `StandardModuleExpansionFingerprint` | Identity of a standard declaration's canonical primitive expansion. |
| `RuntimePolicyId` | Identity of the execution limits that can affect success. |
| `NetworkRevision` | Machine-local installed-topology revision; not currently a persistence-format promise. |
| `PendingEventKey` and `CauseRef` | Runtime/result-scoped references, not durable structural identity. |

Fingerprints use domain-separated, versioned canonical encodings and BLAKE3.
Semantic collections are sorted by stable identity. Metadata, diagnostics,
allocation order, hash iteration order, and dense compiled positions are not
semantic inputs to the current fingerprint projections.

There is currently no machine-state snapshot/restore, replay-log, digest-suite,
forecasting, or general persistence/serialization API in the Rust
implementation. `InputSnapshot` is an implemented input artifact for
initialization; it is not a persisted machine snapshot. The persistence
specifications describe the future boundary.

## Diagnostics, provenance, and inspection

Diagnostics are structured catalogue entries, not parsed display strings. Each
`DiagnosticCode` fixes its spelling, severity, responsibility, evidence schema,
and permitted delivery forms.

The main delivery forms are:

- `Diagnostic` inside a static `DiagnosticSet`/`Report`;
- `RuntimeFailure` for a rejected transaction or operation;
- `DiagnosticOccurrence` for a transient condition committed by a reaction;
- `ActiveDiagnosticEpisode` and `DiagnosticEpisodeChange` for continuous
  retained conditions;
- internal-defect records when diagnostic evidence itself conflicts.

Provenance is a transaction-result-owned immutable causal graph. `CauseRef`
values are scoped to their owning `ProvenanceView`; using one against another
view is rejected. Provenance retains stable subjects, input observations,
stateful transitions, selected pulse contributions, scheduled temporal causes,
pending-event causes, output causes, and episode causes.

Inspection is a projection of semantic machine state. It exposes definition
facts and committed runtime facts separately, including qualified module nodes,
edge observations, stored levels, latch controls, sample-hold state, pending
delay work, schedules, outputs, and active diagnostic episodes.

Use `DiagnosticScope` to select a direct or qualified node, or all primitive
owners within a module hierarchy. `TransactionResult::occurrences_for` and
`diagnostic_episode_changes_for` preserve the committed record order and remain
usable after an owner is removed. `Machine::active_diagnostic_episodes_for`
returns owned current records with their retained causal view. These reads do
not change execution or the condition lifecycle.

The executable [diagnostic example](crates/mossignal/examples/diagnostics.rs)
shows occurrence evidence, unchanged-condition suppression, resolution, and
removal termination. Run it with `cargo run -p mossignal --example diagnostics`.

## Current implemented catalogue

The standard catalogue currently contains three canonical stateless modules:

- `Exactly`;
- `AtMost`;
- `AllEqual`.

Catalogue construction validates exact descriptor identity, parameters, signal
kinds, public input keys, canonical expansion roles, and generated-key
collisions. The resulting module executes through the ordinary module and
machine paths. It also exposes structured inspection and explanations.

Builder conveniences such as `xor`, `nand`, `nor`, `xnor`, and `majority` are
ordinary primitive expansions, not separate runtime semantics.

`BindingSet` and `BoundMachine` provide a non-semantic application boundary:
caller-owned identifiers are mapped to stable external endpoints, inputs are
projected into canonical snapshots/deltas, and outputs are projected back. The
bound façade delegates to the ordinary `Machine`; it must not implement a
second evaluator.

## Testing and verification

Tests are deliberately layered.

- Unit tests live beside implementation modules for local laws, canonical
  encoding, diagnostics, graph algorithms, and invariant checks.
- Integration tests under [`crates/mossignal/tests`](crates/mossignal/tests)
  use only the public API and cover complete semantic flows.
- Equivalence tests compare direct primitive graphs with composed/module-wrapped
  forms over bounded truth tables and histories.
- Temporal tests compare direct large jumps with stepwise deadline execution.
- Failure-atomicity tests verify that late failures roll back all candidate
  state, pending work, provenance, occurrences, and episodes.
- Golden fixtures cover the implemented diagnostic registry, public failure
  inventory, and selected canonical projections.

The repository verification policy treats optimized execution as a refinement
of a simpler reference semantics. Future incremental, optimized, persistence,
forecasting, and reconfiguration paths must preserve that relationship.

The executable gate definitions are in [`Makefile`](Makefile); required
workflow and tooling rules are in [`AGENTS.md`](AGENTS.md).

## Roadmaps and task status

Use the active [continuation roadmap](docs/premium_continuation_roadmap.md)
and its linked earlier roadmap for implementation planning and progress.
Use the [beads records](.beads/) for task scope, dependencies, and completion
details. Inspect approved claimable work with `br ready --json` and individual
tasks with `br show <id> --json`.

Consult those records and the source for current implementation status when
starting a task. This README does not track the latest completed roadmap item.

## Agent navigation recipes

Useful searches:

```bash
# Inspect roadmap headings and approved claimable tasks.
rg -n '^## ' docs/premium_continuation_roadmap.md
br ready --json

# Find the authoritative rule for a concept before changing semantics.
rg -n 'NetworkFingerprint|topology patch|current-reaction|PulseDelay' docs/specs/

# Trace the public lifecycle.
rg -n 'finish\(|validate\(|compile\(|spawn\(|Machine::apply|apply_initialization|apply_advance' \
  crates/mossignal/src

# Trace primitive schemas and evaluator cases.
rg -n 'NodeKind|SemanticNodeKind|schema_for_kind|CompiledNodeKind|EvaluationCause' \
  crates/mossignal/src

# Find the tests for one semantic family.
rg -n '^fn |PulseDelay|TransportDelay|Module|provenance|atomic' \
  crates/mossignal/tests
```

For repository workflow, issue tracking, specification authority, and required
checks, read [`AGENTS.md`](AGENTS.md) rather than duplicating those instructions
here.

## Project semantic boundaries

When extending Mossignal, preserve these product-level boundaries. General
implementation and workflow policy is in [`AGENTS.md`](AGENTS.md).

1. Keep signal kind explicit. Never silently convert `Level` transitions into
   pulses or pulses into temporary levels.
2. Keep logical time exact and caller-owned. Never add wall-clock behavior,
   sleeping, polling, implicit scheduling, or host callbacks.
3. Keep input snapshots and deltas distinct. Never fabricate omitted level input
   values during initialization.
4. Keep current-reaction causality separate from authored graph connectivity.
   Temporal barriers and state semantics must be represented explicitly.
5. Keep stable keys separate from dense runtime indices.
6. Keep modules semantically attributable even when execution is flattened.
7. Keep all fallible transaction work before the single publication point.
8. Keep provenance and diagnostic identity semantic; do not expose private arena
   positions as durable identity.
9. Keep application bindings as projections over the ordinary evaluator.
