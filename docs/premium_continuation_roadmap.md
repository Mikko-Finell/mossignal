# `mossignal` Premium Continuation Implementation Roadmap

**Status:** Official high-level continuation roadmap, approved for detailed task preparation
**Purpose:** Extend the completed premium foundation into a reusable module-based authoring toolkit, then stabilize semantic identity, add durable execution artifacts, and mature operational capabilities without replacing the established architecture.

---

## 1. Relationship to the completed premium roadmap

The original premium roadmap established the implementation foundations through
item 26:

```text
authored structure
→ validation and reaction causality
→ semantic identity
→ compilation and reference evaluation
→ exact-bound inputs
→ machine lifecycle and atomic transactions
→ typed authoring
→ level, pulse, stateful, and temporal execution foundations
```

This continuation begins after those foundations have completed implementation
and corrective acceptance review. It does not reopen them merely to pursue a
different representation or improve style.

The continuation keeps the original numbering. Its first continuation item is
item 32.

The authoritative specifications under `docs/specs/` remain product authority.
This roadmap defines sequencing and task boundaries only. It does not replace
specification research, specification contracts, implementation beads, or
independent review.

---

## 2. Continuation strategy

The next premium phase should make the established execution kernel useful at a
higher level before pursuing broad optimization.

The intended progression is:

```text
proven primitive execution foundations
        ↓
reusable user-defined modules and retained hierarchy
        ↓
versioned standard modules and concise conveniences
        ↓
application bindings and execution façades
        ↓
remaining primitive families required by the public catalogue
        ↓
stable semantic identity baseline
        ↓
durable snapshots, restoration, and replay
        ↓
forecasting, reconfiguration, richer observation, and measured optimization
```

The module system is not merely builder convenience. It introduces stable
interface identity, reusable validated definitions, encapsulation, hierarchy,
module fingerprints, canonical instantiation, and compatibility boundaries.
Accordingly, it remains premium architectural work even though it eventually
enables substantial mechanical and convenience-oriented expansion.

Primitive breadth should be added when it proves an established extension
pattern or unlocks a specified module or convenience. The project should not
delay modules until every primitive exists, nor add every primitive before a
concrete higher-level consumer requires it.

---

## 3. Task preparation and authority

Every implementation item in this roadmap must be converted into a bounded bead
through the `contract-task-prep` workflow before implementation.

Task preparation must:

1. inspect the current repository baseline and working tree;
2. reuse applicable unchanged reviewed contracts;
3. research only changed or uncovered authoritative specification facets;
4. create or update the smallest coherent set of draft contracts;
5. derive one bounded implementation-ready bead;
6. preserve ordinary underspecified choices as deterministic implementation
   freedom;
7. stop before implementation;
8. obtain independent review of changed contracts and the bead before the
   implementation is accepted as ready.

Roadmap wording is provisional task scope, not reusable product truth. When a
roadmap item and an authoritative specification differ, task preparation must
follow the specification and refine the bead accordingly. It must not silently
turn the roadmap into authority.

The next three or four items should be prepared in detail at any one time.
Later items should inherit the concrete types and accepted patterns that actually
land rather than independently designing the entire continuation in advance.

---

# Part I — Module composition

## 32. [DONE] Module interface identities and unchecked definitions

Establish the stable authored vocabulary for reusable modules:

* stable module-instance keys;
* typed module input and output keys;
* owned module interface definitions;
* an unchecked dynamic module representation;
* private internal authored structure using the existing stable node, port, and
  connection identities;
* explicit user-module origin;
* malformed-definition retention suitable for later structured validation;
* deterministic construction, equality, and ordering tests.

Reuse the existing signal kinds, primitive node definitions, metadata values,
stable-key conventions, and unchecked authored-graph principles.

Explicitly exclude:

* module validation;
* `ModuleDef`;
* typed `ModuleBuilder` convenience;
* module instantiation into networks;
* nested modules;
* compiled flattening;
* standard catalogue declarations;
* persistence and reconfiguration.

This item establishes module structure without yet claiming that a module is
valid or executable.

---

## 33. [DONE] Module validation, encapsulation, and semantic identity

Implement the validated reusable module artifact:

* structural validation of module interfaces and internals;
* reaction-dependency and cycle validation through the established graph
  machinery;
* complete output-source and interface-kind validation;
* private-internal encapsulation rules;
* immutable `ModuleDef` ownership;
* user-module origin retention;
* canonical `ModuleFingerprint` construction;
* deterministic module graph and interface inspection;
* blocking-diagnostic suppression of invalid artifacts;
* insertion-order, metadata-exclusion, identity-sensitivity, and golden-vector
  tests.

Module identity must depend on stable semantic structure and declared interface,
not on dense indices, insertion order, or diagnostic presentation metadata.

Explicitly exclude:

* typed module authoring;
* network instantiation;
* nested-module execution;
* standard catalogue origin;
* runtime state;
* persistence envelopes;
* compatibility migration.

At the end of this item, callers can validate and retain reusable user-defined
module artifacts even though networks cannot instantiate them yet.

---

## 34. [DONE] Typed `ModuleBuilder` foundation

Establish typed user-module authoring by adapting the accepted network-builder
pattern:

* builder-scoped typed signals;
* typed module level and pulse inputs;
* typed module level and pulse outputs;
* the currently implemented primitive constructor family;
* explicit stable-key construction forms;
* foreign-signal and duplicate-identity rejection;
* deterministic lowering into the unchecked module representation;
* `finish` through ordinary module validation;
* typed-versus-dynamic module equivalence tests.

The module builder must reuse the authoritative unchecked representation and the
same primitive semantics as `NetworkBuilder`. It must not create a second graph,
validation, or evaluator architecture.

Explicitly exclude:

* module instantiation;
* nested module authoring;
* standard-module constructors;
* application bindings;
* persistence;
* macros or a new authoring language.

This item makes simple reusable modules pleasant to author without yet composing
modules into larger definitions.

---

## 35. [DONE] Exact module instantiation and retained hierarchy

Allow validated modules to be instantiated through `NetworkBuilder` and
`ModuleBuilder`:

* one stable module-instance identity;
* exact typed binding of every public module input;
* rejection of missing, duplicate, unknown, wrong-kind, and foreign-builder
  bindings;
* typed access to public module outputs;
* dynamic module-instance definitions and owned binding sets;
* nested module instantiation;
* logical internal identity based on instance identity plus module-internal
  stable identity;
* public encapsulation of private internals;
* deterministic hierarchy and binding validation;
* generic instantiation equivalence across typed and dynamic construction.

Hierarchy must remain semantically attributable even if later compilation
flattens execution.

Explicitly exclude:

* compiled module execution;
* standard catalogue shortcuts;
* module replacement;
* state migration;
* snapshot encoding;
* optimized sharing or incremental compilation.

At the end of this item, authored and validated networks can contain nested
module instances, but compiled execution integration remains the next boundary.
Until item 36 supplies that integration, `compile` and `compile_ref` reject a
validated network containing module instances with the structured
`compilation.unsupported_module_instances` report. This is a temporary staged
boundary, not permanent product behavior.

---

## 36. Module-aware compilation, execution, and observation

Extend the accepted compilation and runtime paths across module hierarchy:

* deterministic flattening or equivalent executable lowering into existing
  primitive descriptors;
* preservation of module, instance, and internal stable correspondence;
* module-aware network fingerprints;
* retained hierarchy for graph views, diagnostics, provenance, and inspection;
* execution through the ordinary reaction, state, temporal, transaction, and
  budget machinery;
* nested-module output and causal conformance;
* failure atomicity across module-expanded execution;
* equivalent behavior between explicit primitive graphs and their module-wrapped
  forms where the specifications require behavioral rather than identity
  equivalence.

This item must make the temporary
`compilation.unsupported_module_instances` condition unreachable for supported
module instances, remove the item-35 rejection-path tests, and retire the
temporary diagnostic implementation, evidence surface, catalogue entry, API
rule, and contract rule. None of that temporary scaffolding is compatibility
behavior unless later authoritative policy explicitly reserves it for a
distinct unsupported case.

Compilation must not add callback evaluators, hidden state, or module-specific
transaction semantics. Modules execute through their ordinary primitive
expansions.

Explicitly exclude:

* standard catalogue discovery;
* persistence artifacts;
* module replacement and topology patches;
* optimized module-specialized execution;
* complete explanation rendering.

At the end of this item, user-defined modules form one complete public path from
authoring through runtime observation.

---

# Part II — Standard composition and application ergonomics

## 37. Standard catalogue foundation with `Exactly`

Establish the versioned standard-module mechanism using one first stateless
module:

* catalogue and descriptor identity;
* standard-module references and semantic versions;
* typed parameter schemas and validated assignments;
* public role and interface identity;
* canonical stable-keyed primitive expansion;
* explicit standard origin on `ModuleDef`;
* catalogue discovery and exact construction;
* generic and typed instantiation equivalence;
* module-level diagnostics and inspection hooks;
* `Exactly` as the first complete catalogue module;
* exhaustive bounded-arity and valuation conformance.

The standard path must produce an ordinary validated `ModuleDef` and execute
through the generic module and primitive machinery. It must not create a
privileged evaluator extension.

Explicitly exclude:

* `AtMost` and `AllEqual`;
* stateful standard modules;
* convenience aliases unrelated to the first module;
* persistence encoding;
* reconfiguration and migration;
* application-domain modules.

This item establishes the canonical pattern for durable named compositions.

---

## 38. Stateless standard-module expansion

Extend the accepted catalogue pattern mechanically with:

```text
AtMost
AllEqual
```

Implement their exact descriptors, parameter and interface validation,
canonical expansions, typed and dynamic construction paths, module-level
inspection and diagnostics, identity sensitivity, and exhaustive bounded-domain
conformance.

The work may be broad in test count and expansion cases because catalogue
identity, module construction, validation, hierarchy, compilation, and runtime
integration should already exist.

Explicitly exclude:

* stateful catalogue modules;
* new primitive node kinds;
* hidden simplification that changes standard-module identity;
* persistence and migration implementation.

---

## 39. Supported aliases and builder-only conveniences

Implement the convenience classification already expressible through accepted
primitives and modules.

The opening set should include the currently supported subset of conveniences
such as:

```text
xor
majority
nand
nor
xnor
level_gate
```

Each convenience must retain its specified classification:

* primitive aliases create exactly the canonical primitive;
* builder-only operations author ordinary primitive graphs without inventing a
  durable semantic object;
* standard modules retain visible module identity;
* metadata-only operations do not alter behavior or semantic identity.

Provide equivalent methods on `NetworkBuilder` and `ModuleBuilder` where the
specifications require them, together with exact-lowering and identity tests.

Explicitly exclude conveniences whose primitive dependencies are not yet
implemented, including temporal or level-controlled pulse aliases that belong
to later items.

---

## 40. Application bindings and bound execution façade

Add the specified adapter layer between caller-owned application identities and
stable mossignal endpoints:

* immutable typed `BindingSet` values;
* builder validation against one compiled input and output schema;
* caller-owned opaque input and output identifiers;
* bidirectional endpoint lookup;
* `InputProjector` construction of exact network-bound snapshots and deltas;
* projection of output events and current observations back to caller keys;
* an optional `BoundMachine` façade over ordinary machine execution;
* equivalence tests proving that the façade does not change evaluator,
  transaction, scheduling, provenance, or failure semantics.

Bindings must remain non-semantic application adapters. They do not contribute
to network fingerprints, compiled topology, machine state, or replay identity.

Explicitly exclude:

* application callbacks;
* observer subscriptions;
* delivery acknowledgement state;
* domain-specific entity models;
* a general simulation DSL;
* replacement of the structural-key APIs.

At the end of this item, ordinary applications can use their own identifiers
without exposing endpoint-key ceremony throughout their execution code.

---

# Part III — Primitive breadth required by the public toolkit

## 41. Remaining pulse-combinational primitives

Extend the established pulse foundation with:

```text
Coalesce
Zip
```

Carry each primitive through dynamic and typed authoring, structural validation,
current-reaction dependency extraction, cycle validation, compilation,
evaluation, checked pulse arithmetic, provenance, fingerprints, transactions,
and conformance testing.

This work should reuse the established `Merge` and reaction-scoped pulse
patterns while preserving each primitive's distinct multiplicity law.

Explicitly exclude:

* level-controlled pulse nodes;
* stateful nodes;
* temporal scheduling;
* payload transport;
* pulse persistence.

---

## 42. Level-controlled pulse primitives

Implement the regular family:

```text
PulseGate
PulseSelect
PulseRoute
```

Establish their exact level-control and pulse-multiplicity laws, fixed port
roles, current-time control semantics, typed and dynamic authoring, validation,
causality, compilation, reaction execution, provenance, identity, and bounded
conformance.

Current level controls and simultaneous pulse counts must settle through the
ordinary reaction model without hidden ordering or pulse consumption.

Explicitly exclude:

* edge detection;
* stored enable state;
* temporal windows;
* callbacks or host effects.

---

## 43. Transition-sensitive edge-detector family

Extend the stateful foundation with:

```text
RisingEdge
FallingEdge
AnyEdge
```

Implement declared initialization, previous-level ownership, proposed successor
state, same-reaction pulse visibility, exact transition laws, state inspection,
provenance, fingerprints, and exhaustive two-reaction conformance.

The family should share one established state representation and validation
pattern where its members differ only by transition law.

Explicitly exclude:

* debounce or temporal filtering;
* latches;
* counters;
* arbitrary transition callbacks;
* retained pulse history.

---

## 44. Set/reset latch family

Implement the specified pulse- and level-controlled set/reset latch primitives:

```text
PulseSetResetLatch
LevelSetResetLatch
```

Carry their declared initial state, simultaneous set/reset policy, current
output visibility, proposed successor commitment, inspection, provenance,
fingerprints, diagnostics, and exhaustive control-state conformance through the
existing stateful architecture.

Explicitly exclude:

* resettable standard modules;
* `SampleHold`;
* counters and general finite-state machines;
* temporal scheduling;
* state migration.

---

## 45. `SampleHold`

Implement `SampleHold` as the next distinct stateful primitive:

* value and sampling input semantics;
* declared initial held value;
* previous and proposed stored value;
* same-reaction output visibility;
* simultaneous input law;
* focused inspection and provenance;
* typed and dynamic authoring;
* validation, compilation, fingerprints, and conformance.

Reuse the existing state-cell ownership and atomic successor commitment. Do not
introduce a generic arbitrary-state framework merely because the stored value is
selected rather than toggled.

Explicitly exclude:

* resettable standard-module wrappers;
* payload-valued sampling;
* analog values;
* state history;
* topology migration.

---

## 46. `TransportDelay`

Add the first temporal Level-to-Level primitive on the accepted event calendar:

* declared initial input and output state;
* exact delayed transition reproduction;
* multiple pending transitions;
* temporal current-reaction causality barrier;
* pending-event identity and inspection;
* chronological direct-jump execution;
* result-owned provenance;
* runtime-budget accounting;
* atomic checked-time failure;
* typed and dynamic authoring, validation, compilation, fingerprints, and
  conformance.

This item deliberately combines the already-proven state and temporal ownership
patterns. It must not replace the ordered reference calendar or create a
parallel temporal transaction path.

Explicitly exclude:

* cancellation of superseded transitions;
* inertial filtering;
* recurring events;
* duration-change migration;
* calendar optimization.

---

## 47. `InertialDelay`

Extend temporal execution with cancellation and replacement semantics:

* one current candidate transition;
* exact maturity deadline;
* replacement or cancellation when input changes before maturity;
* exact behavior when maturity and new input share one logical time;
* stable public pending-work identity and inspection;
* provenance for matured and canceled/replaced work as required by the
  specifications;
* atomicity and policy enforcement;
* direct-jump versus stepwise conformance.

Reuse the same machine-owned event calendar, chronological transaction loop,
state staging, and result publication model established by `PulseDelay` and
`TransportDelay`.

Explicitly exclude:

* recurring schedules;
* generic cancellation handles;
* retained canceled-event history beyond required observation;
* reconfiguration migration;
* `Debounce` convenience spelling until the primitive is accepted.

---

## 48. `Periodic`

Implement the first recurring temporal primitive:

* declared period and phase semantics;
* enable behavior;
* exact boundary scheduling;
* recurring pending work owned by the machine calendar;
* due-boundary and same-time enable interaction;
* schedule and pending-work inspection;
* chronological direct-jump behavior across several boundaries;
* runtime-policy enforcement preventing unbounded hidden progress;
* provenance, fingerprints, and conformance.

The primitive must schedule network-owned logical work only. It must not read a
wall clock, sleep, create a thread, or advance a machine automatically.

Explicitly exclude:

* absolute-time sources;
* cron-like calendars;
* generic timer callbacks;
* topology migration;
* optimized timing wheels.

---

# Part IV — Complete the initial high-level catalogue

## 49. Stateful standard modules

Using the accepted module, catalogue, latch, and sample-hold machinery,
implement the initial stateful standard modules:

```text
PulseResettableToggle
LevelResettableToggle
LevelResettableSampleHold
```

Each module must have its exact descriptor, stable public roles, canonical
primitive expansion, declared parameters, module-level state inspection,
explanation hooks, identity, and exhaustive simultaneous-control conformance.

The modules must expose their visible durable module boundaries while all state
continues to belong to their ordinary internal primitives.

Explicitly exclude:

* hidden module-specific state;
* generic reset injection;
* counters;
* general finite-state machines;
* migration implementation before the topology-patch phase.

---

## 50. Remaining specified conveniences

Complete the initial convenience surface whose dependencies now exist,
including aliases or builder-only operations such as:

```text
debounce
any_pulse
```

Each convenience must lower to its specified canonical primitive or ordinary
graph, preserve its assigned classification, and expose no false module,
state, migration, or fingerprint boundary.

Verify equivalent behavior and exact semantic identity against the direct
canonical construction path.

Explicitly exclude:

* a general HDL or simulation language;
* macros that obscure stable identity;
* unclassified convenience proliferation;
* application-domain component catalogues.

At the end of this item, the initial standard catalogue and specified
convenience surface should be complete over the implemented primitive language.

---

# Part V — Semantic identity stabilization and durable execution

## 51. First stable semantic-identity baseline

After modules, the initial standard catalogue, and the intended opening
primitive families exist, establish the first explicit compatibility baseline
for semantic identity.

This item should:

* inventory every supported canonical node, port role, module, interface,
  hierarchy, parameter, state schema, and temporal schema contribution;
* finalize the applicable fingerprint projection versions;
* freeze exact domain labels, version inputs, canonical field names, variant
  names, and collection order for the stable baseline;
* define how later compatible additions and incompatible changes select
  successor versions;
* preserve metadata and private-representation exclusions;
* add complete compatibility fixtures and hand-reviewed golden vectors;
* document the stability commitment clearly.

This item may require authoritative specification refinement before contract and
bead review. It must not silently declare the existing pre-stability projection
stable without reconciling all represented identities.

Explicitly exclude:

* snapshot state encoding;
* topology patches;
* support for hypothetical future node kinds;
* compatibility promises for private dense representation.

---

## 52. Execution-state and observable-state digests

Implement canonical semantic projections and identities for running-machine
state:

* execution-state digest;
* observable-state digest;
* complete inclusion of lifecycle, revision, time, authoritative inputs,
  stateful state, temporal state, pending events, public identity cursors,
  output baselines, and required observation roots according to their specified
  scopes;
* exclusion of policy-independent caches, allocation layout, subscribers, and
  presentation-only data;
* deterministic construction independent of private storage order;
* semantic-sensitivity and golden-vector tests.

The digests are semantic identities, not yet snapshot containers or decoders.

Explicitly exclude:

* snapshot envelopes;
* restoration;
* replay logs;
* topology migration;
* hash-based shortcuts that replace exact semantic validation.

---

## 53. Canonical snapshot encoding

Implement one versioned, self-contained machine snapshot artifact covering:

* installed semantic topology identity;
* lifecycle and logical time;
* topology revision;
* external authoritative inputs;
* stateful and temporal node state;
* ordered pending-event records and allocation cursors;
* output baselines;
* required provenance closure or checkpoint roots;
* persistent diagnostic state where required;
* runtime policy identity;
* module hierarchy and standard declarations;
* artifact versions, canonical ordering, and integrity digest.

Encoding must use stable semantic identities and exact public state rather than
dense indices, memory layout, or debug representations.

Explicitly exclude:

* decoding and restoration;
* replay logs;
* topology replacement;
* optional compression or streaming formats;
* compatibility migration beyond recording required version information.

---

## 54. Snapshot validation and restoration

Implement strict restoration of supported snapshots:

* artifact framing and integrity validation;
* version and compatibility checks;
* canonical-definition and fingerprint verification;
* module and standard-expansion validation;
* state-owner and signal-kind validation;
* pending-event and provenance reference validation;
* digest recomputation;
* construction of one complete candidate machine;
* atomic publication only after complete validation;
* corruption, mismatch, unsupported-version, and round-trip conformance.

Restoration must never trust persisted dense positions, record ordering, module
expansions, or causal references without validation.

Explicitly exclude:

* topology migration during restoration;
* partial best-effort recovery;
* remote artifact storage;
* replaying history to reconstruct a snapshot;
* format optimization.

---

## 55. Deterministic replay artifacts

Add replay records and verification over the accepted snapshot and transaction
model:

* an exact initial machine or snapshot identity;
* ordered external transactions and applicable topology operations;
* expected result and digest checkpoints;
* deterministic re-execution through ordinary machine APIs;
* mismatch localization;
* temporal direct-jump and stepwise replay equivalence;
* module-aware identity and event verification;
* corruption and incompatible-version rejection.

Replay must use the ordinary deterministic transition function. It must not add
a second evaluator or bypass runtime policy and failure semantics.

Explicitly exclude:

* distributed consensus;
* live replication protocols;
* arbitrary event-sourcing frameworks;
* nondeterministic host callbacks;
* performance-oriented log compaction.

---

# Part VI — Forecasting, reconfiguration, and observation

## 56. Transaction forecasting

Implement non-publishing execution of one proposed transaction:

* the same validation, chronological temporal advancement, reaction settlement,
  budget accounting, provenance, diagnostics, and output construction as
  `apply`;
* a complete owned forecast result;
* no mutation of machine state, event cursors, schedule, or retained provenance;
* exact equivalence between a successful forecast and later application against
  unchanged machine state;
* structured forecast failures matching application failures.

Forecasting should reuse the same deterministic candidate transition rather than
copying transaction semantics into a separate implementation.

Explicitly exclude:

* topology-patch preparation;
* speculative multi-transaction branches;
* background execution;
* caching forecasts as machine state.

---

## 57. Topology-patch preparation and correspondence

Implement structural preparation of one topology replacement:

* old and new validated topology identity;
* stable structural correspondence;
* module-instance and internal-role correspondence;
* input-schema and binding consequences;
* stateful and temporal migration classifications;
* output and diagnostic consequences;
* explicit state-loss evidence;
* deterministic report and prepared-patch artifact;
* no mutation of a running machine.

Preparation must preserve malformed proposed definitions until ordinary
validation and must not infer semantic correspondence from names, metadata, or
dense positions.

Explicitly exclude:

* patch commitment;
* actual state migration;
* hidden default migration policy;
* partial topology installation;
* optimized incremental compilation.

---

## 58. Atomic topology replacement and migration

Commit prepared topology patches through the ordinary transaction boundary:

* exact revision and time checks;
* final state and pending-work migration at the effective time;
* explicit reconfiguration policy;
* module-aware compatibility and replacement;
* external input projection to the target schema;
* topology-induced reaction settlement;
* output establishment, changes, and removal consequences;
* patch provenance and diagnostic publication;
* revision advancement distinct from state progression;
* complete failure atomicity;
* snapshot and replay compatibility.

The implementation must not install an intermediate topology, silently discard
state or pending events, or encode a patch as a synthetic signal.

Explicitly exclude:

* collaborative graph editing;
* live distributed deployment;
* fuzzy name-based migration;
* unreported state loss;
* performance-oriented incremental patch compilation unless separately proven.

---

## 59. Rich graph, inspection, and explanation queries

Build the specified read-only observation capabilities over stable subjects and
retained hierarchy:

* immutable graph views;
* module-aware region and dependency queries;
* slices affecting or affected by stable endpoints;
* compiled inspection plans where required;
* current node, state, temporal, and module inspection;
* structured explanation and why-not results;
* module-level summaries with primitive drill-down;
* exact attribution to result-owned or retained provenance;
* deterministic query and rendering-independent evidence.

Queries must observe semantic possibility or committed state as specified. They
must not mutate execution, fabricate dynamic causality from static reachability,
or expose dense implementation positions as stable identity.

Explicitly exclude:

* editor UI;
* localization and prose styling beyond separately specified rendering;
* subscriber delivery;
* arbitrary query languages;
* execution optimization based on query plans.

---

## 60. Probes, assertions, and persistent diagnostic operation

Add operational observation and checking only after graph, provenance, and
diagnostic ownership are mature:

* probes and named observation points where specified;
* assertion and fault semantics;
* runtime diagnostic occurrences;
* persistent diagnostic episodes;
* stable subjects and lifecycle transitions;
* transaction-atomic publication;
* snapshot, restoration, replay, module, and topology-patch integration;
* deterministic conformance and failure tests.

These facilities must remain semantic observations and diagnostics. They must
not invoke arbitrary callbacks, perform host effects during propagation, or
change circuit behavior unless an explicit specified primitive does so.

Explicitly exclude:

* application actions;
* logging backends;
* notification delivery protocols;
* unrestricted user code in the evaluator;
* hidden test-only node kinds.

---

# Part VII — Performance and further product evolution

## 61. Benchmark and reference-oracle foundation

Before changing execution algorithms, establish reproducible measurement and
semantic comparison:

* representative combinational, pulse, stateful, temporal, module, and
  reconfiguration workloads;
* allocation, operation-count, transaction-latency, and calendar metrics;
* the full evaluator and ordered reference calendar as correctness oracles;
* differential tests for optimized candidates;
* documented benchmark environments and non-normative performance targets;
* regression reporting that cannot weaken semantic acceptance gates.

Explicitly exclude production optimization from this item.

---

## 62. Incremental reaction evaluation

Introduce dirty propagation or another incremental strategy only against the
accepted reference evaluator:

* exact dirty-root derivation from external input, state, due-event, and topology
  changes;
* predecessor-correct operation scheduling;
* identical settled state, output events, provenance obligations, diagnostics,
  and failures;
* deterministic behavior independent of cache state;
* policy accounting with explicitly specified semantics;
* differential and benchmark evidence across all implemented node families and
  nested modules.

Retain the full evaluator as a test oracle and fallback until the optimized path
has independent acceptance.

Explicitly exclude:

* changing public semantics for speed;
* weakening provenance or diagnostics;
* exposing caches as semantic state;
* combining calendar replacement into the same task.

---

## 63. Temporal calendar optimization if justified

Replace or supplement the ordered reference map only if measurements show a
material temporal bottleneck.

Any optimized calendar must preserve:

* exact least-deadline behavior;
* stable public pending-event identity;
* complete equal-deadline batches;
* deterministic event allocation and inspection;
* cancellation and recurring-event semantics;
* chronological transaction execution;
* atomic rollback;
* snapshot, replay, and migration representation;
* differential equivalence against the ordered reference calendar.

Do not optimize the calendar merely because heaps, timing wheels, or event
arenas are conventional.

---

## 64. Further authoring and application ergonomics

After modules, the catalogue, bindings, durability, and operational inspection
are concrete, reassess remaining user friction.

Possible future work may include:

* a focused simulation or test façade;
* additional domain-neutral user-authored module libraries;
* visualization adapters over graph views;
* carefully designed macros or declarative syntax;
* serialization conveniences around stable artifacts;
* application integration packages.

These are not authorized implementation items until their semantics and identity
consequences are specified and prepared through contracts and beads.

The project should not add a general HDL, operator-overloading scheme, macro
language, callback-defined node API, or application framework merely for shorter
examples. Any new surface must preserve explicit stable identity, typed signal
kinds, validation, module boundaries, and deterministic reconstruction.

---

# Part VIII — Planning cadence and reassessment gates

## 65. Recommended planning clusters

Prepare and implement the continuation in small clusters.

Suggested clusters are:

```text
Cluster A: items 32–34
    module structure, validation/identity, typed builder

Cluster B: items 35–36
    instantiation, hierarchy, compilation, execution, observation

Cluster C: items 37–40
    standard catalogue, stateless modules, conveniences, application bindings

Cluster D: items 41–45
    pulse-controlled, transition-sensitive, and retained-state breadth

Cluster E: items 46–50
    temporal breadth, stateful standard modules, remaining conveniences

Cluster F: items 51–55
    stable identity, digests, snapshots, restoration, replay

Cluster G: items 56–60
    forecasting, reconfiguration, rich observation, operational diagnostics

Cluster H: items 61–64
    measurement, optimization, and later ergonomics
```

These clusters are planning horizons, not permission to create one oversized
bead per cluster. Each numbered item should normally become one bounded bead;
task preparation may split an item when authoritative requirements reveal two
independent architectural responsibilities.

After each cluster:

1. perform corrective implementation review;
2. run the complete repository gate;
3. inspect new contract coverage and known-uncovered facets;
4. reassess whether later item boundaries still fit the accepted code;
5. refine only the next small cluster in implementation detail.

---

## 66. Delegation progression

Premium agents should retain responsibility for new architectural boundaries,
including:

* module definitions and identity;
* hierarchy-aware compilation;
* the standard catalogue mechanism;
* semantic identity stabilization;
* snapshot restoration;
* topology migration;
* incremental execution architecture.

Controlled mechanical extensions may be delegated more broadly after their
patterns have independent acceptance, including:

* additional members of an established primitive family;
* additional standard modules using an accepted descriptor and expansion path;
* aliases and builder conveniences with exact canonical lowering;
* focused conformance cases;
* application-binding adapters;
* diagnostic catalogue additions using an accepted evidence pattern.

Delegated work must still use specification contracts, bounded beads,
corrective review, and repository gates. Broader delegation must not mean weaker
semantic or architectural acceptance.

---

## 67. Governing principles

The continuation may tolerate:

* simple flattening algorithms;
* repetitive descriptor code;
* limited catalogue breadth;
* unoptimized canonical encoding;
* full-state staging;
* reference data structures;
* missing optional ergonomic layers.

It must not tolerate:

* a parallel module evaluator;
* module identity derived from presentation names or dense positions;
* hidden state or pending work inside standard modules;
* convenience APIs that create false semantic boundaries;
* application bindings that alter network semantics;
* fingerprint stabilization before represented identities are reconciled;
* snapshots that trust private layout;
* restoration that skips validation;
* replay through a second transition function;
* partial topology commitment;
* optimizations without differential semantic evidence;
* roadmap or bead text becoming independent product authority.

The continuation strategy is therefore:

> Build the specified high-level composition model first, add primitive breadth
> when it unlocks concrete public capabilities, stabilize identity before
> durable compatibility promises, and optimize only against retained reference
> semantics.
