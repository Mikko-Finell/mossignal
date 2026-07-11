# `mossignal` Exhaustive Diagnostic Code Catalogue

**Status:** Design specification, version 1  
**Diagnostic schema status:** Experimental; not yet compatibility-frozen  
**Defines:** The unified problem-code namespace; diagnostic and failure structure; severity, responsibility, delivery, evidence, subjects, suggestions, deterministic collection, persistent diagnostic episodes, internal-defect reporting, persistence rules, verification obligations, and the exhaustive initial code catalogue  
**Does not define:** Rendered English wording, localization, logging backends, editor presentation, host telemetry, post-freeze deprecation periods, or recovery from arbitrary process corruption

---

## 1. Purpose

This specification defines the complete structured problem-reporting system for `mossignal`.

The library must diagnose malformed definitions, invalid operations, semantic rejection, compatibility failure, corrupt persisted data, resource exhaustion, and implementation defects without reducing them to arbitrary strings or undifferentiated error values.

The same catalogue also supports development of `mossignal` itself. Reference-path divergence, invalid compiled state, broken migration accounting, provenance corruption, and other violated implementation invariants must identify the exact failed obligation and retain enough evidence to reproduce it.

The central rules are:

> A code identifies one semantic condition, not one message, call site, API method, Rust enum variant, or presentation choice.

> Diagnostics, operation failures, persistent episodes, and internal defects use one code and evidence system, while retaining distinct delivery and lifecycle semantics.

> Rendered prose is never authoritative.

> Internal implementation defects are never disguised as invalid caller input.

---

## 2. Normative language

The terms **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.

A **problem code** is one machine-readable identifier from this catalogue.

A **problem record** is the common structured description of one condition.

A **diagnostic finding** is a non-panicking problem record collected in a report or successful runtime result.

An **operation failure** is a problem record that rejects one requested operation.

A **diagnostic occurrence** is a committed one-time runtime finding.

A **diagnostic episode** is committed semantic state representing one continuously active condition.

An **internal defect** is a problem record establishing that `mossignal` violated an invariant that valid caller-controlled input was entitled to rely upon.

## 3. Relationship to the other specifications

This specification is authoritative for:

- diagnostic code identity and namespace;
- severity and responsibility assignment;
- allowed delivery forms;
- condition identity and deduplication;
- structured evidence requirements;
- persistent episode identity and lifecycle;
- canonical diagnostic ordering;
- diagnostic-schema compatibility.

The API and semantics specification remains authoritative for which operations, conditions, and state transitions exist.

The built-in-node and standard-module specifications remain authoritative for the semantic conditions attached to those nodes and modules.

The processor specification remains authoritative for internal invariants and execution boundaries.

The reconfiguration specification remains authoritative for migration, loss, and patch-finalization semantics.

The persistence specification remains authoritative for artifact encoding and compatibility, while this specification defines the diagnostic structures encoded within those artifacts.

The testing and verification policy remains authoritative for verification strategy, while this specification defines the structured codes and evidence produced when those obligations fail.

Where another specification requires a diagnostic or structured failure but does not assign a code, this catalogue supplies the code and evidence contract. Where this catalogue appears to alter the underlying semantic condition, the domain specification remains authoritative and the catalogue must be revised.

---

# Part I — Unified problem model

## 3. One catalogue, distinct delivery forms

Every externally observable problem condition and every named internal verification failure belongs to this catalogue.

The catalogue covers:

```text
authoring
validation and compilation
machine lifecycle
runtime values, time, and policy
input construction and projection
bindings
inspection and explanation
reconfiguration and migration
persistence and restoration
replay
standard modules
observer integration
internal invariants and reference checks
```

This does not collapse every problem into one Rust return type.

The public model retains distinct forms:

```text
Report<T>                       collected findings, possibly with an artifact
Result<T, F>                    ordinary operation rejection
DiagnosticOccurrence           one committed runtime occurrence
DiagnosticEpisodeEvent         begin, change, resolve, or termination of an episode
ActiveDiagnosticEpisode        persistent current condition
InternalDefect                 library-defect record
```

Every public failure variant MUST expose one problem record or an exact lossless projection of one.

A caller MUST NOT parse `Display`, `Debug`, panic text, or rendered diagnostic prose to determine what happened.

## 4. Internal defects

Internal invariant violations use the same code, subject, evidence, ordering, persistence-safe rendering, and regression infrastructure as ordinary diagnostics.

They remain a distinct responsibility and delivery class.

An internal defect MUST NOT be emitted as an ordinary `RuntimeFailure` that implies the caller supplied invalid data.

An implementation MAY deliver an internal defect through:

- a dedicated `InternalDefect` result channel;
- an explicit verification API;
- a structured panic payload after constructing the defect record;
- a test-only or debug-only invariant-check result.

Whatever delivery mechanism is chosen, the complete structured defect MUST be available to tests and support tooling.

Assertions or panics without a stable code and structured evidence are insufficient for named invariants in this catalogue.

## 5. Codes name conditions

Code syntax is:

```text
<namespace>.<condition_name>
```

Each segment MUST match:

```text
[a-z][a-z0-9_]*
```

Examples:

```text
validation.current_reaction_cycle
runtime.stale_revision
reconfiguration.state_loss_rejected
persistence.wrong_time_domain
internal.pending_deadline_not_future
```

Codes MUST NOT be shaped as generic categories such as:

```text
runtime.error
validation.failure
internal.invariant_failed
```

A code does not identify the API method that detected the condition.

The same `validation.current_reaction_cycle` code is used when the invalid target is discovered during ordinary validation, compilation, or topology-patch preparation.

Different conditions require different codes even when they produce similar wording. For example:

```text
reconfiguration.unavoidable_semantic_loss
reconfiguration.state_loss_rejected
```

The first describes discovered loss and may accompany a usable prepared patch. The second describes rejection under the selected runtime policy.

## 6. Experimental and stabilized schemas

This specification defines an **experimental** diagnostic schema.

During active development, codes, evidence records, severity assignments, and catalogue organization MAY be renamed, split, merged, or removed through an intentional `DiagnosticSchemaVersion` change.

Such changes are ordinary coordinated refactoring before the schema is frozen. They do not require tombstones, deprecation periods, or permanent compatibility shims.

Tests and tooling MUST nevertheless use current structured codes rather than prose.

A future release may explicitly declare a diagnostic schema **stabilized**. After that declaration:

- a released code's semantic meaning becomes permanent;
- removed codes remain reserved;
- materially splitting or narrowing a condition introduces new codes;
- incompatible evidence changes require a new diagnostic schema version;
- wording may continue to change without a schema change.

No stability promise is inferred merely from numeric version ordering.

---

# Part II — Common structured representation

## 7. Problem record

The common conceptual representation is:

```rust
pub struct Problem<D> {
    pub code: DiagnosticCode,
    pub severity: Severity,
    pub responsibility: Responsibility,
    pub primary: SubjectRef,
    pub related: Vec<RelatedSubject>,
    pub evidence: ProblemEvidence<D>,
    pub suggestions: Vec<Suggestion>,
}
```

Exact private representation may differ. These fields and distinctions are normative.

`severity` and `responsibility` are fixed by the catalogue entry. An emitter MUST NOT select them dynamically.

`primary`, `related`, and `evidence` describe the actual occurrence.

`ProblemEvidence<D>` is a closed typed family. A code and evidence variant MUST form a valid pair by construction or be rejected during dynamic decoding.

The primary representation MUST NOT be an untyped map such as:

```text
HashMap<String, String>
```

A generic schema projection MAY be provided for editor or language-binding tooling.

## 8. Severity

The public severities are:

```rust
pub enum Severity {
    Info,
    Warning,
    Error,
}
```

### 8.1 `Info`

A semantically valid fact worth surfacing, usually a compatibility, normalization, or resynchronization notice.

### 8.2 `Warning`

The operation or artifact remains valid, but the condition is suspicious, degenerate, lossy, deprecated, or operationally concerning.

### 8.3 `Error`

The requested artifact or operation cannot be accepted, or an internal invariant has failed.

`Fatal` is not a severity. Process termination or operation rejection is delivery behavior.

`Bug` is not a severity. Library responsibility is represented separately.

## 9. Responsibility

Every code has one fixed responsibility classification:

```rust
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
```

### 9.1 `Advisory`

No party necessarily acted incorrectly. The record describes a valid but notable condition.

### 9.2 `CallerInput`

The supplied definition, value, query, patch, or operation violates a required rule.

Decoded but semantically malformed artifacts are caller-controlled input at the semantic validation boundary unless a more specific corruption code applies.

### 9.3 `SemanticRejection`

The request is structurally meaningful, but explicit node or policy semantics reject it.

### 9.4 `Compatibility`

Two otherwise meaningful artifacts, versions, identities, revisions, schemas, or policies cannot be used together.

### 9.5 `ResourceLimit`

An explicit runtime, decode, or verification budget was exceeded.

### 9.6 `CorruptData`

Persisted or retained data contradicts its integrity, schema, digest, or semantic consistency claims.

### 9.7 `UnsupportedFeature`

The request refers to a known category that the current implementation or version does not support.

### 9.8 `ExternalIntegration`

A failure occurred outside semantic machine execution, such as observer delivery.

### 9.9 `LibraryDefect`

A validated implementation state violates a `mossignal` invariant.

## 10. Delivery

Catalogue entries declare one or more allowed delivery forms:

```rust
pub enum ProblemDelivery {
    ReportFinding,
    OperationFailure,
    RuntimeOccurrence,
    PersistentEpisode,
    InternalDefect,
}
```

Delivery is not inferred from severity.

An `Error` in a validation report blocks the artifact. An `Error` returned as an operation failure rejects the operation. An internal-defect `Error` invokes the defect policy. A `Warning` occurrence may accompany a successful transaction.

The same code MAY permit both `ReportFinding` and `OperationFailure` when the same semantic condition can be detected eagerly by a builder or later by full validation.

## 11. Subjects

The subject model MUST identify every problem-bearing category required by the other specifications.

It therefore includes, directly or through typed subreferences:

```text
network
region at a specified revision
module definition
module instance
standard module descriptor
node
input port
output port
connection
external input
external output
binding or external binding key
runtime policy
input schema
transaction
network revision
resolved handle
inspection plan
pending event
provenance record or cause
active diagnostic episode
snapshot
persisted artifact
replay log, chunk, and frame
semantic version component
time domain
```

A subject reference MUST use stable semantic identity where one exists.

Dense indices, arena positions, allocation addresses, hash slots, and rendered paths MUST NOT be authoritative subject identity.

A derived subject such as a region MUST include the network fingerprint or revision context needed to prevent accidental cross-revision identity.

## 12. Related subjects

A related subject has a typed role:

```rust
pub struct RelatedSubject {
    pub role: RelatedSubjectRole,
    pub subject: SubjectRef,
}
```

Representative roles include:

```text
source
target
owner
driver
conflicting_driver
first_definition
duplicate_definition
missing_reference
expected_subject
actual_subject
base_subject
target_subject
migration_source
migration_target
cycle_predecessor
cycle_successor
supporter
blocker
invalidated_artifact
```

Related-subject order is semantic only where the evidence schema explicitly declares it ordered, such as a cycle witness or graph path.

## 13. Typed evidence

Every code defines exactly one evidence variant and its required fields.

Implementations SHOULD generate code-specific enum variants from the catalogue, for example:

```rust
ProblemEvidence::ValidationCurrentReactionCycle(
    CurrentReactionCycleEvidence { ... }
)
```

Reusable payload records MAY be shared internally, but the public code-to-evidence association remains exact.

The evidence-family names used in the catalogue tables, such as `TimeEvidence` or `MigrationEvidence`, identify reusable payload schemas. They do not permit arbitrary pairing between any code and any member of that family.

The generated Rust surface SHOULD therefore expose one code-specific variant per catalogue entry, even when several variants wrap the same reusable payload type. For example:

```rust
ProblemEvidence::RuntimeTimeOverflow(TimeEvidence)
ProblemEvidence::RuntimeInvalidTimeSubtraction(TimeEvidence)
```

A generic schema projection MAY collapse those variants to their shared family for tooling, but the canonical typed representation must retain the exact code-to-variant relationship.

Evidence MUST contain enough information to:

- understand the violated rule without parsing prose;
- identify all materially involved subjects;
- reproduce the condition where practical;
- compare candidate and reference behavior for internal defects;
- render useful human-readable messages;
- support deterministic regression assertions.

Evidence MUST NOT include private pointers or unstable dense positions as authoritative identity.

## 14. Evidence families

The catalogue uses the following recurring evidence shapes.

### 14.1 `ForeignArtifactEvidence`

```text
expected network or builder identity
encountered network or builder identity
foreign subject or artifact
requested operation
```

### 14.2 `KeyConflictEvidence`

```text
key category
stable key
first subject and origin
duplicate subject and origin
```

### 14.3 `MissingReferenceEvidence`

```text
owning subject
semantic role
missing stable reference
expected category and signal kind
```

### 14.4 `DirectionEvidence`

```text
connection or binding subject
source subject and declared direction
target subject and declared direction
required direction relation
```

### 14.5 `KindMismatchEvidence`

```text
owning operation or connection
expected kind
encountered kind
source and target subjects where applicable
```

### 14.6 `DriverConflictEvidence`

```text
target input port
complete conflicting driver set
driver policy
```

### 14.7 `ArityEvidence`

```text
node or module subject
semantic kind
required arity constraint
encountered arity
relevant stable port keys
```

### 14.8 `ParameterEvidence`

```text
owning subject
parameter key
expected parameter kind or valid domain
encountered value
```

### 14.9 `StateSchemaEvidence`

```text
state owner
expected state schema
encountered state schema
incompatible fields or roles
```

### 14.10 `ModuleSchemaEvidence`

```text
module or instance
expected public interface or binding schema
encountered schema
missing, unexpected, or incompatible ports and bindings
```

### 14.11 `HierarchyEvidence`

```text
involved modules or nodes
parent assignments
ordered cycle or malformed containment witness
```

### 14.12 `CurrentReactionCycleEvidence`

```text
strongly connected component members
one ordered cycle witness
for every step: source fact, node dependency, target fact
apparent state or temporal boundaries that do not break the dependency
```

### 14.13 `DependencySignatureEvidence`

```text
node kind and subject
declared dependency relation
derived or required relation
missing, extra, or malformed dependency edges
```

### 14.14 `StaticQualityEvidence`

```text
primary structural subject
relevant connected subjects
node kind, arity, constant law, or reachability facts
```

### 14.15 `LifecycleEvidence`

```text
machine lifecycle state
requested operation
required lifecycle state
logical time where established
input artifact category where applicable
```

### 14.16 `RevisionMismatchEvidence`

```text
network identity
expected revision
actual revision
artifact or operation bound to the expected revision
stable subject to resolve again where applicable
```

### 14.17 `DigestMismatchEvidence`

```text
digest kind
expected digest
actual digest
artifact, frame, or machine context
```

### 14.18 `DigestCollisionEvidence`

```text
digest kind and domain
shared digest value
first canonical record bytes or content identifier
second conflicting canonical record bytes or content identifier
artifact or in-memory context
```

### 14.19 `TimeEvidence`

```text
current time where established
requested time or operands
required relation
checked operation
```

### 14.20 `InputObservationEvidence`

```text
input schema identity
endpoint or external observation key
expected signal kind
encountered observations and values
required establishment status
```

### 14.21 `InputSchemaEvidence`

```text
expected schema identity
actual schema identity
network fingerprint and revision
missing, unexpected, new, preserved, and removed endpoints
```

### 14.22 `BindingEvidence`

```text
network and binding-set identity
endpoint
external binding key
conflicting or missing mappings
expected signal kind
```

### 14.23 `StaleArtifactEvidence`

```text
artifact category
bound fingerprint and revision
actual fingerprint and revision
stable subjects referenced by the artifact
```

### 14.24 `ConflictEvidence`

```text
node subject
conflict policy
previous stored state
set and reset presence or settled values
pulse counts where applicable
logical time and revision
```

### 14.25 `BudgetEvidence`

```text
policy identity
budget name
configured limit
consumed or required amount
phase and logical time
```

### 14.26 `PatchEditEvidence`

```text
base network identity, fingerprint, and revision
normalized operations involved
primary edited subject
conflicting assignments or invalid correspondence
```

### 14.27 `MigrationEvidence`

```text
base and target subjects
base and target definitions or schemas
migration directive and resolved rule
actual state or pending event where finalization-dependent
rejection reason
```

### 14.28 `SemanticLossEvidence`

```text
potential or actual loss identity
loss category
source subject and state or pending-event identity
patch operation and migration rule
runtime predicate where conditional
```

### 14.29 `CanonicalEncodingEvidence`

```text
artifact kind where known
byte offset or path where available
canonical violation kind
encountered token, field, length, or ordering fact
configured decode limit where relevant
```

Canonical violation kinds include at least:

```text
non_shortest_integer
non_shortest_length
indefinite_length
forbidden_map
forbidden_tag
forbidden_float
forbidden_simple_value
invalid_utf8
duplicate_field
unsorted_field
unsorted_set_or_map
wrong_fixed_length
length_overflow
excessive_nesting
trailing_bytes
```

### 14.30 `VersionCompatibilityEvidence`

```text
artifact kind
compatibility stage
version component
encountered version
supported or required versions
whether an explicit representation upgrader exists
```

### 14.31 `ArtifactIdentityEvidence`

```text
artifact kind
expected and actual time domain, network key, fingerprint, revision, or policy identity
compatibility stage
```

### 14.32 `PendingEventEvidence`

```text
pending event identity
owner
originating time
deadline
payload or multiplicity
expected invariant or migration outcome
```

### 14.33 `DiagnosticEpisodeEvidence`

```text
episode identity
code
owner
condition discriminator
began time
last material change
evidence before and after where applicable
```

### 14.34 `ProvenanceEvidence`

```text
cause digest or private cause reference in its owning artifact
record kind and subject
predecessor digest and role where applicable
required root
checkpoint boundary
conflicting canonical bytes where applicable
```

### 14.35 `ReplayEvidence`

```text
log, chunk, and frame identity
frame index and logical time where available
expected and actual revision, fingerprint, policy, and digests
underlying operation code where applicable
```

### 14.36 `StandardModuleEvidence`

```text
standard module reference
module definition or instance
parameter key and value
arity and public port keys
expected and encountered interface schemas
expected and encountered expansion fingerprints
base and target versions
```

### 14.37 `ObserverEvidence`

```text
observer or delivery target identity
cursor or change-set boundary
committed revision and digests
external delivery error category
resynchronization boundary where applicable
```

### 14.38 `InternalInvariantEvidence`

```text
invariant name
semantic boundary where checked
network fingerprint, revision, logical time, and policy where applicable
involved stable subjects
candidate observation
reference or recomputed observation
reproduction seed, generator version, feature configuration, and minimized artifact where available
```

## 15. Suggestions

Suggestions are closed, machine-readable actions.

A code MAY permit suggestions only where the correction is unambiguous from the evidence.

Representative suggestion kinds include:

```text
resolve_stable_key_again
rebuild_input_for_current_schema
supply_missing_level
use_establish_for_new_level
prepare_patch_again
remove_duplicate_connection
connect_missing_required_input
increase_named_budget
run_available_schema_upgrade
use_supported_standard_module_version
```

The library MUST NOT suggest one arbitrary graph edit, migration policy, state reset, or compatibility override when several materially different corrections are possible.

Rendered advice may describe alternatives without encoding one as an authoritative suggestion.

---

# Part III — Reports, failures, ordering, and identity

## 16. Reports

Validation, compilation, binding, and structural patch preparation use:

```rust
pub struct Report<T> {
    pub artifact: Option<T>,
    pub diagnostics: DiagnosticSet,
}
```

Independent findings SHOULD be collected where safe.

An artifact MUST be absent when any blocking `Error` finding remains.

A `Warning` or `Info` finding MAY accompany an artifact.

One malformed condition SHOULD NOT suppress unrelated findings unless continuing would be unsafe or would create misleading cascades.

`ReportFailure` retains the complete diagnostic set and introduces no additional semantic problem code merely because `require_artifact` was called.

## 17. Failure enums

Operation-specific Rust failure enums remain useful and SHOULD retain ergonomic category matching.

Each leaf failure variant MUST map one-to-one to a catalogue code and typed evidence variant.

Wrapper variants such as:

```text
RuntimeFailure::Input(...)
RuntimeFailure::Reconfiguration(...)
ReplayFailure::Underlying(...)
```

are grouping constructs and do not require additional wrapper codes when the underlying problem record already identifies the failure completely.

A wrapper MAY add non-semantic location context such as replay frame index. If that context materially changes the condition, the catalogue provides a replay-specific code.

## 18. Deterministic ordering

`DiagnosticSet` iteration order is canonical:

1. severity rank: `Error`, `Warning`, `Info`;
2. code by ascending ASCII bytes;
3. primary subject by canonical subject order;
4. code-defined condition discriminator;
5. canonical encoded evidence bytes.

Presentation layers MAY regroup findings, but canonical serialization, test comparison, and default iteration use this order.

Diagnostic ordering MUST NOT depend on:

- hash iteration;
- graph traversal order;
- definition insertion order;
- patch operation insertion order;
- evaluator worklist order;
- which independent validator happened to detect the condition first.

## 19. Condition identity and deduplication

Every catalogue entry defines a condition discriminator.

The canonical condition key is:

```text
code
+ primary subject
+ condition discriminator
```

One `DiagnosticSet` contains at most one finding per condition key.

Repeated detection of the same condition merges evidence according to the code-defined merge law.

Set-like evidence is unioned and canonically ordered. Ordered witnesses retain semantic order. Equivalent scalar evidence must agree.

Contradictory scalar evidence for one condition key is `internal.diagnostic_evidence_conflict`.

Different stable subjects, connections, pending events, state cells, or semantic-loss identities MUST NOT be deduplicated merely because their rendered wording is equal.

## 20. Persistent episodes

A code is episode-capable only when its catalogue entry permits `PersistentEpisode` delivery.

The initial catalogue has one core episode-producing condition:

```text
runtime.level_latch_conflict_retained
```

An active episode contains:

```text
DiagnosticEpisodeId
code
primary subject
condition discriminator
began_at
current evidence
last_material_change
```

Episode events are:

```rust
pub enum DiagnosticEpisodeChangeKind {
    Began,
    Changed,
    Resolved,
    Terminated,
}
```

The problem code remains the condition code. Begin, change, resolution, and termination are event kinds, not separate codes.

An unchanged active condition emits no repeated event on unrelated transactions.

A rejected transaction commits no episode creation, evidence change, resolution, or termination.

## 21. Episode identity and migration

A fresh episode identity is derived from:

```text
network identity
condition code
primary structural subject
condition discriminator
```

The resulting opaque identifier is persisted rather than recomputed from private runtime positions.

Across compatible preservation, the identifier remains unchanged.

Across explicit subject reassociation, a migration rule MAY preserve or transform the episode identifier only when the condition retains the same semantic identity and the migration report records the old and new subjects.

Removal or incompatible semantic change resolves or terminates the episode.

Dense-slot reuse MUST NOT transfer an episode.

## 22. Runtime occurrences

Transient runtime diagnostics are committed occurrences attached to one reaction or outer transaction.

The initial runtime occurrence code is:

```text
runtime.pulse_latch_conflict_retained
```

Separate later conflicts are separate occurrences because pulse assertions do not remain active across reactions.

## 23. Context and reproducibility

Problem records MAY carry a standard non-semantic context envelope containing:

```text
library version
DiagnosticSchemaVersion
feature configuration
platform
transaction metadata
random seed and generator version
test or fuzz reproducer location
```

This envelope does not participate in condition identity unless a catalogue entry explicitly makes one field semantic.

Internal reference divergence and randomized verification failures SHOULD include the minimized canonical reproducer or a durable reference to it.

---

# Part IV — Namespace rules

## 24. Initial namespaces

The initial namespace set is:

```text
authoring
validation
lifecycle
runtime
input
binding
inspection
explanation
reconfiguration
persistence
replay
standard_module
observer
internal
```

A namespace identifies the semantic domain of the condition, not necessarily the implementation layer that detected it.

Compilation normally emits `validation.*` for invalid definitions and `internal.*` for violated compiled invariants. No separate public `compilation.*` condition is required in catalogue version 1.

Patch preparation uses ordinary `validation.*` codes for invalid target-graph conditions and `reconfiguration.*` codes for patch, correspondence, migration, and loss conditions.

Decoded semantic definitions use ordinary `validation.*` codes after successful persistence decoding. Persistence-specific codes describe encoding, integrity, compatibility, and restoration-state failures.

---

# Part V — Exhaustive initial catalogue

The following tables are normative.

**Delivery abbreviations:**

```text
Report      ReportFinding
Failure     OperationFailure
Occurrence  RuntimeOccurrence
Episode     PersistentEpisode
Defect      InternalDefect
```

## 25. Authoring

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `authoring.foreign_signal` | Error | CallerInput | Failure | `ForeignArtifactEvidence` | A builder-scoped signal from another builder was supplied. |
| `authoring.foreign_network_artifact` | Error | CallerInput | Failure | `ForeignArtifactEvidence` | A typed artifact belongs to another network or authoring scope. |

Duplicate stable keys, missing subjects, signal-kind mismatch, and other semantic definition defects use the applicable `validation.*` codes even when an authoring API detects them immediately.

## 26. Validation — blocking conditions

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `validation.duplicate_key` | Error | CallerInput | Report, Failure | `KeyConflictEvidence` | Two subjects claim one stable key in a scope requiring uniqueness. |
| `validation.missing_node` | Error | CallerInput | Report | `MissingReferenceEvidence` | A definition references a node that does not exist. |
| `validation.missing_port` | Error | CallerInput | Report | `MissingReferenceEvidence` | A definition references a port that does not exist. |
| `validation.missing_endpoint` | Error | CallerInput | Report | `MissingReferenceEvidence` | A definition references an external or module endpoint that does not exist. |
| `validation.invalid_direction` | Error | CallerInput | Report, Failure | `DirectionEvidence` | A connection or binding violates input/output direction. |
| `validation.signal_kind_mismatch` | Error | CallerInput | Report, Failure | `KindMismatchEvidence` | Connected, bound, or reassociated subjects have incompatible signal kinds. |
| `validation.unsupported_multiple_drivers` | Error | CallerInput | Report | `DriverConflictEvidence` | An input has more drivers than its declared policy permits. |
| `validation.missing_required_input` | Error | CallerInput | Report | `MissingReferenceEvidence` | A required fixed input or module binding is absent. |
| `validation.invalid_fixed_arity` | Error | CallerInput | Report | `ArityEvidence` | A fixed-shape node or interface has the wrong number of ports. |
| `validation.invalid_variadic_arity` | Error | CallerInput | Report | `ArityEvidence` | A variadic node violates its semantic arity domain, including zero-input `Zip`. |
| `validation.invalid_parameter` | Error | CallerInput | Report, Failure | `ParameterEvidence` | A semantic parameter violates its declared domain. |
| `validation.invalid_timing_parameter` | Error | CallerInput | Report, Failure | `ParameterEvidence` | A temporal parameter is zero, overflowing, or otherwise invalid for its node law. |
| `validation.invalid_initial_state` | Error | CallerInput | Report | `ParameterEvidence` | A declared initial state is malformed or incompatible with the node definition. |
| `validation.incompatible_state_schema` | Error | CallerInput | Report | `StateSchemaEvidence` | A state schema does not match its declared owner or semantic kind. |
| `validation.invalid_module_interface` | Error | CallerInput | Report | `ModuleSchemaEvidence` | A module definition or instance fails public interface conformance. |
| `validation.invalid_module_binding` | Error | CallerInput | Report | `ModuleSchemaEvidence` | Module input or output bindings are missing, duplicated, or incompatible. |
| `validation.malformed_hierarchy` | Error | CallerInput | Report | `HierarchyEvidence` | Module containment or parent references are structurally malformed. |
| `validation.hierarchy_cycle` | Error | CallerInput | Report | `HierarchyEvidence` | Module containment contains a directed cycle. |
| `validation.current_reaction_cycle` | Error | CallerInput | Report | `CurrentReactionCycleEvidence` | The current-reaction dependency graph contains a cycle. |
| `validation.invalid_dependency_signature` | Error | CallerInput | Report | `DependencySignatureEvidence` | A node dependency declaration contains impossible or malformed edges. |
| `validation.incomplete_dependency_signature` | Error | CallerInput | Report | `DependencySignatureEvidence` | A node dependency declaration omits influence required by its semantic law. |
| `validation.incompatible_network_reference` | Error | CallerInput | Report, Failure | `ForeignArtifactEvidence` | A definition contains a stable reference bound to another network identity. |
| `validation.unsupported_node_kind` | Error | UnsupportedFeature | Report | `ParameterEvidence` | The definition names a node kind unavailable in the current semantic version. |

## 27. Validation — non-blocking conditions

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `validation.duplicate_source` | Warning | CallerInput | Report | `StaticQualityEvidence` | One upstream source feeds several ports of the same node where multiplicity remains port-based. |
| `validation.empty_variadic_node` | Warning | Advisory | Report | `ArityEvidence` | A total variadic primitive has zero inputs and therefore uses its empty law. |
| `validation.unary_degenerate_node` | Warning | Advisory | Report | `ArityEvidence` | A variadic primitive has one input and reduces to an identity-like or redundant law. |
| `validation.constant_result_node` | Warning | Advisory | Report | `StaticQualityEvidence` | Node parameters and arity make the result constant. |
| `validation.unreachable_output` | Warning | CallerInput | Report | `StaticQualityEvidence` | An external or module output has no meaningful reachable source path. |
| `validation.unused_input` | Warning | CallerInput | Report | `StaticQualityEvidence` | An input or public module input has no semantic incidence, except where a standard descriptor declares this intentional. |
| `validation.isolated_node` | Warning | CallerInput | Report | `StaticQualityEvidence` | A node is disconnected from all semantically relevant boundaries. |
| `validation.redundant_connection` | Warning | CallerInput | Report | `StaticQualityEvidence` | A connection is semantically redundant under the declared graph and node law. |
| `validation.empty_module` | Warning | Advisory | Report | `StaticQualityEvidence` | A module contains no semantic internal structure or public effect. |
| `validation.deprecated_node_form` | Warning | Compatibility | Report | `StaticQualityEvidence` | A supported authored form is deprecated in favor of another current form. |

A more specific warning suppresses `validation.constant_result_node` when both would describe the same cause.

## 28. Lifecycle

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `lifecycle.not_initialized` | Error | CallerInput | Failure | `LifecycleEvidence` | Runtime state, schedule, pending work, or explanation was requested before initialization. |
| `lifecycle.already_initialized` | Error | CallerInput | Failure | `LifecycleEvidence` | An initialization-only operation was requested on a ready machine. |
| `lifecycle.snapshot_required` | Error | CallerInput | Failure | `LifecycleEvidence` | The first transaction did not provide a complete `InputSnapshot`. |
| `lifecycle.delta_before_initialization` | Error | CallerInput | Failure | `LifecycleEvidence` | An `InputDelta` was supplied before an authoritative external valuation existed. |
| `lifecycle.delta_required_after_initialization` | Error | CallerInput | Failure | `LifecycleEvidence` | A ready-machine transaction used an initialization snapshot where the API requires a delta. |

## 29. Runtime values, time, policy, and semantic rejection

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `runtime.stale_revision` | Error | Compatibility | Failure | `RevisionMismatchEvidence` | An operation expected a different topology revision from the machine's current revision. |
| `runtime.stale_execution_state` | Error | Compatibility | Failure | `DigestMismatchEvidence` | An expected execution-state digest does not match the current machine. |
| `runtime.time_not_strictly_increasing` | Error | CallerInput | Failure | `TimeEvidence` | A ready-machine transaction time is equal to or earlier than current logical time. |
| `runtime.time_overflow` | Error | SemanticRejection | Failure | `TimeEvidence` | Checked logical-time arithmetic overflowed. |
| `runtime.invalid_time_subtraction` | Error | CallerInput | Failure | `TimeEvidence` | A duration was requested from a later time to an earlier time. |
| `runtime.zero_span_not_allowed` | Error | CallerInput | Failure | `ParameterEvidence` | A positive duration type or semantic operation received zero. |
| `runtime.pulse_count_overflow` | Error | SemanticRejection | Failure | `ParameterEvidence` | Checked pulse multiplicity arithmetic exceeded the representable range. |
| `runtime.policy_missing_limit` | Error | CallerInput | Failure | `ParameterEvidence` | A semantically relevant runtime-policy field was not set. |
| `runtime.policy_invalid_limit` | Error | CallerInput | Failure | `ParameterEvidence` | A runtime-policy limit violates its declared domain. |
| `runtime.budget_exceeded` | Error | ResourceLimit | Failure | `BudgetEvidence` | A named runtime or migration budget was exceeded. |
| `runtime.pulse_latch_conflict_retained` | Warning | Advisory | Occurrence | `ConflictEvidence` | A pulse set/reset conflict was retained and diagnosed for one reaction. |
| `runtime.pulse_latch_conflict_rejected` | Error | SemanticRejection | Failure | `ConflictEvidence` | A pulse set/reset conflict rejected the transaction under `RejectTransaction`. |
| `runtime.level_latch_conflict_retained` | Warning | Advisory | Episode | `ConflictEvidence` | A continuous level set/reset conflict is retained under `RetainAndDiagnose`. |
| `runtime.level_latch_conflict_rejected` | Error | SemanticRejection | Failure | `ConflictEvidence` | A level set/reset conflict rejected the transaction under `RejectTransaction`. |

## 30. Input construction and projection

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `input.unknown_endpoint` | Error | CallerInput | Failure | `InputObservationEvidence` | An input artifact references an endpoint absent from its schema. |
| `input.wrong_signal_kind` | Error | CallerInput | Failure | `InputObservationEvidence` | An observation or pulse count is supplied for the wrong signal kind. |
| `input.duplicate_observation` | Error | CallerInput | Failure | `InputObservationEvidence` | The same endpoint is supplied more than once with equivalent observations. |
| `input.conflicting_observation` | Error | CallerInput | Failure | `InputObservationEvidence` | The same endpoint is supplied incompatible observations in one batch. |
| `input.ambiguous_observation` | Error | CallerInput | Failure | `InputObservationEvidence` | An external observation resolves to more than one endpoint. |
| `input.missing_required_level` | Error | CallerInput | Failure | `InputSchemaEvidence` | A complete snapshot or target schema lacks a required level value. |
| `input.wrong_network` | Error | Compatibility | Failure | `InputSchemaEvidence` | The input artifact belongs to another network identity. |
| `input.foreign_schema` | Error | Compatibility | Failure | `InputSchemaEvidence` | The input artifact was built for a different schema family or target topology. |
| `input.stale_schema` | Error | Compatibility | Failure | `InputSchemaEvidence` | The input artifact's fingerprint or revision is stale. |
| `input.removed_endpoint` | Error | CallerInput | Failure | `InputObservationEvidence` | A target-bound patch input references an endpoint removed by the patch. |
| `input.new_level_requires_establish` | Error | CallerInput | Failure | `InputObservationEvidence` | A newly introduced external level was set as preserved instead of explicitly established. |
| `input.establish_not_permitted` | Error | CallerInput | Failure | `InputObservationEvidence` | `establish` was used for an input that is not newly introduced. |
| `input.target_schema_mismatch` | Error | Compatibility | Failure | `InputSchemaEvidence` | Transaction input does not match the prepared patch's exact target schema. |

## 31. Bindings

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `binding.unknown_endpoint` | Error | CallerInput | Report, Failure | `BindingEvidence` | A binding references an endpoint absent from the compiled network. |
| `binding.wrong_signal_kind` | Error | CallerInput | Report, Failure | `BindingEvidence` | A binding associates an endpoint with an incompatible typed binding path. |
| `binding.duplicate_endpoint` | Error | CallerInput | Report, Failure | `BindingEvidence` | One endpoint is bound more than once where one binding is required. |
| `binding.duplicate_external_key` | Error | CallerInput | Report, Failure | `BindingEvidence` | One caller-owned external key maps ambiguously to several endpoints. |
| `binding.ambiguous_external_key` | Error | CallerInput | Report, Failure | `BindingEvidence` | Reverse lookup cannot identify one endpoint uniquely. |
| `binding.missing_required_binding` | Error | CallerInput | Report, Failure | `BindingEvidence` | A binding set required by the requested projection is incomplete. |
| `binding.wrong_network` | Error | Compatibility | Failure | `BindingEvidence` | A binding set is used with another network identity. |
| `binding.stale_schema` | Error | Compatibility | Failure | `BindingEvidence` | A binding projector or binding set is stale for the current topology. |

## 32. Inspection

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `inspection.unknown_subject` | Error | CallerInput | Failure | `MissingReferenceEvidence` | The requested stable subject does not exist in the inspected artifact. |
| `inspection.wrong_subject_kind` | Error | CallerInput | Failure | `KindMismatchEvidence` | The requested projection does not apply to the subject kind. |
| `inspection.foreign_handle` | Error | Compatibility | Failure | `StaleArtifactEvidence` | A resolved handle belongs to another network or machine context. |
| `inspection.stale_handle` | Error | Compatibility | Failure | `StaleArtifactEvidence` | A resolved handle is bound to an earlier topology revision. |
| `inspection.foreign_plan` | Error | Compatibility | Failure | `StaleArtifactEvidence` | An inspection plan belongs to another network or machine. |
| `inspection.stale_plan` | Error | Compatibility | Failure | `StaleArtifactEvidence` | An inspection plan is bound to an earlier fingerprint or revision. |
| `inspection.pending_event_not_found` | Error | CallerInput | Failure | `PendingEventEvidence` | The requested pending event does not exist in the inspected state. |
| `inspection.unsupported_projection` | Error | UnsupportedFeature | Failure | `ParameterEvidence` | The requested inspection projection is not supported for this subject or retained state. |

Requests requiring ready runtime state use `lifecycle.not_initialized` rather than an inspection-specific duplicate code.

## 33. Explanation and provenance access

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `explanation.unknown_subject` | Error | CallerInput | Failure | `MissingReferenceEvidence` | The requested explainable subject does not exist. |
| `explanation.foreign_cause` | Error | Compatibility | Failure | `ProvenanceEvidence` | A cause reference is used outside the artifact or view that owns it. |
| `explanation.invalid_cause` | Error | CallerInput | Failure | `ProvenanceEvidence` | A cause reference is malformed or does not resolve in its owning view. |
| `explanation.cause_not_retained` | Error | Compatibility | Failure | `ProvenanceEvidence` | Requested ancestry lies beyond the authoritative retention boundary. |
| `explanation.request_outside_retention` | Error | Compatibility | Failure | `ProvenanceEvidence` | A historical non-occurrence or complete-ancestry claim exceeds retained history. |
| `explanation.history_truncated` | Info | Advisory | Report | `ProvenanceEvidence` | A valid explanation is bounded by an explicit checkpoint or retention boundary. |
| `explanation.unsupported_request` | Error | UnsupportedFeature | Failure | `ParameterEvidence` | The requested why or why-not form is not defined for the subject. |

## 34. Reconfiguration — patch construction and preparation

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `reconfiguration.foreign_artifact` | Error | CallerInput | Failure | `ForeignArtifactEvidence` | A patch operation contains an artifact from another network or time domain. |
| `reconfiguration.duplicate_operation` | Error | CallerInput | Failure | `PatchEditEvidence` | Equivalent or conflicting duplicate operations target one property or subject. |
| `reconfiguration.conflicting_edit` | Error | CallerInput | Failure | `PatchEditEvidence` | The operation set assigns incompatible edits to one subject. |
| `reconfiguration.invalid_replacement_key` | Error | CallerInput | Failure | `PatchEditEvidence` | A replacement definition does not retain the operation subject's required key. |
| `reconfiguration.invalid_reassociation` | Error | CallerInput | Failure, Report | `PatchEditEvidence` | A reassociation has incompatible categories, directions, kinds, chains, or cycles. |
| `reconfiguration.contradictory_hierarchy` | Error | CallerInput | Failure | `PatchEditEvidence` | The patch assigns incompatible parents to one subject. |
| `reconfiguration.base_fingerprint_mismatch` | Error | Compatibility | Report, Failure | `ArtifactIdentityEvidence` | The patch base fingerprint differs from the topology used for preparation. |
| `reconfiguration.base_revision_mismatch` | Error | Compatibility | Report, Failure | `RevisionMismatchEvidence` | A machine convenience was asked to prepare a patch for another revision. |
| `reconfiguration.unknown_base_subject` | Error | CallerInput | Report | `MissingReferenceEvidence` | A patch operation references a subject absent from the base graph. |
| `reconfiguration.non_injective_reassociation` | Error | CallerInput | Report | `PatchEditEvidence` | More than one source maps to one target, or one source maps to several targets. |
| `reconfiguration.incompatible_migration_directive` | Error | CallerInput | Report | `MigrationEvidence` | A migration directive does not apply to the source and target node families. |
| `reconfiguration.incomplete_temporal_migration_policy` | Error | CallerInput | Report | `MigrationEvidence` | A temporal change lacks a total rule for every possible pending obligation. |
| `reconfiguration.unsupported_cross_kind_migration` | Error | UnsupportedFeature | Report | `MigrationEvidence` | No closed migration law exists between the selected semantic kinds. |
| `reconfiguration.ambiguous_event_migration` | Error | CallerInput | Report, Failure | `PendingEventEvidence` | Pending work cannot be transformed with deterministic ownership, origin, deadline, or precedence. |
| `reconfiguration.conflicting_migrated_transitions` | Error | SemanticRejection | Failure | `PendingEventEvidence` | Migrated transitions have indistinguishable origins and no explicit resolution law. |
| `reconfiguration.invalid_target_input_schema` | Error | CallerInput | Report | `InputSchemaEvidence` | The target topology does not yield a coherent explicit input schema. |
| `reconfiguration.conditional_semantic_loss` | Warning | Advisory | Report | `SemanticLossEvidence` | The patch may lose semantic state depending on runtime state at finalization. |
| `reconfiguration.unavoidable_semantic_loss` | Warning | Advisory | Report | `SemanticLossEvidence` | The patch necessarily loses one or more semantic facts if committed. |
| `reconfiguration.empty_patch` | Error | CallerInput | Report | `PatchEditEvidence` | The normalized patch has no effective structural, semantic, hierarchical, endpoint, or metadata change. |

Invalid target graph structure additionally uses the ordinary applicable `validation.*` codes.

## 35. Reconfiguration — finalization

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `reconfiguration.stale_prepared_patch` | Error | Compatibility | Failure | `StaleArtifactEvidence` | A prepared patch no longer matches the machine's exact base revision or fingerprint. |
| `reconfiguration.target_input_schema_mismatch` | Error | Compatibility | Failure | `InputSchemaEvidence` | Patch-bearing transaction input is not bound to the prepared target schema. |
| `reconfiguration.state_migration_rejected` | Error | SemanticRejection | Failure | `MigrationEvidence` | Final state-dependent migration rejected for one state owner. |
| `reconfiguration.pending_event_migration_rejected` | Error | SemanticRejection | Failure | `MigrationEvidence` | Finalization rejected one pending event or temporal schedule. |
| `reconfiguration.require_preserve_failed` | Error | SemanticRejection | Failure | `MigrationEvidence` | `RequirePreserve` could not produce a lossless preserve outcome. |
| `reconfiguration.episode_migration_rejected` | Error | SemanticRejection | Failure | `DiagnosticEpisodeEvidence` | An active episode could not be preserved, transformed, resolved, or terminated under the selected rules. |
| `reconfiguration.provenance_migration_rejected` | Error | SemanticRejection | Failure | `ProvenanceEvidence` | Required causal ancestry could not be migrated or checkpointed without violating the specification. |
| `reconfiguration.state_loss_rejected` | Error | SemanticRejection | Failure | `SemanticLossEvidence` | A nonempty finalized loss set was forbidden by `RejectStateLoss`. |

Checked time and budget failures during finalization use `runtime.time_overflow` and `runtime.budget_exceeded` with reconfiguration phase evidence.

## 36. Persistence — canonical decoding and envelopes

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `persistence.invalid_prefix` | Error | CorruptData | Failure | `CanonicalEncodingEvidence` | Standalone bytes do not begin with the required artifact prefix. |
| `persistence.truncated_artifact` | Error | CorruptData | Failure | `CanonicalEncodingEvidence` | The artifact ends before its declared canonical structure is complete. |
| `persistence.trailing_bytes` | Error | CorruptData | Failure | `CanonicalEncodingEvidence` | Bytes remain after one complete standalone artifact. |
| `persistence.noncanonical_encoding` | Error | CorruptData | Failure | `CanonicalEncodingEvidence` | The bytes violate the strict canonical value profile. |
| `persistence.malformed_envelope` | Error | CorruptData | Failure | `CanonicalEncodingEvidence` | The top-level artifact envelope is syntactically or structurally malformed. |
| `persistence.unknown_artifact_kind` | Error | UnsupportedFeature | Failure | `VersionCompatibilityEvidence` | The artifact kind is not recognized by the current persistence schema. |
| `persistence.unknown_schema_field` | Error | Compatibility | Failure | `VersionCompatibilityEvidence` | A known schema version contains an undeclared non-extension field. |
| `persistence.unknown_schema_variant` | Error | Compatibility | Failure | `VersionCompatibilityEvidence` | A known schema version contains an undeclared variant. |
| `persistence.integrity_digest_mismatch` | Error | CorruptData | Failure | `DigestMismatchEvidence` | The envelope integrity digest does not match canonical content. |
| `persistence.decode_limit_exceeded` | Error | ResourceLimit | Failure | `BudgetEvidence` | A named persistence decoding limit was exceeded. |
| `persistence.unsupported_version` | Error | Compatibility | Failure | `VersionCompatibilityEvidence` | One version-vector component is unsupported at the required compatibility stage. |
| `persistence.wrong_time_domain` | Error | Compatibility | Failure | `ArtifactIdentityEvidence` | Persisted `TimeDomainId` differs from the supplied persistence context. |
| `persistence.semantic_migration_required` | Error | Compatibility | Failure | `VersionCompatibilityEvidence` | Acceptance would require behavior-changing migration rather than representation-only upgrade. |

## 37. Persistence — restoration identity and semantic consistency

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `persistence.network_identity_mismatch` | Error | Compatibility | Failure | `ArtifactIdentityEvidence` | The artifact and supplied compiled network have different network identities. |
| `persistence.fingerprint_mismatch` | Error | Compatibility | Failure | `ArtifactIdentityEvidence` | The semantic network fingerprint does not match. |
| `persistence.topology_revision_mismatch` | Error | Compatibility | Failure | `ArtifactIdentityEvidence` | The required topology revision context is incompatible. |
| `persistence.runtime_policy_mismatch` | Error | Compatibility | Failure | `ArtifactIdentityEvidence` | Snapshot or replay policy identity differs from the supplied runtime policy. |
| `persistence.validation_claim_mismatch` | Error | CorruptData | Failure | `DigestMismatchEvidence` | Revalidation does not reproduce the persisted validation claim or fingerprint. |
| `persistence.lifecycle_shape_invalid` | Error | CorruptData | Failure | `ParameterEvidence` | Persisted fields contradict the declared uninitialized or ready lifecycle shape. |
| `persistence.state_schema_mismatch` | Error | Compatibility | Failure | `StateSchemaEvidence` | Persisted state does not match the compiled semantic state schema. |
| `persistence.unknown_subject` | Error | CorruptData | Failure | `MissingReferenceEvidence` | Persisted runtime state references no compatible structural subject. |
| `persistence.pending_event_invalid` | Error | CorruptData | Failure | `PendingEventEvidence` | A pending event has invalid owner, payload, deadline, or temporal schema. |
| `persistence.event_identity_state_invalid` | Error | CorruptData | Failure | `PendingEventEvidence` | Pending-event keys, serial allocation state, or non-reuse invariants are inconsistent. |
| `persistence.diagnostic_episode_invalid` | Error | CorruptData | Failure | `DiagnosticEpisodeEvidence` | Persisted episode identity, owner, discriminator, or lifecycle is invalid. |
| `persistence.diagnostic_schema_invalid` | Error | CorruptData | Failure | `DiagnosticEpisodeEvidence` | A persisted diagnostic code, severity, responsibility, or evidence variant disagrees with the declared diagnostic schema. |
| `persistence.settled_state_inconsistent` | Error | CorruptData | Failure | `InternalInvariantEvidence` | Persisted settled values disagree with topology, external levels, stored state, and temporal obligations. |
| `persistence.execution_digest_mismatch` | Error | CorruptData | Failure | `DigestMismatchEvidence` | Recomputed execution-state digest differs from the persisted claim. |
| `persistence.observable_digest_mismatch` | Error | CorruptData | Failure | `DigestMismatchEvidence` | Recomputed observable-state digest differs from the persisted claim. |
| `persistence.snapshot_digest_mismatch` | Error | CorruptData | Failure | `DigestMismatchEvidence` | Recomputed snapshot digest differs from the persisted claim. |
| `persistence.digest_collision` | Error | CorruptData | Failure, Defect | `DigestCollisionEvidence` | Equal typed digests identify different canonical records. |

## 38. Persistence — provenance validation

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `persistence.provenance_missing_predecessor` | Error | CorruptData | Failure | `ProvenanceEvidence` | A provenance record references an unavailable predecessor. |
| `persistence.provenance_digest_mismatch` | Error | CorruptData | Failure | `ProvenanceEvidence` | A record's canonical bytes do not reproduce its `CauseDigest`. |
| `persistence.provenance_cycle` | Error | CorruptData | Failure | `ProvenanceEvidence` | Persisted provenance is not acyclic. |
| `persistence.provenance_invalid_subject` | Error | CorruptData | Failure | `ProvenanceEvidence` | A provenance record names an invalid or incompatible semantic subject. |
| `persistence.provenance_invalid_role` | Error | CorruptData | Failure | `ProvenanceEvidence` | A predecessor role is not valid for the record kind. |
| `persistence.provenance_incomplete_root_closure` | Error | CorruptData | Failure | `ProvenanceEvidence` | Required current roots do not reach complete authoritative ancestry or checkpoints. |
| `persistence.provenance_conflicting_record` | Error | CorruptData | Failure | `ProvenanceEvidence` | One cause digest is associated with conflicting canonical record content. |
| `persistence.provenance_false_checkpoint` | Error | CorruptData | Failure | `ProvenanceEvidence` | A checkpoint claims authority or completeness it does not establish. |

## 39. Replay

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `replay.starting_execution_digest_mismatch` | Error | Compatibility | Failure | `ReplayEvidence` | The machine does not match the replay log's required starting execution digest. |
| `replay.starting_observable_digest_mismatch` | Error | Compatibility | Failure | `ReplayEvidence` | The machine does not match the required starting observable digest. |
| `replay.expected_revision_mismatch` | Error | Compatibility | Failure | `ReplayEvidence` | The machine revision differs from the frame's expected revision. |
| `replay.runtime_policy_mismatch` | Error | Compatibility | Failure | `ReplayEvidence` | The machine policy identity differs from the frame or log. |
| `replay.time_domain_mismatch` | Error | Compatibility | Failure | `ReplayEvidence` | Replay artifacts and machine use different time domains. |
| `replay.network_fingerprint_mismatch` | Error | Compatibility | Failure | `ReplayEvidence` | Replay and machine network fingerprints differ. |
| `replay.target_fingerprint_mismatch` | Error | Compatibility | Failure | `ReplayEvidence` | Re-preparing a patch does not reproduce the recorded target fingerprint. |
| `replay.patch_preparation_diverged` | Error | Compatibility | Failure | `ReplayEvidence` | A recorded patch cannot be prepared equivalently under the current supported semantics. |
| `replay.resulting_execution_digest_mismatch` | Error | CorruptData | Failure | `ReplayEvidence` | Applied frame result differs from the recorded execution digest. |
| `replay.resulting_observable_digest_mismatch` | Error | CorruptData | Failure | `ReplayEvidence` | Applied frame result differs from the recorded observable digest. |
| `replay.recorded_result_mismatch` | Error | CorruptData | Failure | `ReplayEvidence` | A retained transaction result does not match the actual normalized result. |
| `replay.frame_missing` | Error | CorruptData | Failure | `ReplayEvidence` | A declared replay sequence omits a required frame. |
| `replay.frame_reordered` | Error | CorruptData | Failure | `ReplayEvidence` | Frame order contradicts sequence, digest, or prior-state linkage. |
| `replay.frame_duplicated` | Error | CorruptData | Failure | `ReplayEvidence` | One frame is duplicated within a sequence where it cannot validly occur twice. |
| `replay.chunk_sequence_mismatch` | Error | CorruptData | Failure | `ReplayEvidence` | Replay chunk numbering or boundary identities are inconsistent. |
| `replay.chunk_link_mismatch` | Error | CorruptData | Failure | `ReplayEvidence` | A chunk's previous-content digest does not match its predecessor. |
| `replay.logs_not_concatenable` | Error | Compatibility | Failure | `ReplayEvidence` | Two replay logs do not share the exact required boundary identities. |

An ordinary runtime failure while applying a valid frame retains its underlying code and adds replay frame context. It does not become a generic `replay.frame_failed` code.

## 40. Standard modules — blocking conditions

These identifiers are inherited from the Standard Module Catalogue.

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `standard_module.unknown_id` | Error | UnsupportedFeature | Report, Failure | `StandardModuleEvidence` | No descriptor exists for the identifier. |
| `standard_module.unsupported_version` | Error | Compatibility | Report, Failure | `StandardModuleEvidence` | The exact semantic or expansion version is unavailable. |
| `standard_module.missing_parameter` | Error | CallerInput | Report, Failure | `StandardModuleEvidence` | A required parameter is absent. |
| `standard_module.unexpected_parameter` | Error | CallerInput | Report, Failure | `StandardModuleEvidence` | The declaration contains an unknown parameter. |
| `standard_module.parameter_kind_mismatch` | Error | CallerInput | Report, Failure | `StandardModuleEvidence` | A parameter has the wrong value kind. |
| `standard_module.invalid_parameter` | Error | CallerInput | Report, Failure | `StandardModuleEvidence` | A parameter violates its schema. |
| `standard_module.interface_mismatch` | Error | CallerInput | Report, Failure | `StandardModuleEvidence` | Public ports do not match the descriptor. |
| `standard_module.expansion_mismatch` | Error | CorruptData | Report, Failure | `StandardModuleEvidence` | Persisted or patched internals differ from the canonical expansion. |
| `standard_module.noncanonical_internal_edit` | Error | CallerInput | Report | `StandardModuleEvidence` | A semantic internal edit attempts to retain standard identity. |
| `standard_module.incompatible_version_migration` | Error | Compatibility | Report, Failure | `StandardModuleEvidence` | No declared migration exists between the selected versions. |
| `standard_module.internal_key_collision` | Error | LibraryDefect | Defect, Failure | `StandardModuleEvidence` | Canonical key derivation produced a duplicate key. |
| `standard_module.catalogue_invariant` | Error | LibraryDefect | Defect, Failure | `StandardModuleEvidence` | Descriptor data and generated expansion disagree. |

A shipped-descriptor defect may be returned by an explicit catalogue-validation API, but it MUST remain classified as `LibraryDefect`.

## 41. Standard modules — non-blocking conditions

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `standard_module.deprecated_alias` | Warning | Compatibility | Report | `StandardModuleEvidence` | An authoring alias is deprecated. |
| `standard_module.empty_variadic` | Warning | Advisory | Report | `StandardModuleEvidence` | A variadic standard module has zero public inputs. |
| `standard_module.unary_degenerate` | Warning | Advisory | Report | `StandardModuleEvidence` | A one-input declaration simplifies to a constant or identity-like law. |
| `standard_module.impossible_threshold` | Warning | Advisory | Report | `StandardModuleEvidence` | `Exactly.threshold` exceeds arity, so the result is always `Low`. |
| `standard_module.constant_result` | Warning | Advisory | Report | `StandardModuleEvidence` | Parameters and arity make the public result constant. |
| `standard_module.duplicate_source` | Warning | CallerInput | Report | `StandardModuleEvidence` | One upstream source is bound to several public variadic ports. |

The suppression and coexistence rules defined by the Standard Module Catalogue remain authoritative.

## 42. Observer integration

The initial machine core does not require an observer implementation, but these codes are reserved for the specified observer boundary.

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `observer.cursor_stale` | Error | Compatibility | Failure | `ObserverEvidence` | An observer cursor cannot continue from the available committed change history. |
| `observer.resynchronization_required` | Info | Advisory | Report | `ObserverEvidence` | The observer must obtain a fresh inspection snapshot before continuing. |
| `observer.delivery_failed` | Error | ExternalIntegration | Failure | `ObserverEvidence` | Delivery failed after semantic commit; the machine remains committed and unchanged by the delivery failure. |

## 43. Internal defects — static compiled invariants

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `internal.diagnostic_code_evidence_mismatch` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | A problem code is paired with the wrong typed evidence variant. |
| `internal.diagnostic_evidence_conflict` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Duplicate detection produced contradictory scalar evidence for one condition key. |
| `internal.compiled_dense_reference_out_of_bounds` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | A compiled dense reference is outside its owning table. |
| `internal.compiled_descriptor_kind_mismatch` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | A compiled evaluator descriptor disagrees with its node kind. |
| `internal.compiled_port_kind_mismatch` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | A compiled port's signal kind disagrees with validated structure. |
| `internal.compiled_connection_driver_invalid` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Compiled incidence violates validated driver rules. |
| `internal.reaction_dependency_not_forward` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | A reaction dependency does not advance in the selected topological order. |
| `internal.reaction_cycle_after_compilation` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Recomputed reaction dependencies contain a cycle after successful compilation. |
| `internal.stable_key_lookup_ambiguous` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | A compiled stable-key lookup resolves to more than one subject. |
| `internal.endpoint_table_incomplete` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Compiled endpoint tables omit or misclassify validated endpoints. |
| `internal.state_slot_family_mismatch` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | A state slot is stored in the wrong semantic family. |
| `internal.state_slot_owner_mismatch` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | A state slot is owned by the wrong node or state role. |

## 44. Internal defects — runtime state and transaction invariants

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `internal.multiple_state_successors` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | One state cell received more than one proposed successor in one reaction. |
| `internal.pending_event_owner_invalid` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | A pending or migrated event has no valid compatible owner. |
| `internal.pending_deadline_not_future` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Newly pending work has a deadline not strictly later than its creating reaction. |
| `internal.event_calendar_minimum_mismatch` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | The optimized calendar reports a different minimum deadline from recomputation. |
| `internal.event_calendar_membership_mismatch` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Arena and deadline-index membership disagree. |
| `internal.event_key_reused` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | A public pending-event identity was reused contrary to allocator state. |
| `internal.region_partition_invalid` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Region membership is not a complete disjoint weak-component partition. |
| `internal.migration_classification_incomplete` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | A required subject, state cell, event, episode, provenance root, or baseline has no migration outcome. |
| `internal.migration_classification_duplicate` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | One migration-owned fact received more than one outcome. |
| `internal.diagnostic_episode_owner_invalid` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | An active episode refers to an absent or incompatible owner. |
| `internal.provenance_cycle` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Committed in-memory provenance is cyclic. |
| `internal.provenance_root_unreachable` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | A required current fact does not reach an authoritative provenance root. |
| `internal.provenance_subject_invalid` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Committed provenance names an invalid semantic subject. |
| `internal.failure_atomicity_violated` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | A structured failure changed published semantic machine state. |
| `internal.machine_mutated_by_inspection` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Inspection, explanation, graph query, or observer setup mutated semantic state. |

## 45. Internal defects — reference-path divergence

| Code | Severity | Responsibility | Delivery | Evidence | Meaning |
|---|---|---|---|---|---|
| `internal.incremental_reaction_divergence` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Incremental and full topological reaction evaluation differ. |
| `internal.transaction_executor_divergence` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Optimized staging and clone-and-swap execution differ. |
| `internal.event_calendar_divergence` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Optimized and ordered semantic event calendars differ. |
| `internal.reconfiguration_divergence` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Optimized and complete rebuild/migration paths differ. |
| `internal.region_divergence` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Incremental and full weak-component recomputation differ. |
| `internal.inspection_divergence` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Cached or incremental inspection differs from a fresh projection. |
| `internal.forecast_divergence` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Forecast differs from applying the same transaction to an unpublished clone. |
| `internal.replay_divergence` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Replay differs from repeated ordinary transaction application. |
| `internal.canonical_encoding_divergence` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Production canonical encoding differs from an independent validator or golden vector. |
| `internal.persistence_projection_invalid` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | A valid opaque semantic artifact cannot be projected into its required canonical persistence schema. |
| `internal.canonical_digest_mismatch` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Incremental or cached digest differs from canonical recomputation. |
| `internal.snapshot_round_trip_divergence` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Snapshot encode, decode, restore, and observation do not preserve semantic equivalence. |

---

# Part VI — Rust API integration

## 46. Required mapping by API family

The concrete Rust API MUST provide exact mappings broadly equivalent to the following.

### 46.1 Authoring and validation

```text
AuthoringFailure
    authoring.*
    validation.* detected eagerly

Report<ValidatedNetwork<D>>
Report<CompiledNetwork<D>>
    validation.*
    standard_module.*
    internal.* only through a distinct defect channel
```

### 46.2 Runtime policy and values

```text
PolicyFailure
    runtime.policy_missing_limit
    runtime.policy_invalid_limit

TimeArithmeticError
    runtime.time_overflow
    runtime.invalid_time_subtraction

ZeroSpanError
    runtime.zero_span_not_allowed

PulseCountOverflow
    runtime.pulse_count_overflow
```

### 46.3 Inputs and bindings

```text
InputBuildFailure
InputFailure
    input.*
    lifecycle.* where lifecycle is the actual condition

BindingFailure
    binding.*
    validation.signal_kind_mismatch where the semantic definition itself is mismatched
```

### 46.4 Runtime application

```text
RuntimeFailure
    lifecycle.*
    runtime.stale_revision
    runtime.stale_execution_state
    runtime.time_*
    input.*
    reconfiguration.* finalization codes
    runtime.* semantic rejection and budget codes
```

Runtime occurrences and episode events appear in a successful `TransactionResult`, not in `RuntimeFailure`.

### 46.5 Inspection and explanation

```text
InspectionFailure
InspectionPlanFailure
GraphQueryFailure
    inspection.*
    lifecycle.not_initialized

ExplanationFailure
    explanation.*
    lifecycle.not_initialized
```

### 46.6 Reconfiguration

```text
PatchBuildFailure
    reconfiguration.* patch-construction codes

Report<PreparedPatch<D>>
    reconfiguration.* preparation codes
    validation.* target-graph codes
    standard_module.*

ReconfigurationFailure
    reconfiguration.* finalization codes
    runtime.time_overflow
    runtime.budget_exceeded
```

### 46.7 Persistence and replay

```text
EncodeFailure
DecodeFailure
RestoreFailure
TransactionMaterializationFailure
    persistence.*
    validation.* for decoded semantic definitions
    standard_module.*

ReplayFailure
    replay.*
    persistence.* underlying artifact failures
    validation.* or reconfiguration.* during patch materialization
    ordinary RuntimeFailure codes while applying a frame
```

### 46.8 Internal defects

```text
InternalDefect
    internal.*
    standard_module.internal_key_collision
    standard_module.catalogue_invariant
    persistence.digest_collision when established as an in-memory implementation defect
```

## 47. No arbitrary fallback strings

Public failure enums MUST NOT contain a fallback variant whose only authoritative content is:

```text
Other(String)
Internal(String)
InvalidData(String)
```

An `Unknown` variant may exist for forward-compatible foreign-language bindings only when it retains the complete unknown code and canonical raw evidence. The Rust core must not emit it for a current-schema condition.

## 48. Non-exhaustive enums

Failure and evidence enums SHOULD remain `#[non_exhaustive]` while the diagnostic schema is experimental.

This does not permit emitters to invent unregistered variants. Every emitted variant must be present in the current catalogue registry.

---

# Part VII — Persistence and compatibility

## 49. Diagnostic schema version

`DiagnosticSchemaVersion` governs:

```text
code registry
severity assignment
responsibility assignment
allowed delivery forms
subject requirements
condition discriminator
structured evidence interpretation
suggestion schemas
episode identity and lifecycle
canonical diagnostic encoding
```

Rendered wording and localization are excluded.

## 50. Canonical persisted form

Persisted problem records contain:

```text
diagnostic schema version
code string
severity
responsibility
primary subject
related subjects with roles
typed evidence variant and payload
machine-readable suggestions
episode identity and lifecycle fields where applicable
```

Severity and responsibility are stored for audit readability but MUST be verified against the code registry during decoding.

A mismatch is `persistence.diagnostic_schema_invalid`.

Rendered summary text MAY be stored only as presentation metadata. It does not establish the condition and does not participate in execution-state identity.

## 51. Unknown codes

Under a known supported `DiagnosticSchemaVersion`, an unknown code or evidence variant is invalid persisted data.

Under an unsupported diagnostic schema version, decoding fails with `persistence.unsupported_version` and identifies `DiagnosticSchemaVersion` as the incompatible component.

The decoder MUST NOT silently treat an unknown problem as an unstructured string.

## 52. Episode persistence

Active episode state participates in:

- execution-state digests;
- snapshots;
- restoration;
- replay;
- reconfiguration migration;
- current inspection.

Completed episode history is optional archival state unless another specification requires retention.

A restored episode must reproduce its exact identity, code, owner, discriminator, begin time, last material change, and current evidence.

---

# Part VIII — Verification obligations

## 53. Catalogue completeness

The implementation MUST maintain a machine-readable registry containing every catalogue entry and its normative metadata.

The registry must prove that:

- every public leaf failure variant maps to exactly one code;
- every report-producing validation rule maps to exactly one code;
- every runtime occurrence and persistent episode condition maps to exactly one code;
- every named debug invariant and required reference comparison maps to exactly one internal code;
- every code maps to exactly one severity, responsibility, evidence schema, condition discriminator, and allowed delivery set;
- no code is emitted outside its allowed delivery forms;
- no code/evidence mismatch is constructible through safe typed APIs.

## 54. Structural comparison

Tests compare problem records by:

```text
code
severity
responsibility
primary subject
related subjects and roles
typed evidence
suggestions
episode identity and event kind where applicable
```

Rendered prose is excluded from semantic equality.

## 55. Golden catalogue

CI MUST generate or validate:

```text
Rust code constants or enum variants
evidence variants and schemas
failure-to-code mappings
Markdown or rustdoc reference pages
canonical persisted vectors
editor schema data
code ownership and delivery tables
```

Generated output must be deterministic and reviewable.

## 56. Determinism and permutation tests

Tests MUST permute:

```text
definition insertion order
hash-map iteration
patch operation insertion order
validator execution order
unordered supporter order
equal-deadline event storage order
diagnostic discovery order
```

Canonical diagnostic sets must remain identical.

## 57. Deduplication tests

For every deduplicating code, tests must cover:

```text
same detector reporting twice
independent detectors reporting the same condition
same wording on different subjects
same subject with different condition discriminators
mergeable set-like evidence
contradictory scalar evidence
```

Contradictory scalar evidence must produce `internal.diagnostic_evidence_conflict` in verification configurations.

## 58. Episode tests

Every episode-capable code must cover:

```text
inactive -> active
active unchanged -> active unchanged
active evidence materially changes
active -> resolved
active -> terminated by topology removal
preserved subject through patch
explicit subject reassociation
incompatible semantic migration
rejected transaction with candidate episode change
snapshot and restoration
replay
```

## 59. Internal-defect tests

Every required debug invariant check must have at least one test demonstrating that deliberate corruption or an injected candidate/reference mismatch produces the exact internal code and evidence variant.

Internal defects must retain:

```text
semantic boundary
involved stable subjects
candidate and recomputed observations
seed and minimized reproducer where generated
```

A test that converts a library defect into a caller-responsibility code is invalid.

## 60. Failure atomicity

For every operation-failure code capable of arising after candidate state construction begins, tests must compare the complete semantic machine before and after rejection.

The state must remain identical, including:

```text
lifecycle
logical time
topology revision
external levels
settled values
node and temporal state
pending events and allocator state
output baselines
provenance roots
active diagnostic episodes
digests
```

## 61. Release gates

A release claiming a feature MUST NOT proceed when:

- a documented failure condition lacks a catalogue entry;
- a public leaf failure emits only prose;
- a code can be paired with incompatible evidence;
- diagnostic order is nondeterministic;
- an episode repeats unchanged warnings;
- a structured rejection partially mutates machine state;
- a named internal invariant can fail without a structured defect;
- a current-schema persisted diagnostic does not round-trip canonically;
- an intentional experimental schema change was made without advancing `DiagnosticSchemaVersion` and updating current golden data.

After a future diagnostic-schema freeze, the release gates must additionally enforce stabilized-code compatibility.

---

# Part IX — Implementation guidance

## 62. Authoritative registry

The implementation MUST represent the catalogue once in an authoritative declarative registry.

Each registered code MUST declare at least:

```text
code string
diagnostic schema version introduced
severity
responsibility
allowed delivery forms
blocking behavior where report delivery is allowed
primary-subject requirements
allowed related-subject roles
condition-discriminator schema
exact code-specific evidence variant and payload schema
allowed machine-readable suggestions
occurrence or episode behavior
persistence and digest participation
rendering key and documentation reference
```

The implementation SHOULD derive from that registry:

```text
DiagnosticCode constants or enum variants
ProblemEvidence variants and validation
severity and responsibility lookup
report ordering and condition-key logic
canonical ordering and encoding
rendering templates
failure mappings
documentation tables
persistence schemas
conformance tests
```

The registry is the implementation form of this specification, not an independent semantic source. Generated checks MUST detect disagreement between the registry and this document while the catalogue remains manually maintained.

Hand-maintained duplicate code lists, evidence mappings, or severity tables MUST be avoided.

## 63. Rendering

Rendering receives only the structured problem record and presentation context.

It MAY use:

```text
diagnostic metadata
current names and paths
localization
source locations
verbosity level
```

Rendering MUST NOT change code, severity, responsibility, subjects, evidence, or suggestions.

Changing a name or diagnostic path after reconfiguration changes rendering, not condition identity.

## 64. Development diagnostics

Debug and test configurations SHOULD enable expensive checks corresponding to the `internal.*` catalogue.

Production builds MAY omit continuous recomputation when validated construction and ordinary runtime maintenance establish the same invariants.

Omitting a check does not remove its code from the catalogue. If the condition is detected by another path, it uses the same code.

## 65. No diagnostic signal semantics

Problem records are not `Level` or `Pulse` signals.

The evaluator does not emit library failures through network ports unless a future node explicitly defines a domain-neutral fault signal as part of its actual signal law.

Diagnostics remain structured library observations and operation results.

---

# Summary

The diagnostic system is one coherent machine-readable account of every significant failure and warning in `mossignal`.

Its defining properties are:

```text
one unified problem-code namespace
codes identify semantic conditions rather than call sites
separate severity, responsibility, and delivery
closed typed evidence per code
stable semantic subjects and related-subject roles
deterministic ordering and deduplication
unambiguous machine-readable suggestions only
separate report, failure, occurrence, episode, and defect lifecycles
persistent episode state with exact migration and persistence
internal invariants represented through the same structured system
experimental pre-freeze evolution through DiagnosticSchemaVersion
canonical persistence independent from rendered wording
exhaustive mappings from every public failure and named debug invariant
```

This gives downstream users precise diagnostics while also making failures discovered during property testing, differential verification, fuzzing, restoration, and reconfiguration directly reproducible and actionable during development of the library itself.
