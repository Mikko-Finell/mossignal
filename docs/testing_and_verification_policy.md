# `mossignal` Testing and Verification Policy

**Status:** Design specification, version 1  
**Defines:** Verification obligations, reference semantics, conformance testing, property-based testing, differential testing, bounded exhaustive exploration, fuzzing, fault injection, debug invariant checking, regression artifacts, and CI/release gates  
**Does not define:** Public signal semantics, built-in node semantics, processor architecture, serialized wire formats, performance targets, editor testing, application integration, or unrestricted formal verification

---

## 1. Purpose

This specification defines how `mossignal` demonstrates that an implementation preserves the semantics and architectural invariants established by the API, built-in node, and processor specifications.

`mossignal` makes stronger claims than ordinary output correctness. It claims deterministic synchronous settlement, glitch freedom, exact caller-owned logical time, atomic transactions, state-preserving reconfiguration, complete pending-work accounting, causal explanation, persistent diagnostic episodes, snapshot sufficiency, and replay equivalence.

Those claims must become executable verification obligations.

The central policy is:

> Every optimized, incremental, compacted, or specialized execution path must refine a simpler reference semantics.

The verification strategy therefore combines:

- specification examples;
- per-node conformance suites;
- executable reference implementations;
- property-based generation and shrinking;
- differential comparison;
- bounded exhaustive exploration;
- fuzzing of untrusted boundaries;
- fault injection for failure atomicity;
- debug recomputation of invariants;
- persistent regression artifacts;
- staged CI and release gates.

Testing alone does not constitute a mathematical proof of the complete implementation. The policy nevertheless uses established mathematical structures directly where they yield complete bounded checks, executable equivalences, or clear proof obligations.

---

## 2. Normative language

The terms **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.

A **verification obligation** is a property that an implementation must demonstrate through one or more of:

- construction by a validated algorithm;
- executable invariant checking;
- differential comparison against a reference path;
- exhaustive bounded exploration;
- property-based testing;
- regression testing;
- focused code reasoning documented beside the implementation.

No single test category is sufficient for every obligation.

---

# Part I — Verification model

## 3. System under verification

For verification purposes, a ready machine is modeled as a deterministic partial transition system.

For machine state `M`, runtime policy `P`, and transaction `τ`:

```text
Apply_P(M, τ) = Success(M', R)
              | Failure(F)
```

where:

- `M'` is the complete successor semantic machine state;
- `R` is the immutable transaction result;
- `F` is a structured failure;
- failure leaves the published machine equal to `M`.

An uninitialized machine participates in the same transition system with lifecycle-specific transaction rules.

Verification compares complete semantic outcomes rather than only external output values.

## 4. Semantic observation

A comparison harness must be able to derive a canonical semantic observation from a machine and result.

The observation should include, where applicable:

```text
lifecycle status
logical time
topology revision
network fingerprint
external level valuation
settled level-port values
stateful-node state
temporal-node state
pending event calendar
external output baselines
active diagnostic episodes
required provenance roots or normalized derivations
schedule
execution-state digest
observable-state digest
transaction result
```

A narrower observation may be used only when the property being tested intentionally concerns a narrower semantic scope.

## 5. Semantic equivalence

Two outcomes are **semantically equivalent** when they agree on every observable fact defined by the applicable specifications after removing representation-only differences.

Permitted normalization may include:

- dense runtime indices;
- arena positions;
- allocation order;
- hash-table order;
- non-semantic provenance record identifiers;
- canonical representation order among semantically unordered supporters;
- internal event-table slot identity when no stable public event identity differs;
- diagnostic prose where stable code, severity, subject, and evidence agree.

Normalization MUST NOT erase:

- stable structural identity;
- logical time;
- topology revision;
- pulse multiplicity;
- pending-event identity when publicly inspectable;
- causal supporter multiplicity;
- deadline;
- migration outcome;
- diagnostic episode identity;
- retention boundaries;
- structured failure category;
- any state capable of affecting future behavior.

The normalization function itself MUST be deterministic and tested.

## 6. Refinement obligations

Let `Reference` denote a simple implementation of specified semantics and `Candidate` an optimized or incremental implementation.

For every valid shared input domain:

```text
Normalize(Candidate(x)) == Normalize(Reference(x))
```

The principal required refinement obligations are:

```text
incremental reaction evaluation
    == full topological reaction evaluation

optimized transaction staging
    == clone-and-swap transaction execution

direct time advancement
    == chronological stepwise advancement

optimized event calendar
    == ordered semantic calendar

incremental topology handling
    == full validation, compilation, migration, and reevaluation

incremental region maintenance, if introduced
    == full weak-component recomputation

incremental inspection subscription update
    == fresh inspection projection

optimized canonical encoding or digest generation
    == stable-keyed canonical reference encoding

forecast
    == applying the same transaction to an unpublished clone

replay
    == repeated ordinary application of the same transactions
```

A candidate path may replace its reference path in production only after the applicable refinement obligation is continuously enforced in tests.

## 7. Categories of obligation

Verification requirements fall into five categories.

### 7.1 Invariant obligations

A property must hold for every reachable state.

Examples:

- reaction dependencies are acyclic;
- every pending deadline is strictly later than current machine time;
- every state slot belongs to the correct state family;
- committed provenance is acyclic;
- active diagnostic episodes refer to surviving semantic subjects.

### 7.2 Equivalence obligations

Two execution methods must produce semantically equivalent results.

Examples include incremental versus full reaction evaluation and direct versus stepwise time advancement.

### 7.3 Completeness obligations

A classification or explanation must account for every relevant item.

Examples:

- every pending event receives a reconfiguration outcome;
- every required current fact reaches an authoritative provenance root;
- every required external level input appears in an initialization snapshot;
- every built-in node kind implements all required semantic fields.

### 7.4 Failure-atomicity obligations

Every structured rejection must leave the published semantic machine unchanged.

### 7.5 Compatibility obligations

Persisted or migrated artifacts must be accepted exactly when the applicable compatibility rules hold and rejected precisely otherwise.

---

# Part II — Verification layers

## 8. Required verification layers

The implementation must use several complementary layers.

### 8.1 Specification examples

Focused examples establish named boundary cases and serve as readable executable documentation.

### 8.2 Conformance suites

Reusable suites verify that every implementation of a semantic family satisfies the common obligations of that family.

### 8.3 Property-based testing

Generated valid structures and histories exercise broad state spaces and semantic interactions.

### 8.4 Differential testing

Candidate implementations are compared against reference implementations.

### 8.5 Bounded exhaustive exploration

Finite small domains are enumerated completely where feasible.

### 8.6 Fuzzing

Untrusted encoded or dynamically authored data is subjected to adversarial malformed input, while valid structured fuzzers exercise long semantic histories.

### 8.7 Regression testing

Every confirmed defect receives a permanent minimized reproducer.

No layer should be treated as a substitute for all others.

---

# Part III — Reference implementations

## 9. Required reference paths

The project MUST retain simple correctness-oriented reference paths for the following subsystems.

### 9.1 Full topological reaction evaluator

The reference reaction evaluator processes every reaction operation exactly once in one valid deterministic topological order.

It does not perform dirty-closure pruning.

It is the oracle for incremental reaction evaluation.

### 9.2 Clone-and-swap transaction executor

The reference transaction path clones the complete semantic machine, executes the transaction on the clone, and replaces the original only on success.

It is the oracle for sparse overlays, copy-on-write roots, private arena segments, and other optimized staging methods.

### 9.3 Ordered semantic event calendar

The reference calendar represents deadlines through an ordered mapping from exact logical time to complete equal-deadline batches.

Canceled events are removed semantically before minimum-deadline selection.

It is the oracle for heaps, arenas, tombstones, generation invalidation, timing wheels, or other optimized structures.

### 9.4 Full validation and compilation

The reference topology path rebuilds all derived structures from the complete authored definition, including:

- stable-key lookup;
- dependency graph;
- SCC decomposition;
- deterministic topological order;
- state layouts;
- temporal descriptors;
- regions;
- graph-query metadata;
- fingerprint input.

It is the oracle for any future incremental compilation path.

### 9.5 Full region recomputation

The reference region implementation recomputes weakly connected components from the complete structural graph.

### 9.6 Stable-keyed canonical state representation

The reference persistence and digest representation is expressed through stable semantic identity rather than dense runtime positions.

### 9.7 Fresh inspection projection

The reference inspection path computes each requested projection directly from the complete committed machine state.

It is the oracle for incremental subscription updates and cached inspection plans.

### 9.8 Straightforward replay fold

The reference replay path applies replay frames sequentially through the ordinary transition function and validates expected prior and resulting digests.

## 10. Availability of reference paths

Reference paths MUST:

- exist in the ordinary test configuration;
- be callable by property and differential tests;
- be available in a debug verification configuration where practical;
- share semantic definitions with production code without sharing the optimized algorithm being verified;
- remain maintained when semantics evolve.

They need not:

- be part of the public API;
- be enabled in default release builds;
- meet production performance targets.

A reference implementation must remain simple enough to audit. An implementation that duplicates the candidate algorithm too closely is not an independent oracle.

## 11. Reference path independence

Reference and candidate paths SHOULD avoid sharing the exact logic whose correctness is under comparison.

Shared low-level semantic laws are acceptable. Shared control flow, event scheduling strategy, dirty propagation, or migration bookkeeping may invalidate the differential test by reproducing the same defect in both paths.

Where full independence is impractical, the limitation must be documented and supplemented with another verification method.

---

# Part IV — Test data generation and reproducibility

## 12. Structured generators

Property-based and structured fuzz generators SHOULD produce semantic objects rather than arbitrary bytes whenever the property requires valid input.

Generators should cover:

```text
valid acyclic networks
invalid current-reaction cycles
typed and mistyped connections
fixed and variadic arities
stateful chains
temporal chains
multiple weak regions
complete initialization snapshots
ready-machine input deltas
future transaction sequences
exact-deadline interactions
runtime policies
topology patches
compatible and incompatible snapshots
inspection and explanation requests
observer query plans
```

Generated networks must retain enough metadata to explain failures and shrink them meaningfully.

## 13. Valid-network generation

A valid-network generator SHOULD construct the reaction graph in a way that guarantees acyclicity, for example by:

1. assigning a generated partial or total rank to reaction operations;
2. adding current-reaction edges only from lower to higher rank;
3. inserting state or temporal barriers explicitly where structural cycles are desired without reaction cycles.

The generator should still produce diverse valid topological orders and structural cycles broken by genuine barriers.

## 14. Invalid-network generation

Invalid generators should target one or a small number of defects deliberately rather than relying only on random corruption.

Required invalid classes include:

- duplicate keys;
- missing nodes or ports;
- wrong direction;
- signal-kind mismatch;
- unsupported multiple drivers;
- missing fixed inputs;
- invalid variadic arity;
- invalid timing parameter;
- invalid initial state;
- incompatible state schema;
- malformed module interface;
- current-reaction self-loop;
- multi-node current-reaction cycle;
- stale network or revision references.

## 15. Transaction-history generation

A transaction-history generator must preserve lifecycle and time rules unless the property intentionally tests rejection.

Valid histories should include:

- initialization at an arbitrary nonzero time;
- snapshots with every required external level;
- strictly increasing transaction times;
- time jumps across zero, one, or many deadlines;
- simultaneous pulse and level input;
- topology patches effective at future transaction times;
- state-only transactions between patch preparation and commitment;
- forecast and replay branches.

## 16. Shrinking

Every generated failure SHOULD shrink toward a minimal semantic counterexample.

Shrinking should attempt, where valid:

```text
fewer nodes
fewer connections
fewer external endpoints
fewer transactions
fewer processed deadlines
fewer patch operations
smaller pulse counts
smaller times and spans
simpler runtime policy
shorter provenance ancestry
fewer requested inspection fields
```

A shrinker must preserve the preconditions of the property being tested. A valid-network equivalence failure must not shrink into an invalid network and then disappear behind validation rejection.

## 17. Reproducibility

Every randomized failure MUST report enough information to reproduce it, including:

- random seed;
- generator version where relevant;
- minimized semantic artifact or serialized reproducer;
- feature configuration;
- runtime policy;
- semantic version;
- candidate and reference observations;
- normalization result;
- platform information when relevant.

CI MUST preserve failing artifacts as downloadable logs or test artifacts.

---

# Part V — Validation and compilation verification

## 18. Validation soundness

Every accepted `ValidatedNetwork` MUST satisfy all static invariants required by compilation and execution.

Tests must verify that malformed authored data cannot become a validated artifact through any public construction path.

For generated invalid definitions, validation must:

- return structured diagnostics;
- omit the validated artifact when blocking diagnostics remain;
- avoid panics;
- produce deterministic diagnostic ordering;
- identify stable structural subjects where available.

## 19. Validation completeness over known defect classes

For every enumerated validation rule, at least one focused positive case and one focused negative case are required.

Property-based invalid generators SHOULD demonstrate that each targeted defect class is rejected independently.

The policy does not require proving that every imaginable malformed representation maps to a unique diagnostic. It does require that every documented invalid condition is detected.

## 20. Reaction-cycle verification

Compilation derives a current-reaction dependency graph that must be a directed acyclic graph.

Tests must verify:

- every true reaction cycle is rejected;
- structural cycles broken by previous state or strictly later time may compile;
- a whole stateful node is not incorrectly treated as a universal barrier;
- a whole temporal node is not incorrectly treated as a universal barrier;
- due-event dependencies and current-input dependencies match the node specification;
- self-loops are detected;
- SCC diagnostics identify a valid cycle witness.

For bounded generated graphs, cycle detection SHOULD be cross-checked against an independent reachability-based oracle.

## 21. Deterministic compilation

Semantically equivalent stable-keyed definitions constructed in different insertion orders must produce equivalent:

- fingerprint;
- structural graph view;
- reaction dependency relation;
- state schemas;
- region partition;
- graph queries;
- externally observable compiled metadata.

Dense-index assignment may differ only where the public semantics permit it. Any representation-dependent difference must not affect machine behavior, persistence, diagnostics, or digests.

## 22. Compiled invariant checks

Debug and test builds must be able to verify:

```text
every dense reference is in bounds
every descriptor matches its node kind
every port has the declared signal kind
every connection obeys driver rules
every reaction edge advances in topological order
every state slot belongs to the correct family
stable-key lookup is unambiguous
endpoint tables are complete
region membership partitions structural subjects
```

---

# Part VI — Initialization verification

## 23. Lifecycle coverage

Tests must cover both lifecycle states:

```text
AwaitingInitialization
Ready
```

An uninitialized machine must not be treated as a ready machine holding `Low` values.

## 24. First-transaction obligations

The first successful transaction must be tested for:

- arbitrary initial logical time;
- complete authoritative level snapshot;
- initial pulse batch;
- declared initial state as previous state;
- ordinary full reaction evaluation;
- successor-state commitment;
- future temporal scheduling;
- provenance roots;
- diagnostic episode establishment;
- `LevelEstablished` output events;
- ready-machine schedule after commitment.

## 25. Initialization rejection

Tests must verify structured rejection of:

- `InputDelta` before initialization;
- incomplete level snapshot;
- duplicate or conflicting observations;
- unknown endpoint;
- wrong network or binding projection;
- stale revision;
- invalid patch;
- budget failure;
- checked time failure.

Every rejection must preserve the complete uninitialized machine unchanged.

## 26. Pre-initialization inspection

Structural inspection must remain available before initialization.

Current runtime inspection must fail structurally for:

- current port values;
- current outputs;
- schedule;
- pending events;
- active runtime diagnostics;
- current explanations.

## 27. Edge-detector initialization

Every edge detector must be tested under both policies:

```text
Baseline
Assume(initial_level)
```

The first observation under `Baseline` must establish memory without emission. `Assume` must compare the first settled input normally.

## 28. Temporal initialization

Required cases include:

- fresh `PulseDelay` with no pending work;
- `TransportDelay` first input equal to and different from its explicit initial level;
- `InertialDelay` first input equal to and different from its explicit initial level;
- fresh disabled `Periodic` remaining anchorless;
- fresh enabled `Periodic` under each first-emission policy.

---

# Part VII — Reaction and signal verification

## 29. Full reaction semantics

The full topological evaluator must be tested directly against the node laws and against hand-authored multi-node examples.

It must evaluate each reaction operation exactly once after all reaction predecessors have settled.

## 30. Incremental reaction equivalence

For generated valid networks, previous machine states, and same-time stimulus batches:

```text
IncrementalReaction(G, M, batch)
    ==
FullTopologicalReaction(G, M, batch)
```

Comparison must include:

- settled level outputs;
- pulse multiplicities;
- proposed successor state;
- future event additions and cancellations;
- diagnostic-condition updates;
- provenance roots or normalized derivations;
- semantic change set.

## 31. Work-order invariance

Where several reaction operations are simultaneously eligible, tests must vary the valid work order.

Every valid topological linear extension must produce equivalent semantics.

For small reaction DAGs, the harness SHOULD enumerate all topological orders. For larger generated DAGs, it should sample several distinct valid orders.

## 32. Input-order invariance

Equivalent same-time input batches must produce equivalent results under permutations of:

- level observation insertion order;
- pulse occurrence insertion order;
- binding projection order;
- external endpoint order;
- equal-time event representation order.

## 33. Glitch freedom

Required focused cases include:

- two inputs changing oppositely while `Any` remains `High`;
- two inputs changing oppositely while `All` remains `Low`;
- parity-preserving simultaneous changes;
- `Select` branch and selector changes at one time;
- multi-layer reconvergent combinational paths;
- edge detectors downstream of settlement-equivalent changes;
- stateful nodes whose current outputs feed downstream edge detectors or stateful nodes.

No downstream semantic observation may expose an evaluator intermediate.

## 34. Pulse algebra

Tests must verify:

- pulse multiplicity is a non-negative count;
- fan-out copies the full count to every destination;
- duplicate sources are counted once per connected port;
- `Merge` sums counts;
- `Coalesce` maps positive counts to one;
- `Zip` takes the minimum and does not retain unmatched pulses;
- pulse routing preserves selected multiplicity;
- no pulse persists as current state after its reaction.

Boundary counts must include at least zero, one, two, and a larger representable count.

Overflow behavior, if a finite representation is used, must follow the public checked-failure policy and preserve transaction atomicity.

## 35. Level algebra

The level combinational catalogue must be tested exhaustively over small arities and all input valuations where feasible.

Required total-law cases include:

```text
All([]) = High
Any([]) = Low
Parity([]) = Low
AtLeast(0, inputs) = High
AtLeast(k > arity, inputs) = Low
```

Commutative nodes must be invariant under port permutation while retaining distinct stable port identity.

## 36. Current-state isolation

Every stateful node test must confirm that:

- current output may reflect current inputs according to the node law;
- every state transition reads the same previous committed state;
- proposed successor state is not exposed as stored state during the reaction;
- one state cell proposes at most one successor value;
- all successor state commits together after successful settlement.

---

# Part VIII — Built-in node conformance

## 37. Node conformance requirement

Every built-in node kind MUST implement a reusable conformance suite before it may be considered complete.

The suite must cover every applicable field required by the built-in node specification.

## 38. Common conformance matrix

For each primitive, the test inventory must explicitly classify and cover:

```text
port kinds and directions
fixed or variadic arity
parameter validation
declared initial state
first-reaction behavior
current-reaction dependency signature
current output law
proposed successor-state law
simultaneous-input law
pulse multiplicity law
due-obligation dependency
exact-deadline law
same-deadline batch law
strictly-future scheduling
inspection schema
current explanation
why-not explanation
transition causality
pending-event representation
snapshot round trip
compatible reconfiguration
incompatible reconfiguration
state-dependent migration
pending-event migration
topology-induced first reaction
transient diagnostics
diagnostic episode lifecycle
```

A field that does not apply must be marked explicitly as not applicable.

## 39. Exhaustive primitive testing

Where the complete input and state domain is finite and small, primitive laws SHOULD be exhaustively enumerated.

This includes:

- all level combinational nodes for bounded arities;
- all selector valuations;
- edge detectors over every previous-observation condition and current input;
- toggles over both stored levels and bounded pulse parities;
- set/reset latches over both states and every simultaneous control presence combination;
- sample-and-hold over both stored values, both sampled values, and zero/nonzero sample count;
- diagnostic episode transitions for level latch conflicts.

## 40. Stateful chain testing

Focused multi-node cases must cover same-reaction chains such as:

```text
Pulse -> Toggle -> RisingEdge -> Toggle
Pulse -> PulseSetResetLatch -> AnyEdge
Level -> SampleHold -> FallingEdge
```

The tests must confirm current downstream visibility together with one atomic successor-state commit.

## 41. Conflict policies

Both set/reset latch kinds must test every conflict policy:

```text
SetDominant
ResetDominant
RetainAndDiagnose
RejectTransaction
```

For level-controlled conflict, `RetainAndDiagnose` must be tested as a persistent diagnostic episode. For pulse-controlled conflict, it must remain a reaction-scoped occurrence.

---

# Part IX — Temporal verification

## 42. Exact discrete time

Tests must use arbitrary initial times and non-unit spans. The implementation must not depend on time beginning at zero or on adjacent caller transactions.

Checked arithmetic tests must cover:

- valid addition;
- valid subtraction;
- invalid reverse subtraction;
- maximum representable boundary;
- overflow rejection;
- no wrapping.

## 43. Strictly future scheduling

Every newly created temporal obligation must satisfy:

```text
deadline > current reaction time
```

Generated semantic histories should assert this invariant after every successful reaction.

Immediate behavior must appear directly in the current reaction rather than as a same-time pending event.

## 44. Direct-jump equivalence

For any ready machine `M` and future target `T`, let the pending deadlines before `T` be:

```text
d1 < d2 < ... < dk < T
```

Tests must compare:

```text
Apply(M, transaction_at(T))
```

with an equivalent sequence that advances through each meaningful deadline and then `T`, accounting for the rule that caller transactions themselves must use strictly increasing times.

The resulting semantic state and chronological observable events must be equivalent.

## 45. Equal-deadline batching

Events sharing one deadline must be processed as one unordered batch.

Tests must permute their representation order and verify invariant results.

The harness must include equal-deadline interactions among:

- several pulse-delay groups;
- transport transitions;
- periodic boundaries and external input;
- due events from different nodes;
- due events combined with a topology patch at the same transaction time.

## 46. Exact-deadline boundary triad

Every temporal cancellation or control law must include cases where the relevant input occurs:

```text
strictly before the deadline
exactly at the deadline
strictly after the deadline
```

This triad is mandatory for inertial cancellation, periodic enable control, and any future temporal primitive with deadline-sensitive input.

## 47. `PulseDelay`

Tests must verify:

- every input occurrence is reproduced once at `origin + delay`;
- multiplicity is preserved;
- groups with equal deadline may aggregate semantically;
- new input at the due time does not suppress the due group;
- new input schedules only strictly future work;
- duration change preserves existing deadlines by default;
- every alternative migration policy is explicit and complete.

## 48. `TransportDelay`

Tests must verify:

- every settled input transition is queued;
- short-lived transitions are preserved;
- current input has no instantaneous path to current output;
- a due transition matures despite a new same-time input transition;
- remembered input updates correctly;
- same-deadline migrated transitions resolve by greatest originating logical time;
- no same-time intermediate output transition is observable;
- indistinguishable conflicting origins reject or resolve explicitly.

## 49. `InertialDelay`

Tests must verify:

- candidate creation only when input differs from the output remaining after due obligations;
- contradictory change strictly before deadline cancels;
- contradictory change exactly at deadline does not cancel maturation;
- the exact-deadline contradiction may create a new opposite candidate;
- current input has no instantaneous path to current output;
- duration-change migration policies preserve their specified origin and deadline semantics.

## 50. `Periodic`

Tests must cover every combination of:

```text
Immediate | AfterFirstPeriod
RestartPhase | PreservePhase
fresh anchorless | previously anchored
enable before boundary | at boundary | after boundary
```

Tests must also verify:

- disabled boundaries emit nothing and do not accumulate;
- large jumps preserve separate emissions at actual boundary times;
- settled same-time enable controls a due boundary;
- reconfiguration policies define anchor and next-deadline outcomes explicitly.

## 51. Event cancellation and next deadline

After cancellation, an event must:

- not fire;
- not appear as pending;
- not determine `next_deadline` or `Schedule::WakeAt`;
- remain only in explicitly retained optional history, if such history exists.

The optimized calendar minimum must be differentially compared with the ordered reference calendar.

---

# Part X — Transaction and failure-atomicity verification

## 52. Single-publication semantics

Tests must establish that public inspection observes either the complete old machine or the complete new machine, never a partially published intermediate.

Where concurrency is not part of the public contract, this may be verified through internal publication boundaries and absence of public callbacks during preparation.

## 53. Candidate-versus-clone equivalence

For generated machines and transactions:

```text
OptimizedApply(M, τ)
    ==
CloneAndSwapApply(M, τ)
```

This comparison applies to both success and structured failure.

## 54. Failure atomicity

For every structured failure category, the test harness must capture the complete semantic observation before execution and compare it after rejection.

At minimum, unchanged state must include:

```text
lifecycle status
topology
revision
logical time
external levels
settled current levels
node state
temporal state
pending events
output baselines
provenance roots
active diagnostic episodes
execution-state digest
observable-state digest
```

## 55. Fault injection

Test-only fault injection SHOULD cover every fallible preparation stage that mutates candidate state or allocates semantic artifacts.

Injection points should include, where applicable:

- earlier-deadline reaction evaluation;
- current-time reaction evaluation;
- topology compilation;
- migration finalization;
- event addition and cancellation staging;
- provenance construction;
- diagnostic-episode update;
- result construction;
- canonical encoding;
- digest computation;
- runtime budget check.

Injected failures must use the same rollback/publication boundary as ordinary structured failures.

Fault injection must not become part of production semantics.

## 56. Rejection categories

Focused tests must cover at least:

- stale revision;
- stale expected execution digest;
- invalid lifecycle operation;
- non-increasing time;
- checked time overflow;
- wrong network identity;
- conflict-rejecting latch;
- prohibited state loss;
- incompatible patch;
- invalid snapshot restoration;
- runtime budget exhaustion.

## 57. No host effects during preparation

The evaluator must not invoke host callbacks during propagation or preparation.

Tests or static API review must ensure that host-visible output is returned only through committed immutable results or post-commit observer delivery.

---

# Part XI — Reconfiguration verification

## 58. Two-stage patch semantics

Tests must distinguish:

```text
structural preparation
transaction-time finalization
```

Structural preparation must depend only on the base topology and static patch content. Exact state-dependent outcomes must be finalized against the state actually reached at the effective time.

## 59. Prepared-patch freshness

Tests must verify:

- state-only transactions do not invalidate structural preparation;
- topology revision changes do invalidate it;
- applying a prepared patch to the wrong base revision fails structurally;
- exact forecasts are additionally bound to execution-state digest, effective time, and runtime policy.

## 60. State reached at effective time

A required differential scenario is:

1. prepare a patch;
2. advance through one or more earlier deadlines that change state;
3. finalize the patch at its effective time;
4. verify migration uses the newly reached candidate state rather than preparation-time state.

## 61. Complete migration classification

For every surviving stateful or temporal subject, finalization must produce exactly one outcome:

```text
Preserve
Migrate
Reset
Reject
```

For every pending event, finalization must produce exactly one outcome:

```text
PreserveDeadline
RecomputeDeadline
TransformPayload
Cancel
Reject
```

Tests must assert that the classification is total and contains no duplicate ownership.

## 62. State-loss policy

Under `RejectStateLoss`, any nonempty finalized semantic loss set must reject the complete transaction.

Under `AllowReportedStateLoss`, every actual loss must appear in the committed migration report.

Required loss cases include:

- removed stored state;
- reset state;
- canceled pending work;
- terminated required provenance ancestry;
- removed output baseline.

Dense-handle invalidation must not be mislabeled as semantic state loss.

## 63. Region merge and split

Adding or removing connections may merge or split weak regions.

Tests must verify that region changes alone do not reset compatible surviving node state, temporal state, pending events, provenance, or diagnostic episodes.

## 64. Topology-induced reaction

After migration, every potentially affected current-reaction operation must be reevaluated.

An incremental patch reaction must be compared with complete evaluation of the new compiled graph.

Required output consequences include:

- preserved level output unchanged;
- preserved level output changed;
- new level output established;
- removed output reported as topology consequence;
- pulse output only where new reaction equations genuinely emit one.

## 65. Stable and resolved identity

Tests must verify:

- preserved stable keys retain identity;
- removed keys become unavailable;
- new keys do not alias removed subjects accidentally;
- stale dense handles fail structurally;
- stale compiled inspection plans fail structurally;
- dense-slot reuse never transfers state, provenance, or diagnostic episodes to another subject.

## 66. Patch differential testing

Generated small patches should be applied through:

```text
prepared migration path
```

and an independent reference path that rebuilds the new topology, computes stable-key correspondence, applies explicit migration laws, and fully reevaluates the new graph.

The resulting machines and reports must be semantically equivalent.

---

# Part XII — Provenance and explanation verification

## 67. Provenance acyclicity

Every committed provenance derivation must be acyclic.

Debug and test builds must be able to recompute acyclicity from the retained graph.

Edges must advance in logical time, reaction dependency order, or migration/checkpoint establishment order as specified.

## 68. Authoritative-root completeness

Every required current historical fact must have a backward path to an authoritative root.

Required facts include:

- current stateful state;
- current external level output baseline;
- every pending event;
- every output event;
- latest retained transition of an inspectable signal;
- migration and reset consequences;
- causally relevant active diagnostic evidence.

An unexplained previous value is a test failure.

## 69. Joint and unordered support

For nodes such as `All`, `Any`, `Parity`, `AtLeast`, `Zip`, and grouped pulse merges, supporter order is not semantic.

Tests must permute supporter representation while preserving:

- supporter identity;
- contribution count;
- joint-support grouping;
- blocker meaning.

Normalized explanations must remain equivalent.

## 70. Current support versus latest transition

Focused tests must change the current supporting facts of an unchanged level output.

The explanation must update current support without fabricating a new output transition cause.

Explanation-sensitive subscriptions must detect this change even when the level value remains constant.

## 71. Checkpoint compaction

Provenance compaction tests must compare future behavior and current explanation before and after replacing older ancestry with an authoritative checkpoint.

After compaction:

- current facts must remain explained;
- retention status must identify the checkpoint boundary;
- no explanation may claim complete ancestry from initialization unless retained;
- execution-state equivalence must be preserved where optional history is excluded.

## 72. Migration provenance

Tests must verify:

- preserved state retains applicable ancestry;
- migrated state derives from old state plus migration rule;
- reset state receives an explicit reset or initialization cause;
- preserved pending events retain scheduling ancestry plus migration facts;
- canceled required work is reported as semantic loss where applicable.

## 73. Why-not explanations

For supported requests, tests must construct known blocked outcomes and verify that explanations identify actual:

- unsatisfied prerequisites;
- blockers;
- conflicts;
- disabled controls;
- disconnections;
- pending conditions;
- nearest relevant paths.

The library must not claim a unique cause or correction where the derivation does not establish one.

---

# Part XIII — Diagnostic verification

## 74. Structured diagnostic conformance

Tests must compare diagnostics structurally, not primarily through rendered prose.

The authoritative fields include:

```text
stable code
severity
primary subject
related subjects
evidence
machine-readable suggestions where valid
episode identity where persistent
```

Diagnostic ordering must be deterministic.

For every implemented diagnostic code, conformance tests must exercise its
specified primary-subject rule, condition discriminator, and evidence merge law.
Where only an initial `SubjectRef` ordering or diagnostic context is specified,
tests must remain within that declared scope and must not imply completeness for
future variants or contexts.

## 75. Persistent episode lifecycle

For every persistent condition, tests must cover:

```text
inactive -> active
active unchanged -> active unchanged
active evidence materially changes
active -> resolved
active subject preserved through patch
active subject migrated
active subject removed
rejected transaction while a candidate episode change exists
```

An unchanged active condition must not emit the same warning on every unrelated transaction.

A rejected transaction must not commit episode creation, modification, or resolution.

## 76. Episode identity

Episode identity must derive from stable semantic facts rather than prose, dense slots, or provenance arena identifiers.

Tests must ensure dense-slot reuse cannot attach an old episode to a new subject.

## 77. Validation report behavior

Validation, compilation, binding, and patch preparation reports should collect independent findings where safe.

Tests must verify:

- blocking diagnostics omit the artifact;
- non-blocking diagnostics may accompany an artifact;
- one malformed condition does not suppress unrelated safe findings unnecessarily;
- suggestions are absent when correction is ambiguous.

---

# Part XIV — Inspection and observer verification

## 78. Inspection purity

Inspection must be a pure projection.

Tests must compare machine digests and complete semantic observation before and after:

- direct inspection;
- explanation;
- graph queries;
- compiling an inspection plan;
- creating or removing an observer subscription.

No operation may alter machine execution state.

## 79. Stable query and compiled plan equivalence

For a valid revision-bound plan:

```text
InspectWithPlan(M, plan)
    ==
FreshInspect(M, stable_query)
```

After topology revision change, the stale plan must fail structurally and must not silently retarget.

Recompiling the stable query may succeed against surviving keys.

## 80. Incremental subscription equivalence

For every committed transition from `M_prev` to `M_next`, an observer projection and delta must satisfy:

```text
ApplyDelta(I(M_prev), delta) == I(M_next)
```

Differential tests must compare incremental observer updates against fresh projection.

## 81. Explanation-sensitive invalidation

Subscriptions requesting explanation data must update when causal support changes even if the observed signal value does not.

Tests must include reconvergent combinational examples where output remains unchanged but supporters change.

## 82. Cursor and resynchronization

Observer tests must cover:

- uninterrupted cursor progression;
- topology revision change;
- missed retained change set;
- expired history;
- explicit resynchronization;
- delivery failure after semantic commit.

Observer failure must not roll back or mutate the semantic machine.

---

# Part XV — Persistence, forecast, replay, and digest verification

## 83. Snapshot round trip

For every generated valid machine, including uninitialized machines:

```text
Restore(Serialize(Snapshot(M)))
```

must produce a semantically equivalent machine under a compatible compiled topology and runtime policy.

The comparison must use semantic state, not private memory layout.

## 84. Snapshot sufficiency

For restored machine `M_r` equivalent to original `M`, and every compatible future transaction sequence `T`:

```text
Replay(M, T) == Replay(M_r, T)
```

Property tests should exercise future histories containing state changes, temporal deadlines, reconfiguration, diagnostics, explanations, and further snapshots.

## 85. Restoration rejection

Malformed or incompatible snapshots must fail precisely for classes including:

- schema version mismatch;
- semantic version mismatch;
- fingerprint mismatch;
- lifecycle inconsistency;
- missing or duplicate state facts;
- invalid state schema;
- unknown stable subject;
- invalid external valuation;
- invalid pending-event owner;
- non-future pending deadline;
- inconsistent output baseline;
- missing or cyclic required provenance;
- invalid diagnostic episode;
- runtime-policy incompatibility where required;
- digest mismatch.

No partially constructed machine may be published.

## 86. Forecast equivalence

For machine `M` and transaction `τ`:

```text
Forecast(M, τ)
    ==
Apply(Clone(M), τ)
```

The original machine must remain unchanged on both forecast success and forecast failure.

Forecast results must be bound to the correct revision, execution-state digest, requested time, and runtime policy.

## 87. Replay equivalence

A recorded successful execution and replay from the same compatible starting snapshot must produce equivalent:

- output events;
- state changes;
- topology revisions;
- diagnostics and active episodes;
- provenance roots and checkpoints;
- schedules;
- final execution and observable digests.

## 88. Replay concatenation

For valid transaction sequences `A` and `B`:

```text
Replay(M, A ++ B)
    ==
Replay(Replay(M, A), B)
```

Tests must include replay boundaries immediately before and after temporal deadlines and topology patches.

## 89. Digest-scope verification

Tests must establish the intended distinctions among:

```text
ExecutionStateDigest
ObservableStateDigest
SnapshotDigest
```

Required cases include:

- optional retained history differs while execution-state digest remains equal;
- current explanation checkpoint state differs and observable digest changes;
- complete snapshot metadata differs and snapshot digest changes;
- dense-index or allocation differences do not change semantic digests;
- presentation metadata does not change topology fingerprint or semantic digests where excluded.

## 90. Canonical encoding vectors

Once canonical encoding is specified, the project must retain golden vectors for representative:

- network fingerprints;
- execution-state digests;
- observable-state digests;
- snapshot digests;
- runtime-policy identifiers.

Golden vectors must be versioned deliberately. A change requires either proof of semantic equivalence under a new encoding version or an explicit compatibility break.

---

# Part XVI — Runtime-policy and budget verification

## 91. Policy identity

Semantically relevant runtime policy fields must contribute to `RuntimePolicyId` canonically.

Tests must verify that:

- equal semantic policies yield equal identifiers;
- presentation or construction order does not affect identity;
- changing a semantically relevant limit changes identity;
- performance-only tuning excluded from semantics does not alter identity.

## 92. Budget boundaries

Every budget must be tested at:

```text
one below limit
exactly at limit
one above limit
```

where representable.

Required budgets include those actually implemented, such as:

- internal reactions;
- evaluated operations;
- pending events;
- events created per transaction;
- required provenance growth.

## 93. Budget failure atomicity

Exceeding a budget must:

- reject the entire outer transaction;
- preserve the published machine exactly;
- identify the exceeded budget;
- report limit and consumed amount where practical;
- never skip semantic work or publish partial settlement.

Direct and reference transaction paths must agree on budget success or failure under the same policy.

---

# Part XVII — Property-based and differential campaigns

## 94. Mandatory property campaigns

The ordinary test suite must include property campaigns covering at least:

```text
compilation order invariance
full versus incremental reaction
clone-and-swap versus optimized transaction
input batch permutation
valid topological order permutation
direct versus stepwise time advancement
ordered versus optimized event calendar
snapshot round trip
snapshot future-behavior sufficiency
forecast versus cloned apply
replay versus original execution
replay concatenation
patch migration versus full rebuild oracle
fresh versus incremental inspection
canonical digest invariance
failure atomicity
```

## 95. Long-history campaigns

Extended CI should execute generated histories containing hundreds or thousands of successful and rejected operations, including:

- state changes;
- temporal scheduling and cancellation;
- forecasts;
- snapshots and restorations;
- patch preparation and commitment;
- stale artifact attempts;
- diagnostic episode changes;
- provenance checkpoints.

The optimized and reference states should be compared periodically, not only at the final step, to localize divergence.

## 96. Metamorphic properties

Where a direct oracle is expensive, tests may use transformations that must preserve or predictably change semantics.

Required useful metamorphic transformations include:

- reordering commutative variadic ports;
- rebuilding a network in different insertion order;
- changing diagnostic metadata only;
- renaming non-semantic provenance IDs;
- splitting a time jump into deadline-aligned steps;
- composing and then flattening a semantically transparent module representation;
- adding an unconnected irrelevant region and confirming unaffected existing regions;
- snapshotting and immediately restoring before continuing.

The predicted invariant or change must be stated explicitly.

---

# Part XVIII — Bounded exhaustive exploration and model checking

## 97. Purpose

Bounded exhaustive exploration provides complete verification over deliberately small finite domains.

It is not a claim of full formal verification of arbitrary networks or unbounded time.

## 98. Required bounded exploration

The project SHOULD exhaustively enumerate:

- all valuations of each finite primitive input domain;
- all previous-state values for Boolean state cells;
- bounded pulse counts sufficient to distinguish zero, odd, even, one, and many;
- every simultaneous set/reset presence combination;
- every edge-detector initialization and first-input combination;
- exact-deadline before/at/after cases;
- small event calendars;
- small reaction DAGs and their topological orders;
- short transaction sequences over small generated networks;
- small reconfiguration preservation and rejection cases;
- diagnostic episode transition systems.

## 99. State-space bounds

Every exhaustive harness must state its bounds explicitly, for example:

```text
maximum nodes
maximum ports
maximum pulse count
maximum logical time
maximum pending events
maximum transaction-sequence length
allowed node kinds
allowed patch operations
```

A passing bounded result must not be described as proof beyond those bounds.

## 100. Transition-system exploration

For small machine models, the harness may enumerate reachable states under all valid bounded transactions.

Useful checked invariants include:

- determinism;
- no same-time second caller transaction;
- strictly future pending work;
- state-cell single-successor rule;
- atomic rejection;
- replay reachability equivalence;
- provenance acyclicity;
- diagnostic episode ownership.

State hashing for exploration must use canonical semantic state, not memory layout.

## 101. External formal tools

Use of a model checker, SMT solver, proof assistant, or symbolic executor is optional unless a later specification makes it mandatory for a particular subsystem.

Such tools are appropriate where they provide concrete leverage, for example:

- verifying a small transition relation for temporal boundary semantics;
- checking a migration classifier is total and mutually exclusive;
- proving a specialized worklist preserves topological readiness;
- verifying an unsafe representation invariant.

Tool-generated claims must state the modeled abstraction and assumptions.

---

# Part XIX — Fuzzing

## 102. Untrusted boundaries

Coverage-guided fuzzing MUST target every boundary that accepts dynamically authored or serialized caller-controlled data, including applicable forms of:

- unchecked network definitions;
- serialized network definitions;
- snapshots;
- replay frames;
- topology patches;
- transactions;
- binding projections;
- inspection queries;
- explanation requests;
- canonical decoders.

## 103. Malformed-input fuzzing obligations

Malformed input must not cause:

- panic attributable to caller-controlled invalidity;
- undefined behavior;
- out-of-bounds access;
- unbounded uncontrolled allocation outside runtime policy;
- silent acceptance of invalid structure;
- partial publication;
- nondeterministic diagnostic category.

Internal invariant failures reached only after successful validation remain processor defects and may follow the explicit defect policy.

## 104. Structured semantic fuzzing

A second fuzzing class should generate valid semantic objects and long operation sequences.

It should compare:

- reference and candidate evaluator paths;
- direct and stepwise time advancement;
- snapshot/restore continuation;
- forecast and cloned apply;
- replay and original execution;
- fresh and incremental inspection.

## 105. Fuzz corpus management

Confirmed unique failures must be minimized and added to the permanent regression corpus.

The project should retain a curated seed corpus containing:

- every built-in node kind;
- structural and reaction cycles;
- exact-deadline temporal cases;
- reconfiguration with pending work;
- active diagnostic episodes;
- provenance checkpoints;
- corrupted persistence artifacts.

Fuzz corpus compatibility across format versions must be managed explicitly.

---

# Part XX — Debug verification and implementation safety

## 106. Debug invariant checks

Debug and test configurations should support expensive recomputation of:

```text
reaction SCCs
topological order validity
incremental versus full reaction result
calendar minimum and membership
strictly future deadlines
state-family ownership
region partition
migration classification completeness
provenance acyclicity
authoritative-root reachability
diagnostic episode ownership
canonical digests
snapshot round trip
```

The checks should run at semantically meaningful boundaries, especially after compilation, reaction settlement, migration finalization, and transaction commitment.

## 107. Assertions versus structured failures

Tests must preserve the distinction between:

- invalid caller-controlled data;
- expected semantic rejection;
- internal invariant violation.

Caller errors and expected rejection must return structured failures without panic.

Impossible states after successful validation may trigger assertions or the configured defect policy.

A test that converts an internal defect into an ordinary caller diagnostic is not acceptable containment.

## 108. Unsafe code verification

The initial implementation should remain safe Rust.

Any later `unsafe` code must have:

- an isolated module boundary;
- a written safety invariant;
- documented preconditions;
- a safe reference behavior;
- differential tests;
- debug validation where practical;
- focused fuzzing or sanitizer coverage;
- review specifically addressing aliasing, lifetime, initialization, and bounds assumptions.

An optimization using `unsafe` is not accepted solely because ordinary tests pass.

## 109. Concurrency verification

If internal parallel evaluation is later introduced, it must refine the same deterministic reaction semantics.

Testing must vary scheduling and thread count while preserving equivalent:

- settled values;
- pulse counts;
- state transitions;
- event operations;
- provenance structure up to allowed normalization;
- diagnostics;
- digests.

Thread scheduling must not become semantic order.

---

# Part XXI — CI and release policy

## 110. Per-change verification

Every ordinary change must run, at minimum:

- compilation under supported core configurations;
- static analysis and formatting policy;
- focused unit tests;
- built-in node conformance suites;
- deterministic specification examples;
- moderate property-based campaigns;
- core differential tests;
- regression corpus;
- snapshot and replay smoke tests;
- debug invariant checks for affected subsystems.

## 111. Extended CI

Extended CI should run:

- larger property case counts;
- longer generated histories;
- all supported feature combinations;
- relevant platform targets;
- bounded exhaustive exploration;
- fuzz corpus replay;
- fault-injection campaigns;
- canonical encoding vectors;
- restoration incompatibility matrix;
- patch migration campaigns.

## 112. Scheduled verification

Scheduled or pre-release jobs should run:

- long-running coverage-guided fuzzing;
- high-count differential campaigns;
- large temporal histories;
- adversarial reconfiguration with pending events;
- repeated snapshot/restore/replay cycles;
- sanitizer or interpreter-based checks where applicable;
- performance regression suites once performance policy exists.

## 113. Required release gates

A release that claims support for a feature must not proceed while any of the following apply:

- a required conformance field lacks tests;
- candidate and reference paths diverge on a confirmed valid case;
- a known structured failure can partially mutate published state;
- a persistence golden vector changes unintentionally;
- a stable diagnostic code changes unintentionally;
- a supported snapshot or replay compatibility promise is broken without explicit versioning;
- a confirmed panic remains reachable through malformed caller-controlled data;
- a new built-in node lacks migration, inspection, explanation, and persistence coverage.

## 114. Flaky tests

A test whose result depends on uncontrolled iteration order, scheduling, timing, or random seed handling is itself a defect.

Flaky tests must not be silently retried until passing.

A quarantine, if temporarily necessary, must retain visible failure status and an assigned corrective issue.

## 115. Test runtime classification

Tests should be labeled by expected cost so the project can run fast checks frequently without losing heavier verification.

Suggested classes are:

```text
unit
conformance
property-short
property-long
exhaustive-bounded
fuzz-corpus
fault-injection
compatibility
performance
```

Classification is operational only and must not weaken release requirements.

---

# Part XXII — Regression artifacts and compatibility history

## 116. Permanent regressions

Every confirmed semantic defect must receive a minimized permanent regression test.

The test should state:

- violated invariant or equivalence;
- smallest known reproducer;
- expected semantic result;
- previous failure mode;
- relevant specification section.

## 117. Historical artifact corpus

Once persistence formats exist, the project should retain representative historical artifacts for every supported compatibility version:

- network definitions;
- uninitialized snapshots;
- ready snapshots;
- replay logs;
- active diagnostic episodes;
- pending temporal work;
- provenance checkpoints.

Release verification must restore or reject them according to the declared compatibility policy.

## 118. Semantic version changes

A change to public semantics must update:

- semantic version identifiers;
- affected reference behavior;
- conformance tests;
- compatibility expectations;
- canonical vectors where applicable;
- regression artifacts;
- migration or rejection diagnostics.

Tests must not be weakened merely to accommodate an unacknowledged semantic change.

---

# Part XXIII — Requirements for future features

## 119. New built-in nodes

A new primitive may enter the core only when:

- its full semantic definition exists;
- its dependency signature is tested;
- its conformance matrix is complete;
- its finite cases are exhaustively tested where feasible;
- its state and pending-work migration are tested;
- its inspection and explanation are tested;
- its snapshot and replay behavior are tested;
- its diagnostic occurrence or episode behavior is tested;
- it participates in generated-network differential campaigns.

## 120. Standard modules

A named standard module must verify:

- public interface conformance;
- canonical expansion or equivalent primitive semantics;
- stable internal identity where promised;
- inspection and hierarchy visibility;
- explanation paths;
- revision compatibility;
- state preservation across compatible module updates.

## 121. New optimized paths

Every new optimization must identify:

1. the reference path it refines;
2. the semantic normalization used for comparison;
3. the generator domain exercised;
4. known assumptions and exclusions;
5. debug checks retained after adoption.

An optimization without an executable equivalence strategy requires explicit architectural justification and stronger alternative verification.

## 122. Declarative contracts and assertions

A future contract subsystem must define its own conformance and model-checking obligations while preserving non-interference with core machine semantics unless contracts are explicitly defined as transaction-rejecting policy.

## 123. Constrained user-defined automata

Any future user-defined automaton facility must provide a finite, inspectable, persistable transition schema suitable for:

- exhaustive bounded transition testing;
- dependency-signature validation;
- deterministic replay;
- migration classification;
- explanation generation;
- malformed-definition fuzzing.

Unrestricted callbacks remain outside the verified evaluator boundary.

---

# Part XXIV — Deliberately unspecified choices

## 124. Implementation freedom

This policy does not mandate:

- a particular Rust testing framework;
- a particular property-testing library;
- a particular fuzzing engine;
- a particular model checker;
- exact CI provider configuration;
- exact randomized case counts;
- exact fuzzing duration;
- code-coverage percentage targets;
- mutation-testing thresholds;
- one canonical serialization format;
- one specific sanitizer suite.

These choices may evolve provided the required verification obligations and release gates remain satisfied.

## 125. Coverage metrics

Line and branch coverage MAY be used as gap-detection tools.

They MUST NOT be treated as proof of semantic coverage.

The authoritative completeness measures are:

- conformance-matrix coverage;
- required property coverage;
- differential oracle coverage;
- bounded state-space coverage where declared;
- regression coverage of known defects;
- compatibility artifact coverage.

## 126. Formal-verification boundary

The project does not claim complete formal verification of arbitrary networks, unbounded event histories, persistence implementations, or the Rust compiler and runtime.

Formal methods should be used where they establish a concrete bounded theorem or validate a critical abstraction. Decorative references to mathematical theory are not verification.

---

# Part XXV — Required verification properties

## 127. Required guarantees

The verification system must continuously exercise and defend:

```text
validated definitions satisfy static invariants
reaction dependencies are acyclic
full topological reaction semantics are deterministic
incremental reaction equals full reaction
valid work order does not affect semantics
same-time batch order does not affect semantics
only settled values are observable
pulse multiplicity is preserved according to node law
one successor is proposed per state cell per reaction
initialization never invents implicit Low input
first level outputs are established, not changed from a fabricated value
new temporal work is strictly future
equal-deadline events form one unordered batch
direct time jumps equal chronological advancement
exact-deadline behavior follows node-specific laws
optimized calendar equals ordered semantic calendar
structured failure preserves complete published state
optimized transactions equal clone-and-swap
patch finalization uses state reached at effective time
state and pending-event migration classification is complete
state loss is rejected or fully reported
region changes do not reset compatible state
topology-induced reaction equals complete new-graph evaluation
stable identity survives compatible revisions
stale dense artifacts fail without retargeting
required provenance is acyclic and rooted
checkpoint compaction preserves current explainability
persistent diagnostic episodes do not repeat unchanged warnings
inspection does not affect execution
incremental observer updates equal fresh projection
snapshot restoration preserves future behavior
forecast equals cloned apply
replay equals original execution
replay concatenation holds
canonical digests ignore representation-only differences
runtime-policy limits fail atomically
malformed caller data does not cause ordinary panics
new primitives satisfy the complete conformance matrix
```

---

# Summary

`mossignal` verification is organized around executable semantic refinement.

The core pattern is:

```text
precise public and node semantics
        ↓
simple reference implementation
        ↓
optimized or incremental candidate
        ↓
canonical semantic normalization
        ↓
differential equivalence
```

That center is reinforced by:

```text
focused specification examples
per-node conformance matrices
property-based generation and shrinking
bounded exhaustive state exploration
malformed-input and structured semantic fuzzing
fault injection for atomicity
debug invariant recomputation
persistent regression artifacts
staged CI and explicit release gates
```

The policy does not attempt to prove the entire library correct through one technique. It assigns each class of claim to the verification method that gives the strongest practical leverage.

Simple reference algorithms define executable truth for optimized paths. Finite primitive domains are enumerated completely. Generated networks and histories exercise composition. Fuzzing defends untrusted boundaries. Fault injection verifies publication discipline. Persistence and replay are checked through future-behavior equivalence rather than serialization round trips alone.

A feature is not complete merely when it appears to work. It is complete when its semantic obligations are named, its reference behavior is established, its boundaries are tested, its failure behavior is atomic, and its optimized implementation remains continuously equivalent to the model it claims to realize.
