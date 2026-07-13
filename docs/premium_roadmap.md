# `mossignal` Premium Foundation Implementation Roadmap

**Status:** Provisional implementation roadmap approved for detailed planning
**Purpose:** Establish a trustworthy architectural and executable foundation using premium implementation agents before reassessing whether broader work can safely be delegated to less capable agents.

---

## 1. Strategy

The initial implementation will be divided into narrow, ordered foundation tasks.

Each early task should introduce one primary architectural concept, establish its local representation and invariants, and stop. Early tasks must not combine several unrelated design problems merely because they eventually participate in the same execution pipeline.

A task may become broad in line count when it extends an already established pattern mechanically. It should not be broad because it must invent several foundational patterns simultaneously.

The initial premium-agent phase is intended to establish:

* authoritative project structure;
* stable implementation patterns;
* representation and ownership boundaries;
* validation and compilation stages;
* the first complete execution path;
* reusable testing patterns;
* extension conventions for later node families and subsystems.

The result should be a **premium foundation**: incomplete in breadth, but architecturally coherent and safe to extend.

---

## 2. General task-sizing rules

Each foundation task should normally have:

* one primary architectural responsibility;
* one coherent local subsystem;
* a small number of closely related types;
* explicit dependencies on earlier completed tasks;
* focused invariants and tests;
* explicit exclusions;
* no obligation to design unrelated future subsystems.

Foundation tasks should avoid formulations such as:

```text
Implement the graph, validation, compilation, diagnostics, runtime, and tests.
```

Instead, each stage should leave behind a concrete artifact consumed by the following stage.

Later tasks may cover several types or primitive nodes together when they all follow an already established implementation pattern.

---

# Part I — Fundamental values and shared infrastructure

## 3. Signal value types

Implement only:

* `Level` and `Pulse` marker types;
* sealed `SignalType`;
* `SignalKind`;
* `LogicLevel`;
* `PulseCount`;
* checked pulse-count arithmetic;
* focused unit tests.

Explicitly exclude:

* graph representation;
* builders;
* nodes;
* runtime execution;
* diagnostics;
* persistence.

This task establishes the fundamental signal type model without depending on any network structure.

---

## 4. Logical-time value types

Implement only:

* `Time<D>`;
* `Span<D>`;
* `NonZeroSpan<D>`;
* checked time and span arithmetic;
* trait implementations independent of traits on `D`;
* zero, boundary, invalid-subtraction, and overflow tests.

Explicitly exclude:

* event scheduling;
* temporal nodes;
* machine time progression;
* runtime transactions;
* persistence encoding.

Time values are kept separate from signal values because their generic representation and checked arithmetic form an independent design concern.

---

## 5. Stable structural key types

Implement only:

* the initial stable-key families;
* typed signal-bearing keys;
* erased key enums;
* explicit durable construction;
* deterministic equality, hashing, and canonical ordering;
* caller-owned `KeyAllocator`;
* typed-to-erased and erased-to-typed conversions;
* category-safety and conversion tests.

Explicitly exclude:

* graph storage;
* global identity allocation;
* dense runtime indices;
* revision-bound resolved handles;
* persistence encoding.

The task must establish a clear distinction between structural key categories even when their opaque payloads happen to be numerically equal.

---

## 6. Diagnostic metadata values

Implement only the owned metadata values required by authored definitions:

* `DiagnosticMeta`;
* `DiagnosticPath`;
* `OriginRef`;
* owned names and descriptions;
* tags;
* equality, cloning, and ownership tests.

Explicitly exclude:

* validation findings;
* diagnostic codes;
* reports;
* runtime diagnostic occurrences;
* persistent diagnostic episodes;
* rendering;
* provenance.

Metadata is descriptive information attached to semantic subjects. It is not itself the diagnostic reporting system.

---

## 7. Problem and report kernel

Implement the real common problem-reporting architecture needed by later validation tasks:

* catalogue-backed `DiagnosticCode`;
* fixed severity and responsibility associated with each code;
* typed, code-specific evidence;
* `Problem<D>`;
* `DiagnosticSet<D>`;
* `Report<T, D>` or the final equivalent artifact-plus-findings type;
* deterministic problem ordering;
* deterministic report merging;
* focused construction and ordering tests.

Only the diagnostic codes and evidence required by the opening implementation tasks need to be added.

Explicitly exclude:

* the exhaustive diagnostic catalogue;
* runtime diagnostic occurrences;
* persistent diagnostic episodes;
* provenance;
* rendered prose systems;
* observer delivery.

This task must not introduce a temporary simplified diagnostic model that later needs replacement.

---

# Part II — Authored structure and validation

## 8. Unchecked authored graph representation

Implement a deliberately restricted authoritative authored model supporting:

* network identity;
* external level inputs;
* external level outputs;
* node definitions;
* typed input and output port definitions;
* connection definitions;
* diagnostic metadata;
* a tiny primitive set consisting initially of `Constant` and `Not`;
* unchecked construction and direct test fixtures.

The core authored representation must be signal-kind-aware even though the first executable subset is level-only.

The unchecked representation must preserve malformed authored input, including where relevant:

* duplicate keys;
* dangling references;
* mismatched kinds;
* invalid directions;
* conflicting drivers;
* missing fixed ports.

Storage must not silently overwrite duplicate-key entries.

Explicitly exclude:

* complete validation;
* reaction dependencies;
* cycle checking;
* dense runtime indices;
* compilation;
* machine state;
* execution.

The purpose is to establish the authoritative stable-keyed structural representation.

---

## 9. Structural validation phase

Implement only the structural validation rules applicable to the restricted graph:

* stable-key uniqueness;
* referenced subject existence;
* port direction;
* signal-kind compatibility;
* required fixed ports;
* connection incidence;
* input driver rules;
* deterministic diagnostic ordering;
* preservation of all discovered findings where possible.

This phase must not construct the public `ValidatedNetwork`.

It should produce a private validation candidate or workspace containing structurally usable data for reaction-dependency extraction.

Explicitly exclude:

* reaction-dependency extraction;
* SCC analysis;
* cycle validation;
* topological ordering;
* compilation;
* execution.

The private intermediate representation must not imply an additional public lifecycle stage.

---

## 10. Reaction-dependency extraction

Implement only:

* the internal reaction-operation model;
* current-reaction dependency signatures for `Constant` and `Not`;
* derivation of the reaction dependency graph from the structurally checked candidate;
* stable correspondence between reaction operations and authored subjects;
* deterministic graph inspection for tests.

Explicitly exclude:

* cycle detection;
* SCC decomposition;
* topological ordering;
* dense compiled layouts;
* machine execution.

This task establishes the essential distinction between:

```text
the authored structural graph
```

and:

```text
the current-reaction dependency graph
```

A whole node must not be treated as a causality barrier merely because future node families may own state.

---

## 11. Reaction-cycle validation and deterministic topological order

Implement only:

* SCC-based current-reaction cycle detection;
* self-loop detection;
* cycle witness construction;
* cycle diagnostics tied back to stable structural subjects;
* deterministic topological ordering for valid reaction graphs;
* graph invariant tests;
* insertion-order invariance tests.

Only after structural validation and reaction-cycle validation both succeed may this task construct:

```text
ValidatedNetwork
```

Explicitly exclude:

* dense runtime indices;
* compiled execution descriptors;
* evaluator execution;
* machine state.

This task completes semantic validation for the restricted graph.

---

# Part III — Semantic identity and compilation

## 12. Restricted semantic fingerprints and input-schema identity

Implement semantic identity for the currently supported restricted model:

* `NetworkFingerprint`;
* `InputSchemaFingerprint`;
* the required persistent time-domain identity contribution;
* canonical semantic projection of the restricted validated network;
* deterministic fingerprint construction;
* insertion-order invariance;
* exclusion of diagnostic metadata;
* exclusion of dense indices and private representation details.

Explicitly exclude:

* complete persistence artifact encoding;
* snapshots;
* state digests;
* replay;
* topology-patch identity;
* unsupported node families.

This task exists before input artifacts so snapshots and deltas can bind to real semantic network and schema identity rather than temporary pointers or ad hoc tokens.

---

## 13. Initial compiled representation

Implement only:

* `ValidatedNetwork -> CompiledNetwork`;
* stable-key-to-dense-index resolution;
* distinct opaque dense index types;
* immutable node and port descriptors;
* immutable reaction-dependency adjacency;
* retained deterministic topological order;
* stable-key lookup;
* compiled invariant checking.

The first representation need not optimize for compactness beyond avoiding obviously pathological design.

Explicitly exclude:

* machine state;
* evaluator execution;
* dirty propagation;
* incremental compilation;
* resolved public handles;
* temporal descriptors;
* event storage.

The primary objective is the correct ownership and identity boundary between stable authored structure and revision-local dense execution representation.

---

# Part IV — First evaluator and input artifacts

## 14. Full evaluator for `Constant` and `Not`

Implement only the simple full reference evaluator for the restricted compiled graph:

* evaluate every reaction operation once;
* use the retained deterministic topological order;
* accept externally supplied level facts;
* store complete level results;
* evaluate `Constant`;
* evaluate `Not`;
* expose evaluator results to direct tests;
* verify each operation runs only after its predecessors are available.

Explicitly exclude:

* `Machine`;
* lifecycle;
* transactions;
* cloning or transaction staging;
* pulse values;
* stateful nodes;
* output events;
* incremental dirty evaluation.

The evaluator should prioritize clarity and auditability over optimization.

---

## 15. Complete level `InputSnapshot` and builder

Implement only:

* an owned network-bound `InputSnapshot`;
* a builder bound to the exact network fingerprint and input-schema fingerprint;
* one authoritative `LogicLevel` value for every external level input;
* rejection of unknown inputs;
* rejection of duplicate or conflicting observations;
* rejection of incomplete snapshots;
* deterministic snapshot contents;
* focused construction and failure tests.

Explicitly exclude:

* machine initialization;
* input deltas;
* pulse input;
* persistence encoding;
* binding projection.

The completed snapshot must be a valid owned artifact that later initialization execution can consume without discovering omitted external levels.

---

## 16. Minimal validated runtime policy and identity

Implement only the minimum runtime-policy foundation required before transaction execution:

* an immutable validated `RuntimePolicy`;
* the initially required execution limits;
* validation of policy values;
* `RuntimePolicyId`;
* deterministic policy identity;
* focused policy-validation tests.

Explicitly exclude:

* broad performance tuning;
* dynamic policy mutation;
* persistence artifacts;
* budget accounting for unsupported subsystems.

Only limits that can affect success or failure in the restricted runtime should be introduced.

---

# Part V — Machine lifecycle and level transactions

## 17. Machine spawning and lifecycle storage

Implement only:

* spawning independent machines from one `CompiledNetwork`;
* explicit `AwaitingInitialization`;
* explicit `Ready`;
* stored runtime policy identity;
* topology revision storage;
* ownership of restricted runtime state;
* lifecycle inspection;
* independence tests between machines spawned from one compiled topology.

No transaction execution should occur yet.

Explicitly exclude:

* initialization application;
* input deltas;
* output events;
* temporal schedules;
* stateful node storage;
* snapshots;
* reconfiguration.

A newly spawned machine must not contain implicit `Low` external inputs or settled signal values.

---

## 18. Initialization transaction execution

Implement the first successful transaction path:

* an initialization transaction at an arbitrary logical time;
* exact topology revision expectation;
* one complete `InputSnapshot`;
* ordinary invocation of the existing full evaluator;
* atomic staging;
* transition from `AwaitingInitialization` to `Ready`;
* authoritative external level storage;
* settled level result storage;
* external `LevelEstablished` events;
* rejection leaving the complete machine unchanged;
* initialization success and failure tests.

Explicitly exclude:

* `InputDelta`;
* later transactions;
* pulse input;
* stateful nodes;
* temporal deadlines;
* topology patches;
* persistence;
* optimized staging.

The transaction path should use a deliberately simple correctness-oriented staging mechanism.

---

## 19. Level `InputDelta` and builder

Implement only:

* an owned network-bound `InputDelta`;
* builder binding to the exact network fingerprint, input-schema fingerprint, and applicable revision context;
* explicit level changes;
* omission meaning retention of the previous authoritative value;
* rejection of unknown inputs;
* rejection of duplicate or conflicting entries;
* deterministic delta representation;
* focused construction tests.

Explicitly exclude:

* transaction execution;
* pulse input;
* binding projection;
* persistence encoding.

`InputSnapshot` and `InputDelta` must remain distinct and non-interchangeable types.

---

## 20. Ready-machine level transaction execution

Implement later level-only transactions:

* strict logical-time increase;
* expected topology revision checks;
* application of `InputDelta`;
* retention of unmentioned external level values;
* clone-and-swap or equivalently simple full-state staging;
* ordinary invocation of the existing full evaluator;
* settled runtime-state replacement;
* external `LevelChanged` events;
* no event when a settled output remains unchanged;
* complete transaction failure atomicity;
* focused time, output, and rollback tests.

A state-only transaction must not advance `NetworkRevision`.

Explicitly exclude:

* pulse input;
* stateful nodes;
* event calendar;
* temporal execution;
* topology patches;
* persistence;
* optimized sparse staging.

At the end of this task, the following complete restricted path should work:

```text
author
→ structurally validate
→ derive reaction dependencies
→ validate causality
→ construct ValidatedNetwork
→ compile
→ spawn
→ initialize
→ apply later level transaction
→ observe settled output events
```

---

# Part VI — Required premium extensions before broad delegation

## 21. Typed `NetworkBuilder` foundation

Before broad implementation is delegated, establish the typed authoring pattern:

* `NetworkBuilder<D>`;
* builder identity;
* builder-scoped `Signal<S>`;
* foreign-signal rejection;
* typed external level inputs and outputs;
* typed `Constant` and `Not` constructors;
* explicit keyed construction forms;
* deterministic lowering into the existing unchecked authored representation;
* typed-versus-dynamic equivalence tests.

Explicitly exclude:

* the complete primitive constructor family;
* modules;
* bindings;
* pulse constructors;
* persistence.

This task must reuse the existing authoritative unchecked representation rather than create a competing graph model.

---

## 22. First variadic primitive: `All`

Implement `All` through every already established layer:

* authored node and variadic port representation;
* typed builder construction;
* dynamic definition;
* structural validation;
* dependency signature;
* reaction graph extraction;
* cycle validation;
* compilation;
* evaluation;
* total zero-input and unary semantics;
* duplicate-source behavior;
* deterministic variadic-port handling;
* focused conformance tests.

This task establishes the canonical pattern for variadic primitives.

After this pattern is secure, a broader task may reasonably implement:

```text
Any
Parity
AtLeast
Select
```

because the architectural work should then be largely mechanical.

---

# Part VII — Work after the premium foundation

## 23. Mechanical level-combinational expansion

Once `All` establishes the variadic pattern, broader tasks may add the remaining level-combinational primitive family using the established architecture:

```text
Any
Parity
AtLeast
Select
```

Such work may be broad in line count because it should no longer invent:

* graph representation;
* typed builder architecture;
* dependency extraction;
* compilation;
* evaluator dispatch;
* transaction integration;
* basic conformance-test structure.

---

## 24. Pulse foundation

Pulse support must be introduced separately from stateful-node support.

The first pulse foundation should establish:

* reaction-scoped pulse representation;
* complete simultaneous pulse counts;
* pulse propagation and fan-out;
* pulse result storage;
* one simple pulse primitive;
* pulse-specific evaluator and transaction integration;
* multiplicity tests.

Only after this is established should the rest of the pulse-combinational family be implemented mechanically.

---

## 25. Stateful-node foundation

Previous and proposed state must be established using one narrowly chosen stateful node.

The foundation should introduce:

* previous committed state;
* proposed successor state;
* one proposal per state cell;
* atomic state commitment;
* same-reaction downstream visibility;
* state-family storage;
* state inspection hooks;
* stateful conformance tests.

Only after that pattern exists should broader stateful-node families be added.

Pulse storage and stateful storage must not be invented in the same foundation task.

---

## 26. Temporal foundation

Temporal execution should begin only after ordinary reaction and state ownership patterns are stable.

The first temporal foundation should separately establish:

* exact ordered event calendar;
* stable pending-event identity;
* equal-deadline unordered batches;
* strictly future scheduling;
* chronological advancement through internal deadlines;
* one simple temporal primitive, preferably `PulseDelay`;
* direct-jump versus stepwise equivalence tests.

More complex temporal nodes should follow only after the event and ownership model is proven.

---

# Part VIII — Deferred systems

The premium opening sequence does not initially implement:

* complete persistence encoding;
* replay logs;
* complete provenance;
* elaborate inspection queries;
* topology patches;
* standard modules;
* optimized incremental evaluation;
* complete diagnostic catalogue;
* every primitive constructor;
* performance-oriented data structures.

However, the foundation must not make these systems impossible or require replacement of core ownership and identity models.

In particular:

* runtime state must remain attributable to stable structural owners;
* dense indices must remain revision-local implementation details;
* authored malformed data must remain representable;
* metadata must remain non-semantic;
* transaction failure must remain atomic;
* topology revision must remain distinct from state progression;
* future persisted artifacts must be expressible through stable semantic identity.

---

# Part IX — Planning and implementation process

## 27. Planning cadence

The complete roadmap may be retained, but only the next small cluster of tasks should be planned in implementation-level detail.

Recommended cadence:

1. fully design the next three or four tasks;
2. convert them into implementation beads;
3. have premium agents implement them in order;
4. inspect the resulting architecture and code;
5. revise the next roadmap cluster based on what actually landed.

Later beads should inherit concrete types and patterns from committed code rather than restating or independently reinventing the complete architecture.

---

## 28. Bead structure

Each implementation bead should contain:

```text
Purpose
Dependencies
Scope
Required public behavior
Required internal representation
Required invariants
Required tests
Acceptance commands
Explicit exclusions
Artifacts exposed to the following task
Architectural decisions intentionally deferred
```

A bead should reference exact specification sections where necessary, but it should not duplicate the complete specification corpus.

The bead must clearly distinguish:

* requirements owned by the current task;
* established patterns that must be reused;
* later decisions that remain open;
* unrelated work that must not be introduced.

---

## 29. Premium-agent objective

Premium agents are not being purchased merely to write the first several features.

Their purpose is to establish code that later answers, through concrete implementation:

* how public and internal types are separated;
* how authored, validated, compiled, and running forms relate;
* how malformed definitions survive until validation;
* how structural and reaction graphs differ;
* how stable identity and dense execution identity differ;
* how diagnostics and reports are represented;
* how fingerprints bind semantic artifacts;
* how full evaluation works;
* how machine lifecycle is stored;
* how transactions commit atomically;
* how later primitive families integrate with the system.

The premium phase succeeds when later work can extend these patterns without redesigning the foundations.

---

## 30. Reassessment of broader delegation

After the premium foundation is complete, less capable agents should first receive controlled extension tasks, such as:

* implement another edge-detector variant using an existing pattern;
* implement another pulse combinational node;
* add one diagnostic code and evidence type;
* add one conformance case;
* extend one existing primitive through persistence or inspection;
* add one mechanically similar builder constructor.

Their work should be evaluated for:

* architectural conformity;
* unnecessary redesign;
* specification compliance;
* test quality;
* respect for explicit exclusions;
* convergence after review findings.

Only after repeated success should they receive broader subsystem or node-family tasks.

---

## 31. Governing principle

The opening implementation may tolerate:

* incomplete feature coverage;
* simple algorithms;
* poor performance;
* limited ergonomics;
* missing optimizations;
* repetitive code that can later be consolidated.

It must not tolerate:

* incorrect semantic foundations;
* temporary identity systems;
* silent loss of malformed authored information;
* incomplete validation represented as `ValidatedNetwork`;
* hidden default inputs;
* transaction partial commitment;
* conflation of stable keys and dense indices;
* architectural reinvention in each bead.

The implementation strategy is therefore:

> Establish one narrow architectural fact at a time, verify it, and make the next task build concretely upon it.
