# `mossignal` Processor and Runtime Architecture

**Status:** Consolidated design specification, version 2
**Defines:** Processor architecture, compilation, runtime state, reaction evaluation, temporal execution, transactions, reconfiguration, provenance, inspection, persistence, replay, and implementation boundaries
**Does not define:** The complete public API surface, individual built-in node semantics, serialized wire formats, testing policy, application integration, model checking, or performance targets

---

## 1. Purpose

This specification defines the intended internal architecture of the `mossignal` processor.

The public API and built-in node specifications define the library’s observable semantics. This document defines how the processor should realize those semantics while preserving:

* deterministic execution;
* glitch-free reactions;
* exact caller-owned logical time;
* unordered simultaneous stimulus batches;
* atomic state commitment;
* explicit temporal obligations;
* state-preserving reconfiguration;
* structured causal explanation;
* complete inspection;
* snapshot and replay equivalence;
* structured failure containment.

The architecture distinguishes between:

1. **Required invariants**, without which the public semantics cannot be guaranteed.
2. **Recommended initial implementation direction**, intended to guide the first implementation.
3. **Provisional representation choices**, which may change after implementation and profiling.

Exact container types, memory layouts, integer widths, allocation strategies, and optimization techniques are not normative unless they affect observable behavior.

---

## 2. Theory-grounded engineering

Where the processor implements a recognized mathematical structure, the design, implementation, and verification should name that structure and use its established results directly.

The intended method is:

1. identify the mathematical object being implemented;
2. state the relied-upon preconditions;
3. derive the architectural consequence;
4. choose an established algorithm where appropriate;
5. encode the resulting invariant in validation, tests, or code comments.

Relevant foundations include:

* directed graph theory;
* strongly and weakly connected components;
* partial orders and topological orderings;
* deterministic transition systems;
* synchronous reactive systems;
* discrete-event simulation;
* incremental computation;
* graph rewriting;
* causal derivation graphs;
* transactional state transitions.

Mathematical references should provide real correctness, algorithmic, complexity, or verification leverage. They should not be included decoratively.

---

# Part I — Representation and ownership

## 3. Lifecycle

The canonical representation lifecycle is:

```text
NetworkBuilder / UncheckedNetwork
                ↓
         ValidatedNetwork
                ↓
         CompiledNetwork
                ↓
             Machine
```

### 3.1 Authored definitions

`NetworkBuilder` and `UncheckedNetwork` represent authored structure:

* stable structural keys;
* nodes and typed ports;
* directed connections;
* external endpoints;
* modules and hierarchy;
* semantic parameters;
* initial-state declarations;
* diagnostic metadata.

These representations may prioritize authoring clarity and stable identity over execution efficiency.

### 3.2 Validated definitions

`ValidatedNetwork` establishes that every static structural and semantic requirement holds.

Validation includes:

* key uniqueness;
* node and endpoint existence;
* signal-kind compatibility;
* connection direction;
* driver rules;
* fixed and variadic arity;
* module-interface validity;
* parameter validity;
* initial-state validity;
* state-schema compatibility;
* current-reaction causality.

Only a validated definition may compile.

### 3.3 Compiled topology

`CompiledNetwork` is an immutable executable program.

It retains stable structural identity while deriving:

* dense runtime indices;
* reaction dependencies;
* deterministic topological metadata;
* state layouts;
* temporal descriptors;
* graph-query indices;
* region membership;
* semantic fingerprint.

It contains no mutable machine state and should be shareable by multiple independent machines.

### 3.4 Running machine

`Machine` pairs one compiled topology with mutable semantic execution state.

It owns:

```text
lifecycle status
current logical time
topology revision
external level state
current settled level values
stateful-node state
temporal-node state
pending event calendar
external output baselines
required provenance roots
active persistent diagnostic episodes
```

Client-specific subscriptions, delivery cursors, and inspection plans are not part of the semantic machine.

---

## 4. Immutable program and mutable store

The processor follows the conceptual model:

[
Machine = (CompiledProgram,\ MutableStore)
]

The compiled program owns immutable facts such as:

```text
stable-key lookup
dense node and port descriptors
reaction dependency graph
topological ordering
successor and predecessor ranges
state-family layouts
temporal-node descriptors
external endpoint tables
region assignments
graph-query metadata
semantic fingerprint
```

The mutable store owns:

```text
lifecycle state
logical time
revision
external level valuation
current level-port values
node state
temporal state
pending events
output baselines
provenance roots
active diagnostic episodes
```

This separation ensures that:

* two machines spawned from one compiled network cannot share semantic state accidentally;
* runtime mutation cannot corrupt compiled graph invariants;
* fingerprints do not depend on execution history;
* forecasts and transaction staging have clear ownership boundaries;
* topology can be replaced atomically during reconfiguration.

---

## 5. Stable keys and dense indices

Stable keys and dense runtime indices serve different purposes.

Stable keys support:

* persistence;
* diagnostics;
* external bindings;
* graph queries;
* reconfiguration;
* cross-revision identity;
* provenance.

Dense indices support:

* direct array access;
* compact adjacency;
* state-slot access;
* reaction scheduling;
* cache locality;
* efficient inspection.

Compilation establishes a revision-specific partial resolution:

[
Resolve_r : StableKey \rightharpoonup DenseIndex_r
]

A dense index has no meaning outside the compiled revision that created it.

Stable keys and dense indices must use distinct opaque types. Node, port, state, event, and provenance indices must not be freely interchangeable as raw integers.

Revision-bound resolved handles must fail structurally when used against an incompatible topology. They must never silently refer to a different subject after recompilation.

---

# Part II — Initialization

## 6. Machine lifecycle states

A newly spawned machine is explicitly uninitialized.

Conceptually:

```text
AwaitingInitialization
Ready
```

An uninitialized machine has:

* a compiled topology;
* declared initial node and temporal state;
* no current logical time;
* no authoritative external level valuation;
* no settled current signal valuation;
* no external output baseline;
* no pending event schedule;
* no current runtime explanation.

Using `Low` as an implicit external default is forbidden. `Low` is a real signal value, not an absence marker.

The processor must not add a third `Level` value representing “unknown.” Instead, missing authoritative external input is represented by the machine lifecycle.

## 7. First transaction

The first successful transaction initializes the machine.

It must provide:

* an initial logical time (T_0);
* a complete `InputSnapshot`;
* any topology patch effective at (T_0), if supported.

An `InputDelta` is invalid before initialization because there is no previous external valuation.

The first reaction uses:

```text
declared initial stored state
+ complete external level snapshot
+ initial pulse batch
+ no prior due events
        ↓
evaluate reaction graph
        ↓
current outputs
proposed successor state
future temporal obligations
diagnostic episodes
provenance roots
        ↓
atomic initialization commit
```

The ordinary reaction evaluator is used. Initialization does not use a reduced or special evaluator.

## 8. Initial state behavior

Declared initial state acts as the previous stored state supplied to the first reaction.

A stateful node may therefore change immediately in response to the initialization batch.

For example, a toggle with initial `Low` and one initialization pulse produces `High` and commits `High` as its successor state.

## 9. Edge-detector initialization

Edge detectors have explicit initialization policy.

### 9.1 Baseline

The previous observation begins unestablished.

On the first reaction:

* no edge pulse is emitted;
* the current input becomes the remembered observation.

### 9.2 Assume

The configured initial level acts as the previous observation.

The first current input is compared normally and may emit an edge pulse.

## 10. Output establishment

A level output has no prior observable value before initialization.

The first committed value therefore requires an establishment event conceptually distinct from an ordinary transition:

```text
LevelEstablished {
    output,
    value,
    at,
    cause,
    revision,
}
```

Subsequent changes use:

```text
LevelChanged {
    output,
    from,
    to,
    at,
    cause,
    revision,
}
```

The same distinction applies when reconfiguration adds a new external level output.

Removing an output is reported as a topology change, not as a fabricated transition to `Low` or “no signal.”

Pulse outputs generated during initialization are ordinary pulse events.

## 11. Periodic initialization

A newly spawned periodic node has:

```text
no phase anchor
no next deadline
no prior enabled state
```

The first settled `High` enable establishes the initial phase anchor at (T_0).

* `Immediate` may emit during the initialization reaction.
* `AfterFirstPeriod` schedules the first future boundary.
* `PreservePhase` has no earlier phase to preserve until an anchor has first been established.

## 12. Pre-initialization inspection

Before initialization, structural inspection is available:

* topology;
* node definitions;
* declared initial state;
* metadata;
* graph structure.

Current runtime inspection is unavailable:

* current port values;
* current outputs;
* current explanations;
* pending events;
* schedule;
* active runtime diagnostics.

Such requests must return a structured not-initialized failure.

An uninitialized machine is not `Dormant`. Dormancy applies only to a settled ready machine with no pending future work.

---

# Part III — Graph model and causality

## 13. Structural graph

The authored network forms a directed typed structural graph containing:

* nodes;
* ports;
* connections;
* external endpoints;
* modules;
* stable keys.

It may be a multigraph because distinct ports and connections retain identity even when they share structural subjects.

The structural graph is authoritative for:

* identity;
* persistence;
* diagnostics;
* patching;
* hierarchy;
* graph queries.

It does not by itself define reaction evaluation order.

## 14. Reaction equations

Each built-in node is interpreted as a deterministic synchronous transducer.

During one logical reaction, it may read:

* current signal inputs;
* previous committed state;
* temporal obligations due now.

It may produce:

* current signal outputs;
* proposed successor state;
* future temporal operations;
* diagnostic-condition updates;
* provenance facts.

Formally:

[
(current\ outputs,\ next\ state,\ future\ effects)
==================================================

F(previous\ state,\ current\ inputs,\ due\ events)
]

Current outputs are available to downstream reaction equations during the same reaction.

Proposed state is not visible as stored state during that reaction. Each state cell has one previous value and at most one proposed successor value, committed once at the end.

## 15. Reaction dependency graph

Compilation derives a current-reaction dependency graph (G_R).

Conceptually, its vertices represent reaction facts and deterministic evaluation operations, including:

* external current inputs;
* previous-state facts;
* due-event facts;
* node reaction operations;
* current signal outputs.

An edge:

[
u \rightarrow v
]

exists when the current-reaction result of (v) may depend on the current-reaction result of (u).

Proposed successor state and future temporal events are terminal products of the current reaction. They do not feed current-reaction evaluation.

The implementation may fuse several conceptual equations into one compiled evaluator where doing so preserves the same dependency relation.

## 16. Dependency-specific causality barriers

A whole node is not inherently a causality barrier.

A particular dependency path breaks current-reaction causality only when it crosses:

* previous-reaction stored state; or
* strictly later logical time.

A node may contain both instantaneous and delayed dependency paths.

Examples:

* a toggle’s current pulse input affects its current output immediately;
* its proposed successor state does not feed the same reaction;
* a pulse delay’s current input affects future events, not current output;
* a due pulse-delay event affects current output;
* a periodic node’s current `enable` may affect whether a due boundary emits.

The required invariant is:

[
G_R \text{ is a directed acyclic graph}
]

No unspecified fixed-point iteration or same-time microstep semantics are used.

## 17. Static dependency signatures

Each built-in node kind must define a conservative current-reaction dependency signature.

A dependency exists if one current input may affect one current output under any valid state or parameter configuration.

Static causality must not depend on current runtime values.

For example, all possible `Select` branches contribute structural dependencies even though only the selected branch contributes current causal support.

This keeps network validity independent of the current signal valuation.

## 18. Strongly connected components

Current-reaction cycle detection should use strongly connected component decomposition.

A component is cyclic when:

* it contains more than one vertex; or
* it contains one vertex with a self-loop.

Every directed cycle lies within an SCC, so SCC decomposition gives a complete cycle criterion.

The implementation may use Tarjan’s, Kosaraju’s, or another established SCC algorithm.

Diagnostics should identify:

* the cyclic reaction operations;
* the corresponding node and port path;
* the dependency through each node;
* any expected state or temporal barrier that does not actually break the cycle.

## 19. Topological order

Because (G_R) is acyclic, it admits a topological ordering.

Compilation should choose one deterministic linear extension, using stable structural ordering to break ties where appropriate.

The selected topological order is not itself semantic.

Every valid topological ordering must produce equivalent:

* current outputs;
* proposed successor state;
* future temporal operations;
* diagnostics;
* provenance structure, up to non-semantic identifier renaming and ordering of unordered supporters.

## 20. Regions

Weakly connected regions are the connected components of the structural graph after edge direction is ignored.

They uniquely partition the structural subjects.

Regions may support:

* graph queries;
* visualization;
* diagnostics;
* observer scoping;
* execution pruning.

They are not independent machine lifecycles.

Merging or splitting regions does not itself reset, migrate, or reinitialize surviving node state.

The initial implementation should recompute regions after topology changes using ordinary graph traversal. Fully dynamic connectivity should be introduced only if profiling justifies its complexity.

---

# Part IV — Compilation

## 21. Compilation responsibilities

Compilation transforms a validated network into an immutable executable representation.

It should derive:

* stable-key lookup tables;
* dense node and port indices;
* reaction-operation descriptors;
* reaction dependency adjacency;
* deterministic topological ordering;
* predecessor and successor ranges;
* state-family storage layouts;
* temporal-node descriptors;
* external endpoint tables;
* region membership;
* graph-query indices;
* semantic fingerprint;
* inspection-resolution metadata.

## 22. Compiled invariants

A compiled network must establish:

* every dense reference is in bounds;
* every node descriptor matches its node kind;
* every port has the correct signal kind;
* every connection satisfies driver rules;
* every reaction dependency advances in topological order;
* the reaction graph is acyclic;
* every state slot belongs to the correct state family;
* region membership partitions structural subjects;
* stable-key lookup is unambiguous;
* endpoint tables are complete and type-correct.

The runtime may rely on these invariants without revalidating them during ordinary reaction evaluation.

## 23. Semantic fingerprint

The compiled fingerprint is derived from canonical semantic structure, including:

* stable structural keys;
* node kinds;
* typed ports;
* connections;
* semantic parameters;
* state-relevant module structure;
* signal-semantics version.

It excludes:

* dense-index assignment;
* memory layout;
* insertion order;
* hash iteration;
* presentation metadata;
* diagnostic names and descriptions.

Equivalent stable-keyed semantic definitions must produce the same fingerprint regardless of construction order.

---

# Part V — Runtime storage

## 24. Storage by semantic family

Runtime state should be partitioned by semantic family rather than stored as one heap-allocated trait object per node.

Recommended families include:

```text
persistent level values
reaction-scoped pulse counts
edge-detector state
toggle state
latch state
sample-hold state
temporal-node state
external output baselines
active diagnostic episodes
pending event records
provenance roots
```

A compiled evaluator descriptor identifies its node kind and relevant dense slots.

This improves:

* locality;
* snapshot encoding;
* migration;
* inspection;
* state-schema validation;
* closed-set dispatch.

The exact enum, grouped table, generated-dispatch, or data-layout strategy remains provisional.

## 25. Persistent levels

Level ports have persistent current values.

Propagation continues only when a newly evaluated level differs from the current stored value for the candidate reaction.

## 26. Reaction-scoped pulses

Pulse values exist only within one logical reaction.

They are multiplicities in:

[
\mathbb{N}
]

A pulse port stores the complete simultaneous count, not a sequence of arrivals.

A recommended sparse representation uses generation stamps:

```text
pulse_count[slot]
pulse_generation[slot]
current_generation
```

A slot from another generation is semantically zero.

This avoids clearing the complete pulse array before every reaction.

## 27. Previous and proposed state

Stateful evaluation must distinguish:

```text
previous committed state
proposed successor state
```

Every state transition reads the same previous-state vector.

A state cell must not be overwritten while another reaction operation may still require its previous value.

A sparse proposed-transition set may contain:

```text
state slot
previous value
proposed value
cause
```

All successor values commit atomically after successful reaction evaluation.

## 28. Adjacency

Compiled reaction adjacency should use compact immutable storage.

Compressed predecessor and successor ranges are recommended, but the exact representation is not normative.

The compiled form must support efficient traversal in every direction required by:

* settlement;
* explanation;
* graph queries;
* inspection planning;
* patch invalidation.

## 29. Event records and deadline index

Temporal storage has two separate responsibilities:

1. stable access to individual pending events;
2. chronological extraction of equal-deadline batches.

A recommended structure is:

```text
event arena:
    PendingEventKey → event record

deadline index:
    Time → pending event keys
```

Stable event identity must not depend on heap position.

Generation-checked event keys are recommended if arena slots may be reused.

## 30. Provenance records

Committed provenance records are immutable.

A transaction may create a private provenance arena segment and publish it only at commit.

Current semantic facts store opaque `CauseRef` roots.

Later transactions must not mutate historical derivations retroactively.

---

# Part VI — Reaction evaluation

## 31. Reaction inputs and outputs

A logical reaction receives:

* previous committed machine state;
* one complete same-time external stimulus batch;
* all surviving temporal obligations due now;
* topology and migration facts effective now.

It evaluates the acyclic reaction dependency graph and produces:

* current settled signal outputs;
* proposed successor state;
* future event additions and cancellations;
* external output events;
* diagnostic-episode transitions;
* causal provenance;
* a semantic change set.

These products commit atomically.

## 32. Glitch freedom

Only settled pre-reaction and post-reaction values are semantically observable.

A reaction operation is evaluated only after every affected predecessor has reached its final current-reaction value.

Internal traversal order must not create observable intermediate transitions.

A downstream edge detector compares:

* its remembered previous observation;
* the complete settled current input.

It does not observe evaluator glitches.

## 33. Full reference evaluation

The reference reaction evaluator processes every reaction operation once in a valid topological order.

Because the reaction graph is a DAG and every operation is deterministic over settled predecessors, this produces a unique reaction result.

The central optimization obligation is:

[
IncrementalReaction(G,s,\Delta)
===============================

FullTopologicalReaction(G,s,\Delta)
]

for all valid networks, states, and stimulus batches.

A full reference evaluator should exist in test or debug configurations.

## 34. Incremental dirty propagation

A correct incremental evaluator should:

1. identify changed or newly established source facts;
2. seed their affected successors;
3. process dirty operations in topological order;
4. evaluate each operation at most once per reaction;
5. propagate when a complete current output differs or emits a nonzero pulse batch;
6. stop when no dirty work remains.

An operation must not be evaluated before every affected predecessor has settled.

A plain arrival-order FIFO queue is insufficient unless additional dependency accounting proves that condition.

## 35. Recommended worklist

The initial implementation should use:

* dense topological indices;
* generation-stamped dirty state;
* an ordered worklist.

A min-priority queue keyed by topological position is acceptable initially.

Because every dependency edge advances in topological order, work discovery is monotonic. Later implementations may use rank buckets or specialized monotonic queues.

## 36. Same-time chains through stateful nodes

Current outputs from stateful nodes may affect downstream stateful nodes during the same reaction.

For example:

```text
Pulse
  ↓
Toggle A
  ↓
RisingEdge
  ↓
Toggle B
```

is valid when its reaction dependencies are acyclic.

Each operation computes current outputs and one proposed successor state. All state cells commit together after the complete reaction.

There are no same-time microsteps and no repeated transition of one state cell.

---

# Part VII — Logical time and temporal execution

## 37. Discrete exact time

Logical time is exact and discrete.

For each caller-defined domain `D`:

```text
Time<D>          discrete logical instant
Span<D>          non-negative integral tick count
NonZeroSpan<D>   positive integral tick count
```

The caller defines what one tick means. `mossignal` does not interpret the unit.

Time need not begin at zero. The first transaction establishes an arbitrary initial instant.

The caller may jump directly between distant increasing times. The processor evaluates only meaningful intervening deadlines.

## 38. Time arithmetic

Time supports checked operations such as:

```text
Time + Span       → Time
later - earlier   → Span
Time comparison
Span addition
Span comparison
```

Overflow and invalid subtraction produce structured failure.

Arithmetic must never wrap.

## 39. Event calendar

The semantic event calendar is:

[
C : Time \rightharpoonup FiniteMultiset(PendingEvent)
]

Each deadline maps to one complete unordered finite batch.

Deadlines are totally ordered. Events sharing a deadline have no semantic internal order.

The next deadline is the least element of the calendar domain.

## 40. Strictly future scheduling

Every newly pending event must satisfy:

[
deadline > current\ reaction\ time
]

Immediate behavior is produced directly in the current reaction rather than inserted into the calendar at the current time.

With discrete time, positive delays, finite networks, and finite event creation per reaction, infinitely many temporal steps cannot occur before one finite target time.

## 41. Advancing time

For a transaction at time (T), the processor must:

1. process the least pending deadline strictly earlier than (T);
2. evaluate that deadline’s complete event batch as one reaction;
3. repeat until no earlier deadline remains;
4. finalize any topology patch effective at (T);
5. migrate or cancel events due exactly at (T);
6. combine surviving due events with external input and topology-induced facts;
7. evaluate the current-time reaction.

Intervening deadline reactions remain internal steps of the one outer atomic transaction.

## 42. Large-time-jump equivalence

If intervening deadlines are:

[
d_1 < d_2 < \dots < d_k < T
]

then direct advancement must be observationally equivalent to:

[
Step(d_k)\circ\dots\circ Step(d_1)
]

followed by the reaction at (T).

Equal-deadline events must be processed as one unordered batch for this equivalence to hold.

## 43. Exact-deadline obligations

A pending temporal obligation due at (T) is evaluated from temporal state entering the reaction.

Same-time current signal input modifies or suppresses that obligation only where the node’s semantic definition explicitly declares an instantaneous dependency.

Examples:

* a due pulse-delay event emits regardless of new pulses arriving at (T);
* a due transport transition occurs regardless of a new input transition at (T);
* a periodic boundary may be suppressed by settled `enable = Low` at (T);
* an inertial candidate that survived until (T) matures at (T).

## 44. Inertial deadline boundary

An inertial candidate created at (S) with deadline (D) succeeds if its target remains the input throughout:

[
[S,D)
]

A contradictory input change strictly before (D) cancels it.

A contradictory input change exactly at (D):

* does not cancel the matured transition;
* allows the due transition to affect current output;
* may establish a new opposite candidate from (D).

Therefore current input has no instantaneous dependency on the current output of `InertialDelay`.

## 45. Cancellation

Cancellation removes an event from the semantic calendar.

The implementation may use:

* eager removal;
* tombstones;
* generation invalidation;
* event-table state.

Canceled events:

* do not fire;
* do not determine the next deadline;
* do not appear as pending;
* may remain in optional retained history.

## 46. Event aggregation

Pending events may be aggregated only where an established associative and commutative law preserves observable behavior.

Pulse-delay groups with equal semantic destination and deadline may sum multiplicities while preserving causal contributors.

Events at distinct logical times must not be collapsed without proving equivalence for the complete affected subsystem.

---

# Part VIII — Transactions and atomicity

## 47. State-transition model

Machine execution is a deterministic partial transition:

[
\delta(M,\tau)=
\begin{cases}
(M',R) & \text{on success}\
Failure & \text{on rejection}
\end{cases}
]

Failure atomicity requires:

> If execution fails, the published semantic machine remains exactly (M).

This includes:

* topology;
* revision;
* logical time;
* external levels;
* node state;
* temporal state;
* pending events;
* output baselines;
* provenance roots;
* active diagnostic episodes;
* execution-state digest.

## 48. Single commit point

A transaction has one conceptual publication point.

Before commit, public inspection observes the old machine. After commit, it observes the complete new machine.

No public state may exist in which only part of a transaction has become visible.

## 49. Preparation and commitment

Preparation includes all operations that may fail:

* revision validation;
* input validation;
* earlier-deadline execution;
* topology compilation;
* migration finalization;
* reaction evaluation;
* state staging;
* event scheduling;
* provenance construction;
* diagnostic-episode updates;
* result construction;
* digest calculation;
* resource-budget checks.

Commitment should be small and non-fallible:

```text
install prepared machine roots
return prepared immutable result
```

## 50. Reference execution strategy

Clone-and-swap is the reference atomic implementation:

```text
clone machine
execute transaction on clone
replace original on success
```

The production runtime may use:

* sparse overlays;
* copy-on-write;
* private arena segments;
* prepared replacement structures.

Undo logging is not recommended initially because every mutation requires a complete correct inverse and rollback path.

## 51. Pending-event staging

The candidate calendar is semantically:

[
C' =
(C \setminus Fired \setminus Cancelled \setminus MigratedOut)
\cup Added
\cup MigratedIn
]

The implementation may realize this through overlays or replacement roots, provided all transaction-local reads observe the candidate semantics.

## 52. Output and diagnostic publication

Output events, state changes, diagnostics, migration reports, and semantic change sets are staged values.

The evaluator must not invoke host callbacks during propagation.

Host-visible effects occur only after successful commit through the returned immutable result.

## 53. Forecast

Forecasting executes the same transition function on unpublished candidate state:

[
Forecast(M,\tau)=Apply(Clone(M),\tau)
]

It must use the same:

* reaction evaluation;
* deadline processing;
* migration;
* provenance;
* diagnostics;
* budgets;
* failure semantics.

A forecast is bound to the execution state against which it was evaluated. It does not become automatically committable later.

---

# Part IX — Reconfiguration

## 54. Graph rewrite model

A topology patch is a graph rewrite with an explicit preserved interface:

[
G_{old}
\xleftarrow{}
P
\xrightarrow{}
G_{new}
]

where (P) identifies preserved structural subjects.

Stable keys propose continuing identity. Semantic compatibility confirms whether state and pending work may survive.

## 55. Two-stage patch handling

Patch handling is divided into:

1. topology-dependent structural preparation;
2. state-dependent transaction-time finalization.

### 55.1 Structural preparation

Structural preparation performs:

* graph-rewrite validation;
* compilation of the proposed topology;
* stable-key correspondence;
* reaction-cycle validation;
* static compatibility analysis;
* construction of migration functions;
* construction of pending-event migration rules;
* classification of conditional and unavoidable loss;
* resolved-handle and inspection-plan invalidation analysis.

It produces a reusable prepared patch bound to the base topology revision.

Ordinary state-only transactions do not invalidate this structural preparation.

### 55.2 Transaction-time finalization

At effective time (T), the outer transaction:

1. processes all deadlines before (T) under the old topology;
2. obtains the actual candidate state immediately before the patch;
3. applies the prepared migration functions;
4. classifies actual state and pending-event outcomes;
5. enforces the configured state-loss policy;
6. installs the new candidate topology;
7. migrates events due exactly at (T);
8. evaluates the reaction at (T) under the new topology;
9. commits atomically.

Migration therefore applies to the state actually reached at the patch time, not the state that existed when preparation began.

## 56. Compatibility outcomes

Each stateful or temporal subject receives one compatibility outcome:

```text
Preserve
Migrate
Reset
Reject
```

Compatibility may depend on:

* node kind;
* state schema;
* port shape;
* semantic parameters;
* timing policy;
* module identity;
* current value;
* pending-event payload;
* signal-semantics version.

Compatibility may be directional.

## 57. Structural validity

The planner must prevent dangling structure.

Every connection incident to a removed node or port must itself be removed or explicitly redirected.

Preserved identity should be one-to-one unless a node-specific merge migration defines another operation.

## 58. Pending-event outcomes

Every pending event must receive one outcome:

```text
PreserveDeadline
RecomputeDeadline
TransformPayload
Cancel
Reject
```

No pending event may disappear silently.

## 59. State-loss policy

Under `RejectStateLoss`, commitment is permitted only if the semantic loss set is empty.

Under `AllowReportedStateLoss`, every loss must appear in the plan and transaction result.

Semantic loss may include:

* removed stored state;
* reset state;
* canceled pending work;
* terminated required provenance ancestry;
* removed output baseline.

Dense-handle invalidation is not semantic state loss.

## 60. Topology changes as reaction causes

A topology patch may change current outputs without any external signal change.

After migration, structural changes seed reevaluation of every potentially affected operation in the new reaction graph.

The initial implementation may conservatively evaluate the complete new graph.

An incremental patch reaction must be equivalent to complete evaluation.

Topology changes are causes and invalidation sources. They are not synthetic pulses or levels.

## 61. Output consequences

For a preserved level output:

* compare its prior published baseline under the old topology;
* with its settled value under the new topology.

Emit `LevelChanged` if they differ.

For a new level output, emit `LevelEstablished`.

For a removed output, emit a topology consequence rather than a signal transition.

A topology patch produces a pulse only when the new reaction equations genuinely produce one from current signals, due events, or node-specific initialization semantics.

## 62. Exact patch preview

An exact preview of a future patch is a forecast.

It is bound to:

* base topology revision;
* base execution-state digest;
* effective time;
* runtime policy identity.

The reusable structural patch preparation remains distinct from this state-specific forecast.

---

# Part X — Causal provenance

## 63. Derivation structure

Causal provenance is an immutable labeled acyclic derivation graph, not an execution log.

Some results depend jointly on several facts, so the conceptual structure may contain hyperedges representing unordered joint support.

Examples include:

* `All` producing `High` from all inputs;
* `Zip` producing groups from all pulse inputs;
* state transitions depending on previous state and current input;
* delayed events depending on an originating stimulus and temporal policy.

## 64. Causal order

Every provenance edge must advance in at least one of:

* logical time;
* reaction dependency order;
* migration or checkpoint establishment order.

This provides a well-founded order and implies acyclicity.

A committed provenance cycle is an internal invariant violation.

## 65. Current support and transition cause

Current combinational justification is distinct from the cause of the most recent transition.

Current support is reconstructed from:

* compiled reaction structure;
* current settled values;
* node-specific explanation rules.

Explicit retained provenance is required where current facts depend on historical information that cannot be reconstructed:

* external input origins;
* stored-state establishment;
* pending temporal obligations;
* output transitions;
* migration;
* initialization;
* checkpoint roots.

## 66. Authoritative roots

An explanation may terminate at an authoritative root such as:

* declared initial state;
* external input observation;
* committed caller transaction;
* retained temporal origin;
* topology migration or reset;
* snapshot checkpoint;
* explicit provenance checkpoint.

An unexplained previous value is not a valid terminal root.

## 67. Provenance checkpoints

Provenance may be compacted by replacing older ancestry with explicit authoritative checkpoint roots.

A checkpoint establishes facts such as:

```text
at time T and revision R:
    stateful node N stored value V
    event E was pending with deadline D
    external input X held value L
```

Later explanations may use those facts as premises.

The explanation result must state whether ancestry is:

```text
CompleteFromInitialization
CompleteFromCheckpoint {
    checkpoint_time,
    checkpoint_revision,
}
```

Checkpointing may reduce historical depth but must not leave current facts unexplained.

## 68. Retention

Let (R) be the set of required current roots and policy-selected historical roots.

The required retained provenance is the backward-reachable closure of (R).

Unreachable optional provenance may be collected.

Pruned explanations must terminate at explicit checkpoint or retention boundaries.

## 69. Pulse provenance

Pulse provenance records grouped contribution counts rather than inventing ordered pulse identities.

For example, `Merge` may record:

```text
input A contributed 2
input B contributed 4
result multiplicity = 6
```

Equivalent executions must produce equivalent derivation structures up to:

* renaming of non-semantic provenance IDs;
* canonical representation ordering of unordered supporters.

## 70. Migration provenance

Preserved state may retain causes established under an earlier revision.

Migrated state receives a derivation from:

```text
old state fact
+ migration rule
→ new state fact
```

Reset state receives an explicit reset or initialization cause.

Pending events preserved across revisions retain their original scheduling ancestry plus migration facts.

---

# Part XI — Persistent diagnostics

## 71. Diagnostic episodes

Persistent diagnostic conditions are represented as semantic episodes rather than repeated messages.

Conceptually:

```text
Inactive

Active {
    began_at,
    current_evidence,
    last_material_change,
}
```

A diagnostic event is emitted when the episode:

* begins;
* materially changes;
* optionally resolves.

The same unchanged warning is not emitted on every unrelated transaction.

## 72. Episode identity

Episode identity derives from stable semantic facts such as:

```text
diagnostic code
primary structural subject
condition discriminator
```

It does not derive from rendered wording or provenance arena IDs.

## 73. Semantic state

Active episode state affects whether future transactions emit a “condition began” diagnostic.

Therefore active episodes belong to semantic machine state and participate in:

* snapshots;
* restoration;
* replay;
* execution-state digests;
* migration.

Completed historical episodes may be retained optionally outside execution state.

## 74. Reconfiguration

When the owning subject is:

* preserved compatibly: preserve the episode if its condition retains the same meaning;
* migrated: apply an explicit episode migration rule;
* removed: resolve or terminate the episode with a topology consequence;
* reset: reevaluate the condition from the new initial state.

An episode must not attach to a different subject because a dense slot was reused.

---

# Part XII — Inspection and observer state

## 75. Inspection as projection

Inspection is a pure projection:

[
I_P : MachineState \rightarrow InspectionSnapshot
]

It must not alter:

* execution;
* scheduling;
* migration;
* fingerprints;
* provenance;
* semantic digests.

A snapshot observes one complete committed machine version.

## 76. Stable queries and compiled plans

Stable inspection intent uses structural keys and requested fields.

A compiled plan resolves that intent into:

* dense access paths;
* state-family slots;
* graph slices;
* explanation roots;
* dependency watch sets;
* deterministic output ordering.

The stable query may survive revision changes and be recompiled.

The dense compiled plan is revision-bound and must not silently retarget.

## 77. Observer-layer boundary

Subscriptions, delivery state, and compiled inspection plans live outside the semantic `Machine`.

The observer layer may own:

```text
compiled plans
stable inspection queries
last delivered views
subscription cursors
retained delta history
acknowledgement state
delivery queues
resynchronization state
```

Creating or removing a subscription must not alter machine execution or semantic snapshots.

## 78. Semantic change set

Every successful transaction produces an immutable semantic change set containing sufficient facts for observers, including:

* logical times processed;
* level changes and establishments;
* pulse activity;
* state transitions;
* event additions and removals;
* diagnostic-episode changes;
* topology and region changes;
* provenance-root changes.

This change set exists regardless of whether any subscriber is present.

## 79. Subscription update

A subscription maintains:

[
V_t = I_P(M_t)
]

Its incremental delta must satisfy:

[
ApplyDelta(V_{t-1},\Delta V_t)=I_P(M_t)
]

Observer updates occur after semantic commit.

Observer delivery failure must never roll back the machine.

## 80. Explanation-sensitive subscriptions

A requested explanation may change while the observed output value remains unchanged.

Subscriptions requesting explanation data must therefore track explanation dependencies, not only output transitions.

They may use:

* conservative structural dependency cones; or
* dynamically maintained current-support sets.

Either approach must be equivalent to fresh inspection.

## 81. Cursors and resynchronization

Observer cursors identify committed machine versions and delivery continuity.

Missed or expired history requires explicit resynchronization.

The observer layer must never fabricate a continuous delta sequence when retained history is incomplete.

Client-specific state is excluded from semantic machine snapshots and semantic digests.

---

# Part XIII — Persistence, replay, and identity

## 82. Snapshot sufficiency

A machine snapshot serializes the complete semantic state sufficient to determine all future behavior.

This is the machine-level Markov property:

> Given an equivalent restored state and the same future compatible transaction sequence, behavior does not depend on omitted earlier execution history.

## 83. Snapshot contents

A semantic snapshot includes:

* lifecycle state;
* logical time;
* topology revision;
* network fingerprint;
* semantic versions;
* external level valuation;
* stateful and temporal-node state;
* pending event calendar;
* output baselines;
* required provenance roots or checkpoints;
* active diagnostic episodes;
* execution-state digest;
* observable-state digest;
* runtime policy identity where required.

It excludes:

* dense indices;
* heap positions;
* dirty generations;
* worklists;
* compiled inspection plans;
* subscriber delivery state;
* memory-layout details.

## 84. Restoration

Restoration is validation, not deserialization alone.

It must validate:

* schema and semantic versions;
* fingerprint compatibility;
* stable subject existence;
* state schemas and values;
* pending-event ownership;
* future deadlines;
* output baselines;
* provenance roots and acyclicity;
* active diagnostic episodes;
* duplicate or missing facts;
* runtime policy compatibility where exact operational replay is required.

Snapshots must not contain trusted dense runtime indices.

Ordinary restoration must not silently perform topology or semantic migration.

## 85. Replay

Replay is repeated application of the deterministic transition function:

[
Replay(M,T)=foldl(\delta,M,T)
]

Replay frames should include:

* expected previous execution-state digest;
* expected revision;
* transaction;
* resulting execution-state digest.

Snapshot plus replay frames must reproduce the same semantic results and final execution state as uninterrupted execution.

## 86. Replay concatenation

For transaction sequences (A) and (B):

[
Replay(M,A+!!+B)
================

Replay(Replay(M,A),B)
]

where every transaction remains valid at its application point.

This supports:

* checkpointing;
* chunked replay;
* resumption;
* divergence localization.

## 87. Digest scopes

The architecture distinguishes three digest scopes.

### 87.1 Execution-state digest

Covers state capable of affecting future execution:

* topology fingerprint;
* logical time;
* revision;
* external levels;
* stateful and temporal state;
* pending events;
* active diagnostic episodes.

It excludes optional retained history and subscriber state.

This digest is used for:

* replay-chain validation;
* forecast freshness;
* exact state-sensitive patch previews;
* evaluator divergence detection.

### 87.2 Observable-state digest

Extends execution state with current inspectable facts such as:

* current output baselines;
* required current provenance roots;
* current explanation checkpoint state;
* current diagnostic evidence.

It supports comparison of complete current inspection behavior.

### 87.3 Snapshot digest

Covers the complete canonical snapshot artifact, including:

* observable state;
* retained optional provenance history;
* checkpoint metadata;
* persisted observation metadata;
* schema versions.

Two snapshots may share an execution-state digest while differing in retained historical depth.

## 88. Canonical encoding

Digests derive from canonical semantic encoding independent of:

* dense-index assignment;
* memory addresses;
* allocation order;
* hash iteration;
* heap shape;
* presentation metadata.

Unordered sets and multisets are canonically ordered only for representation. That ordering is not semantic causality.

Digest inequality proves inequality under the canonical encoding.

Digest equality is a strong practical consistency check, not a formal proof against hash collision.

---

# Part XIV — Runtime policy and replay

## 89. Runtime policy

Configurable limits that may change transaction success or failure are explicit immutable runtime policy.

Examples include:

```text
maximum internal reactions
maximum evaluated operations
maximum pending events
maximum events created per transaction
maximum required provenance growth
```

Performance-only tuning that cannot affect semantic results is excluded.

## 90. Runtime policy identity

Semantically relevant policy fields contribute to a canonical:

```text
RuntimePolicyId
```

Exact operational replay requires:

```text
same semantic versions
same initial topology and state
same transaction sequence
same RuntimePolicyId
```

Machines may share execution state while using different policies.

The exact replay identity is therefore conceptually:

```text
ExecutionStateDigest
+
RuntimePolicyId
```

## 91. Budget failure

Exceeding a budget:

* fails the complete outer transaction;
* leaves the published machine unchanged;
* identifies the exceeded budget;
* reports the configured limit and consumed amount where practical;
* never skips work or publishes partial settlement.

Optional historical provenance retention should not cause semantic failure where it can instead be pruned.

Limits on required current provenance do affect operational behavior and belong to runtime policy identity.

---

# Part XV — Errors and containment

## 92. Invalid caller-controlled data

Examples include:

* malformed definitions;
* stale revisions;
* wrong signal kinds;
* invalid time progression;
* corrupted snapshots;
* incompatible patches.

These produce structured errors or diagnostics and must not panic.

## 93. Expected semantic rejection

Examples include:

* conflict-rejecting state transitions;
* prohibited state loss;
* checked time overflow;
* runtime budget exhaustion.

These are ordinary deterministic runtime failures and preserve transaction atomicity.

## 94. Internal invariant violations

Examples include:

* a compiled reaction edge violates topological order;
* a state slot belongs to the wrong family;
* a migrated event has no valid owner;
* committed provenance contains a cycle;
* a stable-key lookup is ambiguous after successful compilation.

These indicate processor defects.

They should trigger assertions, panics, or another explicit defect policy rather than being misreported as caller error.

The library guarantees atomicity for structured semantic failures. It does not promise recovery from arbitrary panics, memory corruption, or undefined behavior.

## 95. Unsafe code

The initial implementation should prefer safe Rust.

Any later `unsafe` optimization must:

* be isolated;
* document the exact relied-upon invariant;
* retain a safe reference behavior;
* be differentially tested;
* validate preconditions in debug or test builds where practical.

---

# Part XVI — Subsystems and crate boundaries

## 96. Initial crate structure

The initial implementation should use one cohesive Rust library crate with strong internal module boundaries.

A crate split is justified only by a genuine independent:

* public API;
* dependency surface;
* reuse case;
* release policy;
* target environment;
* compilation constraint.

Premature crate fragmentation would complicate cross-cutting identity, diagnostics, provenance, and reconfiguration work without improving correctness.

## 97. Internal dependency direction

The internal subsystem dependency graph should remain acyclic.

A plausible conceptual layering is:

```text
fundamental types and diagnostics
        ↓
definition and node schemas
        ↓
validation and compilation
        ↓
runtime state and transaction engine
        ↓
provenance, inspection, reconfiguration, persistence
        ↓
public ergonomic façade
```

Lower layers must not depend on higher-level presentation or host-integration concerns.

In particular:

* node evaluation must not depend on rendered diagnostic text;
* persistence must not serialize private runtime containers directly;
* the evaluator must not invoke host bindings;
* provenance must not depend on prose rendering;
* subscriptions must not influence semantic execution.

---

# Part XVII — Performance direction

## 98. Compilation complexity

SCC decomposition, topological ordering, region discovery, and adjacency construction should be approximately:

[
O(|V|+|E|)
]

Canonical sorting for fingerprints may add:

[
O(|V|\log |V| + |E|\log |E|)
]

where required.

## 99. Reaction complexity

Incremental reaction evaluation should approach:

[
O(|V_a|+|E_a|)
]

plus worklist overhead, where (V_a) and (E_a) are the affected reaction closure.

Full topological evaluation remains the reference oracle.

## 100. Temporal complexity

Ordinary deadline insertion and extraction may initially use:

[
O(\log N)
]

calendar operations.

Advancing across a large time span may legitimately require work proportional to the number of meaningful intervening reactions.

No constant-time shortcut is required where distinct logical times have distinct observable effects.

## 101. Reconfiguration complexity

Full recompilation and full region recomputation are acceptable initially.

Incremental graph algorithms should be introduced only when:

* profiling demonstrates need;
* their exact update model is understood;
* a complete rebuild oracle remains available.

## 102. Inspection and persistence complexity

Direct dense-field inspection should be approximately constant time.

Graph slices, explanations, complete snapshots, and complete digests are proportional to the relevant traversed semantic structure.

---

# Part XVIII — Reference paths and verification hooks

## 103. Reference implementations

The architecture should retain simple correctness references:

* full topological reaction evaluator;
* clone-and-swap transaction execution;
* ordered-map event calendar;
* full graph recompilation;
* full region recomputation;
* stable-keyed snapshot form;
* canonical semantic digest input.

Optimized paths should be differentially checked against them.

## 104. Debug invariant checks

Debug and test configurations should support expensive validation such as:

* recomputing SCCs;
* verifying every reaction edge advances topologically;
* comparing incremental and full reaction results;
* recomputing calendar minimum;
* checking provenance acyclicity;
* recomputing regions;
* validating active diagnostic episodes;
* recomputing canonical digests;
* snapshot round-trip checking.

Compilation proves static invariants once. Runtime maintains dynamic invariants incrementally. Debug checks independently recompute them.

---

# Part XIX — Deliberately unspecified choices

## 105. Open implementation freedom

This specification does not mandate:

* exact vector, map, heap, or arena types;
* dense-index widths;
* `Arc` or another sharing mechanism;
* structure-of-arrays versus array-of-structures everywhere;
* enum dispatch versus generated evaluator tables;
* provenance interning;
* timing wheels or calendar queues;
* incremental SCC maintenance;
* fully dynamic connectivity;
* parallel reaction evaluation;
* SIMD execution;
* custom allocators;
* incremental fingerprint maintenance;
* exact digest algorithm;
* serialized wire encoding.

These choices may evolve if they preserve the architectural invariants and reference equivalences defined here.

---

# Part XX — Required architectural properties

## 106. Required guarantees

The processor architecture must preserve:

```text
explicit uninitialized and ready machine phases
complete first-snapshot initialization
validated acyclic reaction dependencies
dependency-specific state and temporal barriers
deterministic topological reaction evaluation
work-order invariance
unordered simultaneous stimulus batches
glitch-free current values
one proposed successor per state cell per reaction
single atomic state commitment
exact discrete caller-owned time
strictly future pending work
chronological deadline processing
large-time-jump equivalence
explicit exact-deadline node semantics
atomic transaction publication
two-stage patch preparation and finalization
complete state and event migration classification
topology-induced reaction settlement
revision-safe dense access
authoritative provenance checkpoints
persistent diagnostic episodes
inspection non-interference
observer-state separation
snapshot sufficiency
replay and forecast equivalence
explicit digest scopes
explicit runtime-policy identity
structured failure boundaries
```

Optimizations are valid only when they preserve these properties.

---

# Summary

`mossignal` is implemented as a deterministic synchronous transition system over an immutable compiled reaction graph and a mutable semantic store.

Its core architecture is:

```text
stable authored structure
        ↓ validation
acyclic reaction dependency system
        ↓ compilation
immutable dense executable topology
        +
mutable semantic machine state
        ↓ transaction
chronological deadline advancement
        ↓
single topological reaction evaluation
        ↓
proposed state and future obligations
        ↓
atomic publication
```

Stateful nodes are synchronous transducers: their current outputs may propagate during the reaction, while their successor state commits once afterward.

Temporal nodes distinguish due obligations from current inputs through explicit dependency signatures and exact-deadline rules.

Topology patches are prepared structurally, finalized against the actual state reached at their effective time, and evaluated as causes in the new reaction graph.

Current causal explanation is preserved through immutable derivations and authoritative checkpoint roots rather than unlimited execution logs.

Simple reference algorithms define correctness. Optimized implementations remain replaceable and must not become hidden semantics.
