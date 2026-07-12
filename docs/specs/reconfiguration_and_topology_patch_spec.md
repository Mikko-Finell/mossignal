# `mossignal` Reconfiguration and Topology-Patch Specification

**Status:** Design specification, version 1  
**Defines:** Topology-patch values, the exhaustive patch-operation language, declarative graph-rewrite semantics, structural preparation, stable correspondence, state and pending-work migration, module replacement, target input schemas, transaction-time finalization, semantic-loss policy, migration reports, diagnostics, and verification obligations  
**Does not define:** General processor execution, built-in node behavior outside reconfiguration, serialized wire encodings, the exhaustive diagnostic-code catalogue, performance targets, editor interaction, automatic multi-user patch merging, or unrestricted user-defined migration callbacks

---

## 1. Purpose

This specification defines how a running or uninitialized `mossignal` machine changes topology without introducing hidden ordering, silent state loss, stale identity, or partial publication.

A topology patch may:

- add, remove, or replace nodes;
- add, remove, or redirect connections;
- add, remove, or replace external endpoints;
- change built-in semantic parameters;
- change module instances and hierarchy;
- change diagnostic metadata;
- explicitly associate a removed structural subject with a newly introduced successor for migration;
- choose built-in migration policies for compatible state and pending temporal work.

The design must preserve the guarantees established by the API, node, processor, and verification specifications:

- deterministic behavior;
- exact caller-owned logical time;
- atomic commitment;
- stable structural identity;
- complete migration accounting;
- explicit state-loss policy;
- current-reaction acyclicity;
- state-preserving compatible revisions;
- exact pending-event treatment;
- causal explanation of migration and topology-induced change;
- snapshot and replay equivalence.

The central rule is:

> A patch changes structure declaratively. Preparation determines every possible migration rule from topology alone. Commitment determines the actual migration outcome from the state reached at the patch’s effective logical time.

---

## 2. Normative language

The terms **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.

A **base topology** is the compiled topology against which a patch was authored.

A **target topology** is the complete topology produced by applying the patch structurally to the base topology.

A **preserved subject** retains the same stable key and compatible structural category in both topologies.

A **reassociated subject** has a different target stable key but is explicitly nominated as the migration successor of one base subject.

A **surviving subject** is either preserved or reassociated.

A **removed subject** has no migration successor.

A **new subject** has no migration predecessor.

A **state migration** transfers, transforms, resets, or rejects semantic state owned by a surviving node or runtime subject.

A **pending-work migration** classifies one actual pending temporal obligation at finalization.

A **semantic loss** is a finalized loss of future-determining or required observable semantic information, as defined in Part IX.

---

# Part I — Patch semantic model

## 3. Patch as a declarative graph rewrite

A topology patch is semantically a graph rewrite:

```text
G_base  <-  C  ->  G_target
```

where:

- `G_base` is the complete stable-keyed base definition;
- `G_target` is the complete stable-keyed target definition;
- `C` is a partial one-to-one correspondence between surviving structural subjects.

The correspondence identifies which old and new subjects may participate in continuity of:

- node state;
- temporal state;
- pending events;
- external input valuation;
- external output baselines;
- active diagnostic episodes;
- current provenance roots;
- module-instance identity context.

Stable-key equality establishes the ordinary correspondence. Explicit reassociation may establish additional correspondence where keys change.

The patch operation list is an authoring representation. Its semantic meaning is the resulting target graph, correspondence, and migration directives—not the order in which operations were appended.

## 4. Declarative order independence

Equivalent patches MUST produce equivalent preparation artifacts regardless of operation insertion order.

For any permutation of a patch’s mutually consistent operations:

```text
Prepare(base, operations)
```

must produce the same, up to representation-only ordering:

- target definition;
- target fingerprint;
- current-reaction dependency graph;
- stable correspondence;
- static migration plan;
- potential semantic-loss classification;
- invalidated-artifact classification;
- diagnostics.

The implementation MUST NOT interpret a patch as a sequence of temporarily malformed intermediate graphs.

For example, removing a connection and then removing its node is equivalent to listing those operations in the opposite order. Validation considers only the base graph, the complete operation set, and the resulting target graph.

## 5. Base binding

Every `NetworkPatch<D>` is bound to:

```text
NetworkKey
base NetworkFingerprint
base NetworkRevision
signal-semantics version
logical time domain D
```

A patch MUST NOT be prepared against:

- another network identity;
- another base fingerprint;
- another signal-semantics version;
- another time domain.

`Machine::prepare_patch` MUST additionally require the patch’s base revision to equal the machine’s current topology revision.

A patch does not bind to an execution-state digest. State-only transactions therefore do not invalidate an otherwise compatible prepared patch.

## 6. Topology revision and fingerprint

A committed effective patch advances the machine-local topology revision exactly once.

This remains true when the patch changes only:

- diagnostic metadata;
- hierarchy that does not affect the semantic fingerprint;
- presentation-only structure excluded from fingerprinting.

A metadata-only patch may therefore have:

```text
base fingerprint == target fingerprint
base revision != target revision
```

A patch with no effective structural, semantic, hierarchical, endpoint, or metadata change MUST be rejected as an empty patch. It MUST NOT consume a topology revision.

## 7. No implicit rebasing or merging

The initial core does not automatically:

- rebase a patch onto a different topology revision;
- merge independently authored patches;
- infer conflict resolution between patches;
- compose prepared patches across an intervening topology revision;
- infer stable correspondence from names or structural similarity.

A caller may construct a new patch explicitly against the new base topology.

Patch composition MAY be added later only with explicit base/target compatibility and deterministic conflict semantics.

---

# Part II — Structural correspondence and identity

## 8. Ordinary stable-key preservation

When the same stable key exists in base and target with the same structural category and compatible signal kind, it is an ordinary candidate for preservation.

Examples include:

```text
NodeKey -> NodeKey
ConnectionKey -> ConnectionKey
ModuleInstanceKey -> ModuleInstanceKey
InPortKey<Level> -> InPortKey<Level>
ExternalInputKey<Pulse> -> ExternalInputKey<Pulse>
```

Stable-key equality does not by itself guarantee state compatibility. The target definition and migration rules must still permit preservation or migration.

A stable key MUST NOT change category. For example, one numeric payload cannot cause a `NodeKey` to become a `ConnectionKey`, nor may a `Level` port become a `Pulse` port under the same typed key.

## 9. Explicit reassociation

A patch may explicitly associate one removed base subject with one new target subject of a compatible category.

Representative shape:

```rust
#[non_exhaustive]
pub enum SubjectReassociation<D> {
    Node {
        from: NodeKey,
        to: NodeKey,
        migration: NodeMigrationDirective<D>,
    },
    ModuleInstance {
        from: ModuleInstanceKey,
        to: ModuleInstanceKey,
        migration: ModuleMigrationDirective<D>,
    },
    InPort {
        from: AnyInPortKey,
        to: AnyInPortKey,
    },
    OutPort {
        from: AnyOutPortKey,
        to: AnyOutPortKey,
    },
    ExternalInput {
        from: AnyExternalInputKey,
        to: AnyExternalInputKey,
    },
    ExternalOutput {
        from: AnyExternalOutputKey,
        to: AnyExternalOutputKey,
    },
}
```

Reassociation permits migration continuity but does not pretend that the stable key itself survived.

The result MUST report:

- termination of the old stable identity;
- introduction of the new stable identity;
- the explicit migration correspondence between them;
- invalidation of all revision-bound handles referring to the old identity.

Connection reassociation is unnecessary for semantic state because connections own no runtime signal state. A connection whose key changes is removed and added. Tooling may correlate such changes through caller metadata, but that correlation is non-semantic.

## 10. Correspondence constraints

The complete correspondence relation MUST be a partial injection in both directions.

Therefore:

- one base subject maps to at most one target subject;
- one target subject maps from at most one base subject;
- a subject cannot be both preserved by equal key and reassociated elsewhere;
- reassociation cannot form chains or cycles within one patch;
- source and target categories must match;
- typed signal subjects must retain signal kind and direction;
- both ends must belong to the same network patch;
- the source must exist only in the base result;
- the target must exist only in the target result.

One-to-many splitting and many-to-one merging of stateful identity are not part of the initial core.

A future node-specific merge or split migration would require a separately specified primitive with explicit algebra, provenance, and loss semantics.

## 11. Port correspondence

Port correspondence is based on:

1. equal stable typed port key; or
2. explicit same-kind, same-direction reassociation.

For fixed-shape built-in nodes, compatibility additionally requires that corresponding ports occupy compatible semantic roles.

For example, a `Select` node’s selector cannot be reassociated with its `when_high` input merely because both are `Level` inputs.

For commutative variadic nodes, individual variadic ports retain stable identity even though display order is non-semantic.

Port correspondence affects:

- connection validity;
- graph queries;
- diagnostic continuity;
- explanation paths;
- node-specific migration compatibility where semantic roles matter.

## 12. Module-qualified internal identity

For module internals, the logical structural identity is:

```text
(ModuleInstanceKey, module-internal stable key)
```

A compatible module-instance replacement preserves an internal subject when:

- the module instance survives or is explicitly reassociated;
- the internal stable key survives;
- the internal subject category and kind remain compatible.

Flattened compiler keys or dense positions MUST NOT determine module-internal continuity.

---

# Part III — Canonical patch-operation language

## 13. Required operation enum

The canonical operation language is a closed owned value family broadly equivalent to:

```rust
#[non_exhaustive]
pub enum PatchOperation<D> {
    AddNode(NodeDef<D>),
    RemoveNode {
        node: NodeKey,
    },
    ReplaceNode {
        node: NodeKey,
        replacement: NodeDef<D>,
        migration: NodeMigrationDirective<D>,
    },

    AddConnection(ConnectionDef),
    RemoveConnection {
        connection: ConnectionKey,
    },
    ReplaceConnection {
        connection: ConnectionKey,
        replacement: ConnectionDef,
    },

    AddExternalInput(ExternalInputDef),
    RemoveExternalInput {
        input: AnyExternalInputKey,
    },
    ReplaceExternalInput {
        input: AnyExternalInputKey,
        replacement: ExternalInputDef,
    },

    AddExternalOutput(ExternalOutputDef),
    RemoveExternalOutput {
        output: AnyExternalOutputKey,
    },
    ReplaceExternalOutput {
        output: AnyExternalOutputKey,
        replacement: ExternalOutputDef,
    },

    AddModuleInstance(ModuleInstanceDef<D>),
    RemoveModuleInstance {
        module: ModuleInstanceKey,
    },
    ReplaceModuleInstance {
        module: ModuleInstanceKey,
        replacement: ModuleInstanceDef<D>,
        migration: ModuleMigrationDirective<D>,
    },

    SetParent {
        subject: HierarchicalSubjectRef,
        parent: Option<ModuleInstanceKey>,
    },
    SetDiagnosticMeta {
        subject: StructuralSubjectRef,
        meta: DiagnosticMeta,
    },
    Reassociate(SubjectReassociation<D>),
}
```

Exact private representation may differ, but every operation family and its semantics are required.

The operation enum MUST remain data. It MUST NOT contain arbitrary closures, trait objects, or host callbacks.

The supporting structural shapes are broadly equivalent to:

```rust
pub struct ModuleInstanceDef<D> {
    pub key: ModuleInstanceKey,
    pub module: ModuleDef<D>,
    pub bindings: ModuleBindingSet,
    pub parent: Option<ModuleInstanceKey>,
    pub meta: DiagnosticMeta,
}

#[non_exhaustive]
pub enum StructuralSubjectRef {
    Network(NetworkKey),
    Module(ModuleInstanceKey),
    Node(NodeKey),
    InPort(AnyInPortKey),
    OutPort(AnyOutPortKey),
    Connection(ConnectionKey),
    ExternalInput(AnyExternalInputKey),
    ExternalOutput(AnyExternalOutputKey),
}

#[non_exhaustive]
pub enum ModuleMigrationDirective<D> {
    Standard,
    Explicit {
        node_overrides: Vec<ModuleNodeMigrationDirective<D>>,
        internal_reassociations: Vec<ModuleInternalReassociation<D>>,
    },
}
```

`ModuleBindingSet`, `ModuleNodeMigrationDirective`, and `ModuleInternalReassociation` are owned stable-keyed values. They contain no runtime callbacks. Node overrides use the same closed `NodeMigrationDirective<D>` family defined in Part V.

## 14. Node operations

### 14.1 `AddNode`

`AddNode` introduces one complete `NodeDef<D>` whose node key does not exist in the base topology unless it is the target of an explicit valid reassociation.

The definition includes:

- node key;
- node kind and semantic parameters;
- complete fixed or variadic port definition;
- module membership where represented structurally;
- diagnostic metadata.

A new stateful or temporal node receives its declared initial state and no inherited pending work unless explicit reassociation and migration say otherwise.

### 14.2 `RemoveNode`

`RemoveNode` removes one base node.

The canonical operation does not cascade implicitly.

Every incident connection, endpoint source, or hierarchy reference that would otherwise dangle MUST be separately removed, replaced, or redirected in the same patch.

Removing a stateful or temporal node exposes potential state or pending-work loss during preparation and actual loss during finalization.

### 14.3 `ReplaceNode`

`ReplaceNode` supplies the complete target definition for an existing node key.

The replacement definition MUST use the same `NodeKey` as the operation subject. A key-changing replacement uses remove, add, and explicit reassociation.

`ReplaceNode` covers:

- changing semantic parameters;
- changing node kind;
- changing fixed or variadic port shape;
- adding or removing variadic ports;
- changing port keys;
- changing node metadata embedded in the definition;
- changing module membership where embedded in the definition.

Every removed or changed port incidence must be handled explicitly by the connection operations in the same patch.

## 15. Connection operations

### 15.1 `AddConnection`

`AddConnection` introduces one complete stable-keyed `ConnectionDef`.

The target topology must validate:

- source existence;
- destination existence;
- direction;
- signal-kind equality;
- driver policy;
- module visibility;
- current-reaction causality.

### 15.2 `RemoveConnection`

`RemoveConnection` removes one existing connection.

Removing a connection does not reset state by itself. It changes the target reaction graph and seeds ordinary reevaluation of affected operations.

### 15.3 `ReplaceConnection`

`ReplaceConnection` preserves the connection key while replacing its complete definition, including endpoints or metadata.

The replacement definition MUST use the same `ConnectionKey`.

Redirecting a connection is not represented as a remove followed by an add when connection identity is intended to survive.

Connections own no independent runtime signal state. Their preservation affects identity, diagnostics, graph queries, and causal structure rather than state migration.

## 16. External input operations

### 16.1 `AddExternalInput`

A new external `Level` input creates a target-schema establishment obligation.

A ready-machine patch transaction MUST explicitly establish it through the prepared patch’s target-bound `InputDelta`.

An initialization patch transaction supplies it through the complete target-bound `InputSnapshot`.

A new external `Pulse` input requires no persistent establishment value. Absence in the current batch means zero occurrences.

### 16.2 `RemoveExternalInput`

A removed external input is absent from the target input schema.

A removed external `Level` input loses its authoritative stored valuation unless it is explicitly reassociated with a target external `Level` input.

Every connection sourced by the removed input must be removed or redirected explicitly.

### 16.3 `ReplaceExternalInput`

`ReplaceExternalInput` preserves an endpoint key while replacing its complete definition.

The replacement definition MUST use the same typed endpoint key as the operation subject.

Because signal kind is encoded in the typed key, a replacement MUST retain signal kind.

Changing `Level` to `Pulse` or `Pulse` to `Level` requires removal and addition and cannot carry input valuation across kinds.

## 17. External output operations

### 17.1 `AddExternalOutput`

A new external output must refer to a valid target signal source.

A new `Level` output has no prior published baseline and emits `LevelEstablished` when the patch-time reaction settles.

A new `Pulse` output emits only if the patch-time reaction genuinely produces a nonzero pulse count.

### 17.2 `RemoveExternalOutput`

Removing an output produces a topology consequence, not a fabricated signal event.

A removed `Level` output baseline is semantic state loss under the rules in Part IX.

### 17.3 `ReplaceExternalOutput`

`ReplaceExternalOutput` preserves the output key while replacing its source or metadata.

The replacement definition MUST use the same typed endpoint key as the operation subject.

Signal kind must remain unchanged.

For a preserved `Level` output, its prior published baseline is compared with the target settled value after patch-time reaction settlement.

## 18. Module-instance operations

### 18.1 `AddModuleInstance`

A module instance operation introduces:

- the instance key;
- referenced validated module definition or fingerprint-bound module artifact;
- input bindings;
- hierarchy parent;
- diagnostic metadata;
- expanded internal structural subjects.

All module inputs must be bound exactly once.

### 18.2 `RemoveModuleInstance`

Removing a module instance does not implicitly remove descendants, external connections, or exported endpoint use in the canonical patch model.

The patch must explicitly remove or relocate every affected structural subject and incidence.

Builder conveniences MAY expand a caller-requested cascading removal into the required explicit canonical operations before `finish`.

### 18.3 `ReplaceModuleInstance`

The replacement definition MUST use the same `ModuleInstanceKey` as the operation subject. A key-changing module replacement uses remove, add, and explicit reassociation.

Replacing a module instance may change:

- the module definition revision or fingerprint;
- module input binding;
- module output exposure;
- internal structure;
- parameters;
- hierarchy;
- metadata.

Preparation expands both base and target definitions, derives internal stable-key correspondence, and applies the same node, port, connection, state, and pending-work rules as a direct graph patch.

A convenience for updating one reusable module definition across several instances MUST expand into explicit replacement operations for every affected instance. The update commits atomically as one patch.

## 19. Hierarchy operations

`SetParent` changes the parent module instance of a hierarchical subject.

The initial hierarchical subject set includes:

```rust
#[non_exhaustive]
pub enum HierarchicalSubjectRef {
    Node(NodeKey),
    ModuleInstance(ModuleInstanceKey),
}
```

A hierarchy change MUST preserve:

- acyclic containment;
- module-interface visibility;
- stable subject identity;
- target graph validity.

Moving a subject between modules does not by itself reset compatible runtime state or cancel compatible pending work.

Hierarchy may affect the semantic fingerprint where it affects encapsulation, module identity, or state-relevant structure. Presentation-only diagnostic paths remain excluded.

## 20. Metadata operations

`SetDiagnosticMeta` replaces the complete diagnostic metadata value for one surviving subject.

Metadata changes:

- advance topology revision;
- may change graph and inspection rendering;
- may change diagnostic paths;
- MUST NOT affect execution;
- MUST NOT affect state compatibility;
- MUST NOT affect semantic fingerprint unless a field is explicitly reclassified as semantic in a future version.

Names and descriptions MUST NOT be used to infer correspondence or migration.

## 21. Convenience operations

Builder APIs MAY expose conveniences such as:

```text
change node parameter
add variadic port
remove variadic port
redirect connection
rename subject
move node into module
remove node with incident connections
replace module definition in all selected instances
```

Every convenience MUST expand deterministically into the canonical operation language before patch completion.

The canonical patch artifact MUST expose or retain the normalized operation set for inspection, diagnostics, persistence, and testing.

## 22. Forbidden implicit edits

The core MUST NOT silently:

- remove incident connections when a node or port is removed;
- choose a new driver for an input;
- reconnect by matching names;
- preserve state across changed keys without explicit reassociation;
- reset a node because migration was inconvenient;
- cancel pending work because an event structure was difficult to migrate;
- establish a new external level input as `Low`;
- synthesize pulses to represent topology change;
- infer a temporal rescheduling policy;
- reparent orphaned hierarchy automatically;
- mutate caller-owned binding sets.

---

# Part IV — Patch normalization and structural preparation

## 23. Patch construction

A patch builder is created from one compiled topology and explicit base revision:

```rust
let patch = compiled
    .patch(base_revision)
    .add_node(node)?
    .remove_connection(connection)?
    .finish();
```

A machine convenience supplies its current revision:

```rust
let patch = machine.patch();
```

`NetworkPatchBuilder<D>` owns its operations and MUST NOT borrow the machine after construction.

## 24. Immediate builder checks

The builder SHOULD reject immediately detectable contradictions, including:

- duplicate non-identical additions of one key;
- two conflicting replacements of one subject;
- removal and replacement of the same subject;
- metadata assignment to a subject also removed;
- contradictory parent assignments;
- contradictory reassociations;
- reassociation kind or direction mismatch visible from operation data;
- a replacement definition whose key differs from the replacement subject;
- an operation referencing another network’s typed artifact where detectable.

The builder MAY accept target-invalid structure that requires full graph context to diagnose. Complete validation belongs to structural preparation.

## 25. Normalization

Patch completion MUST produce a deterministically normalized operation set.

Normalization may occur incrementally in builder methods or when `finish` consumes the builder. Because operation methods reject local contradictions, `finish` remains infallible as required by the concrete API.

Normalization includes:

- canonical sorting by operation family and stable subject identity;
- deduplication of byte-for-byte or semantically identical duplicate assignments;
- folding compatible metadata or hierarchy updates into complete target definitions where useful;
- deriving one effective edit per structural subject and property;
- rejecting contradictory assignments through the operation method that introduces them;
- validating one-to-one reassociation form.

A syntactically empty or semantically ineffective patch may still exist as an owned value, but structural preparation MUST reject it as an empty effective patch and return no artifact.

Normalization MUST NOT inspect machine runtime state.

## 26. Target construction

Structural preparation constructs the target definition as a pure deterministic function:

```text
TargetDefinition = Rewrite(BaseDefinition, NormalizedPatch)
```

The rewrite is all-at-once. No intermediate operation state is semantically relevant.

The target definition is then validated through the ordinary complete network validation path.

## 27. Dangling and incidence condition

The target graph must contain no dangling structure.

At minimum:

- every connection source exists;
- every connection destination exists;
- every external output source exists;
- every module binding endpoint exists;
- every hierarchy parent exists;
- every removed port has no surviving incident connection;
- every removed module interface has no surviving external incidence;
- every fixed node port required by the target node kind exists exactly once;
- every input driver policy is satisfied.

The implementation SHOULD diagnose the exact missing companion edits required to make the rewrite valid.

This condition corresponds to the established dangling condition used in algebraic graph rewriting: deletion is valid only when no unaccounted incident structure survives.

## 28. Structural validation

Preparation MUST perform the complete validation required for an independently authored network, including:

- stable-key uniqueness;
- node and endpoint existence;
- kind compatibility;
- direction;
- fixed and variadic arity;
- driver rules;
- module-interface validity;
- hierarchy acyclicity;
- semantic parameter validity;
- initial-state validity;
- state-schema validity;
- current-reaction dependency construction;
- SCC-based reaction-cycle detection;
- target fingerprint construction.

A target topology that would not compile independently MUST NOT produce a `PreparedPatch` artifact.

## 29. `PreparedPatch<D>`

Successful preparation produces an immutable artifact:

```rust
pub struct PreparedPatch<D> { /* opaque shared target and migration program */ }
```

It contains at least:

```text
base network identity
base fingerprint
base revision
proposed revision
target validated definition
target compiled topology
target fingerprint
normalized patch operations
stable correspondence
static migration plan
target input schema
potential semantic-loss predicates
region merge and split analysis
invalidated artifact analysis
diagnostics
```

It MUST NOT contain exact runtime migration outcomes that depend on the machine state at the future effective time.

`PreparedPatch<D>` SHOULD be cheaply cloneable through immutable shared ownership.

## 30. Static migration plan

The prepared static plan MUST classify every relevant structural subject.

Representative shape:

```rust
pub struct StaticMigrationPlan<D> {
    pub subjects: Vec<StaticSubjectPlan<D>>,
    pub event_rules: Vec<StaticEventRule<D>>,
    pub external_inputs: Vec<ExternalInputPlan>,
    pub external_outputs: Vec<ExternalOutputPlan>,
    pub diagnostics: Vec<DiagnosticEpisodePlan>,
    pub provenance: Vec<ProvenanceMigrationPlan>,
    pub potential_losses: Vec<PotentialSemanticLoss<D>>,
}
```

Every stateful or temporal surviving node must have exactly one static compatibility rule.

Every temporal node capable of owning pending work must have a total event-migration rule, including the case of no actual events.

## 31. Preparation is state-independent

Structural preparation MUST NOT read or depend on:

- current logical time;
- current external level valuation;
- current node state;
- current temporal state;
- actual pending events;
- current output baselines;
- active diagnostic episodes;
- current provenance roots;
- execution-state digest.

A machine convenience may verify base revision before delegating, but the migration plan remains a topology-only artifact.

State-dependent predicates may be encoded in the static plan and evaluated only during transaction-time finalization.

## 32. Prepared-patch freshness

State-only transactions do not invalidate a prepared patch.

Any committed topology patch invalidates every prepared patch bound to the previous revision, even when the resulting fingerprint happens to be equal.

Applying a prepared patch requires exact equality of:

```text
machine NetworkKey
machine current revision
machine current fingerprint
prepared base revision
prepared base fingerprint
signal-semantics version
runtime time domain
```

Mismatch is a structured stale-patch failure.

## 33. Target-bound input schema

Preparation derives a target input schema.

For a ready-machine patch transaction:

- preserved external `Level` inputs retain their authoritative values unless changed with `set`;
- reassociated external `Level` inputs inherit the source value and are treated as preserved target inputs;
- new external `Level` inputs must be supplied through `establish`;
- removed external inputs are invalid to reference;
- target pulse inputs accept current-time counts, with absence meaning zero.

For an initialization patch transaction, the target-bound `InputSnapshot` must contain every target external `Level` input, whether preserved, reassociated, or new.

## 34. Invalidated artifacts

Preparation MUST report revision-bound artifacts made stale by commitment, including categories such as:

```text
resolved node handles
resolved endpoint handles
resolved port handles
compiled inspection plans
prepared patches based on the old revision
binding projectors bound to the old input schema
input snapshots and deltas bound to the old schema
```

Invalidation is not semantic state loss.

Stable keys that survive remain valid identities and may be resolved again against the target revision.

Binding sets are caller-owned and are not mutated by the patch. Preparation SHOULD report endpoint additions, removals, and reassociations relevant to rebinding.

## 35. Region analysis

Preparation derives region merges and splits from the complete target structural graph.

Region changes are derived consequences, not patch operations.

A region merge or split MUST NOT by itself alter compatibility of otherwise surviving state, pending work, provenance, or diagnostic episodes.

---

# Part V — State compatibility and migration

## 36. Compatibility outcomes

Every surviving stateful or temporal node receives exactly one finalized outcome:

```rust
#[non_exhaustive]
pub enum StateMigrationOutcome<D> {
    Preserve,
    Migrate {
        rule: NodeMigrationRule<D>,
    },
    Reset {
        reason: ResetReason,
    },
    Reject {
        reason: MigrationRejection,
    },
}
```

The four semantic classes are:

```text
Preserve
Migrate
Reset
Reject
```

They are mutually exclusive and collectively exhaustive for every surviving state-owning subject.

A stateless subject needs no state outcome but still receives a structural continuity classification.

## 37. Migration directives

Representative public shape:

```rust
#[non_exhaustive]
pub enum NodeMigrationDirective<D> {
    Standard,
    RequirePreserve,
    Reset,
    TransferStoredLevel,
    PulseDelay(PulseDelayMigration),
    TransportDelay(TransportDelayMigration),
    InertialDelay(InertialDelayMigration),
    Periodic(PeriodicMigration),
}
```

`Standard` selects only the built-in standard rule defined by this specification and the built-in node specification.

`RequirePreserve` rejects finalization unless the actual outcome is lossless `Preserve`.

`Reset` initializes the target node from its target declared initial state and discards incompatible source state and pending work as explicitly reported loss.

The remaining variants are closed, node-family-specific migration rules. An incompatible directive produces a preparation diagnostic and no artifact.

No arbitrary callback-defined migration is permitted.

## 38. New nodes

A genuinely new node has no migration predecessor.

At the patch-time reaction:

- declared initial state acts as previous state;
- settled current target inputs are evaluated through the ordinary node law;
- the node may produce current output and one successor state;
- a fresh temporal node starts with no inherited event schedule;
- topology-induced first-reaction behavior follows the built-in node specification.

Initialization is not semantic loss because no source state existed.

## 39. Removed nodes

A removed stateless node loses no internal state.

A removed stateful or temporal node loses every source state component for which no migration successor exists.

Every actual pending event owned by a removed temporal node receives `Cancel` unless it is transferred through one explicit surviving-node migration supported by the node family.

The initial core supports only one-to-one owner migration through node preservation or reassociation. Pending work cannot migrate to an unrelated node by name or structural similarity.

## 40. Uninitialized-machine state

An uninitialized machine still owns declared initial node and temporal state.

When a patch is committed as part of initialization:

- structural preparation remains state-independent;
- finalization uses the base declared initial state as migration input;
- ordinary preservation carries that base declared state forward;
- changing a target initial-state parameter does not overwrite preserved declared state;
- selecting `Reset` uses the target declared initial state;
- there are no prior external valuations, output baselines, pending events, runtime diagnostic episodes, or runtime provenance roots.

Thus a caller who changes an initial parameter on a preserved node before initialization must select reset if the new initial value is intended to replace the prior declared state.

## 41. Derived current port values

Current level-port values are derived runtime facts, not independently migratable authored state.

The implementation may copy compatible values as evaluation seeds, but after patch installation the target machine must be observationally equivalent to complete evaluation of the target reaction graph.

Reaction-scoped pulse values are never migrated across reactions or topology revisions.

External output baselines and stateful node values are distinct and follow their own rules.

## 42. Standard built-in compatibility matrix

The standard rule is determined by the source and target node families:

| Source and target relation | Standard state result |
|---|---|
| Same stateless built-in kind | No state; ordinary reevaluation |
| Different stateless built-in kinds | No state; allowed if target structure validates |
| Same edge-detector kind | Preserve remembered observation state |
| Different edge-detector kinds | Migrate remembered observation state |
| Same boolean-state kind | Preserve stored level |
| Different boolean-state kinds | Reject unless `TransferStoredLevel` or `Reset` is explicit |
| Same temporal kind, no migration-relevant parameter change | Preserve temporal state and apply standard pending-work rule |
| Same temporal kind, migration-relevant parameter change | Use node-specific standard or explicit temporal policy |
| Different temporal kinds | Reject unless a separately enumerated migration exists; initial version provides none |
| Stateful or temporal to stateless | Reset target/no target state; source state and work are reported loss |
| Stateless to stateful or temporal | Initialize target state; no source state loss |
| Unrelated stateful families | Reject unless reset is explicit |

The boolean-state family is:

```text
Toggle
PulseSetResetLatch
LevelSetResetLatch
SampleHold
```

The edge-detector family is:

```text
RisingEdge
FallingEdge
AnyEdge
```

The temporal family members remain distinct:

```text
PulseDelay
TransportDelay
InertialDelay
Periodic
```

## 43. Combinational nodes

Combinational nodes own no semantic state and no pending work.

Parameter, port, and connection changes cause ordinary target-graph reevaluation.

Changing between combinational kinds under one preserved node key is structurally permitted when:

- the target port shape is valid;
- every incidence is explicitly updated;
- signal kinds remain valid;
- the target reaction graph is acyclic.

No state migration policy is required.

## 44. Edge detectors

An edge detector state is:

```text
unestablished baseline
or
established remembered LogicLevel
```

For same-kind or cross-kind migration within the edge-detector family:

- an established remembered level is preserved;
- an unestablished baseline remains unestablished;
- changing `EdgeInitialization` does not retroactively alter preserved state;
- `Reset` re-enters the target initialization policy;
- target reaction evaluation compares the migrated previous observation with the settled target input and may emit according to the target detector kind.

A topology-induced edge pulse is legitimate only when the target edge equation genuinely observes a transition relative to migrated previous observation.

## 45. Boolean-state nodes

For the same boolean-state kind:

- stored `LogicLevel` is preserved;
- changing an initial-level parameter does not alter preserved state;
- connection changes do not alter stored state directly;
- the patch-time reaction applies current target controls to the preserved previous state;
- changing latch conflict policy preserves the stored level and reevaluates diagnostic-episode semantics.

`TransferStoredLevel` permits migration between different boolean-state kinds:

- the source stored `LogicLevel` becomes the target previous stored level;
- target-only auxiliary initialization fields use their target declared form;
- current target inputs are evaluated at the patch-time reaction;
- active diagnostic episodes migrate only if their condition identity remains meaningful under the target node kind;
- incompatible episodes otherwise resolve or terminate explicitly.

`TransferStoredLevel` is not inferred automatically.

## 46. Temporal-state common rules

Temporal migration distinguishes:

```text
node-owned persistent temporal state
actual pending events
current target input at patch time
```

Preparation defines the rule. Finalization applies it to actual state and events immediately before the patch-time reaction.

Every actual pending event receives exactly one outcome:

```rust
#[non_exhaustive]
pub enum PendingEventMigrationOutcome<D> {
    PreserveDeadline,
    RecomputeDeadline {
        from: Time<D>,
        to: Time<D>,
    },
    TransformPayload {
        rule: EventPayloadMigration,
    },
    Cancel {
        reason: EventCancellationReason,
    },
    Reject {
        reason: EventMigrationRejection,
    },
}
```

An event may have both deadline and payload transformation in its detailed report, but its top-level classification must remain one canonical mutually exclusive outcome chosen by the specification’s precedence rules.

Recommended precedence is:

```text
Reject > Cancel > TransformPayload > RecomputeDeadline > PreserveDeadline
```

No event disappears from the source event set without one report entry.

### 46.1 Pending-event identity

A one-to-one migration that preserves, recomputes, or transforms one obligation SHOULD preserve its `PendingEventKey`.

Cancellation terminates the key. Firing at patch time terminates pending status through the ordinary fired-event change record.

A migration that must replace one semantic event with another key must report the exact source-to-target event mapping.

Many-to-one or one-to-many event migration is not provided by the initial built-in policies. An internal representation may aggregate storage only where public event identity, multiplicity, causal contributors, and inspection remain semantically equivalent.

## 47. Deadline recomputation and overdue policy

A migration rule that computes a target deadline from an original event origin uses:

```text
new_deadline = checked_add(original_origin, target_duration)
```

Checked overflow rejects the patch transaction atomically.

If `new_deadline` is:

- greater than patch time `T`, it remains pending at that deadline;
- equal to `T`, it joins the patch-time due batch;
- less than `T`, the selected overdue policy applies.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverdueMigrationPolicy {
    Reject,
    MatureAtPatchTime,
}
```

`MatureAtPatchTime` converts the surviving obligation into a due-at-`T` fact. It does not fabricate an event at an earlier unobserved time.

This conversion is explicitly reported as temporal retiming.

An obligation migrated to `T` is an existing obligation transformed during finalization. It is not a newly scheduled same-time event and therefore does not violate the strictly-future scheduling rule for reaction-created work.

## 48. `PulseDelay` migration

The migration enum is:

```rust
#[non_exhaustive]
pub enum PulseDelayMigration {
    PreserveDeadlines,
    RecomputeFromOrigin {
        overdue: OverdueMigrationPolicy,
    },
    RestartFromPatchTime,
    CancelPending,
    RejectIfPending,
}
```

### 48.1 Standard rule

Changing delay under `Standard` uses `PreserveDeadlines`:

- existing pulse groups retain deadlines;
- existing multiplicities and causal contributors are preserved;
- the target delay applies only to pulse input received at or after the patch-time reaction.

### 48.2 `RecomputeFromOrigin`

Each group receives deadline:

```text
origin + target delay
```

Multiplicity and causal contributors are preserved.

### 48.3 `RestartFromPatchTime`

Each group receives deadline:

```text
T + target delay
```

Its original pulse cause remains in provenance, while the patch becomes the cause of rescheduling.

Discarding already elapsed wait progress is semantic loss.

### 48.4 `CancelPending`

Every actual pending group is canceled and reported as semantic loss.

### 48.5 `RejectIfPending`

Finalization rejects if any actual pending group exists. With no pending group, the parameter change proceeds.

## 49. `TransportDelay` migration

The migration enum is:

```rust
#[non_exhaustive]
pub enum TransportDelayMigration {
    PreserveDeadlines,
    RecomputeFromOrigin {
        overdue: OverdueMigrationPolicy,
    },
    RestartFromPatchTime,
    CancelPending,
    RejectIfPending,
}
```

The remembered input and current output level are preserved for same-kind survival unless reset is explicit.

### 49.1 Standard rule

Changing delay under `Standard` preserves every queued transition deadline. The target delay applies only to future input transitions.

### 49.2 Recomputed or restarted queues

Every queued transition retains:

- target level;
- originating logical time;
- originating causal support.

If migration causes several transitions to mature at one deadline, the target output for that reaction is selected by greatest originating logical time.

If two conflicting transitions have indistinguishable originating logical time and no specified semantic precedence, finalization rejects.

### 49.3 Connectivity changes

Changing input connectivity does not cancel compatible queued transitions.

At the patch-time reaction, the settled new input is compared with migrated remembered input. A difference may create one new strictly future queued transition under the target delay.

### 49.4 Cancellation and rejection

`CancelPending` reports every canceled transition as semantic loss.

`RejectIfPending` rejects when the actual queue is nonempty.

## 50. `InertialDelay` migration

The migration enum is:

```rust
#[non_exhaustive]
pub enum InertialDelayMigration {
    PreserveDeadline,
    RecomputeFromOrigin {
        overdue: OverdueMigrationPolicy,
    },
    RestartFromPatchTime,
    CancelCandidate,
    RejectIfCandidate,
}
```

The remembered input and current output level are preserved for same-kind survival unless reset is explicit.

### 50.1 Standard rule

When delay changes while no candidate exists, `Standard` preserves the empty candidate state and applies the target delay to future candidates.

When a candidate exists, `Standard` is insufficient. The patch must select one explicit `InertialDelayMigration` policy.

### 50.2 `PreserveDeadline`

The candidate retains:

- target level;
- qualification origin;
- deadline;
- causal ancestry.

The target delay applies only to future candidates.

### 50.3 `RecomputeFromOrigin`

The candidate deadline becomes:

```text
qualification_origin + target delay
```

The original qualification interval remains semantically credited.

### 50.4 `RestartFromPatchTime`

The candidate target remains unchanged, but:

```text
qualification_origin = T
deadline = T + target delay
```

The original input cause remains causal ancestry, and the patch records that qualification restarted.

Discarding elapsed qualification is semantic loss.

### 50.5 `CancelCandidate`

The candidate is canceled and reported as semantic loss.

The patch-time reaction may create a new candidate from settled target input according to the ordinary target node law.

### 50.6 `RejectIfCandidate`

Finalization rejects if a candidate exists.

## 51. `Periodic` migration

The migration enum is:

```rust
#[non_exhaustive]
pub enum PeriodicMigration {
    PreserveNextDeadline,
    RecomputeFromExistingAnchor,
    ReanchorAtPatchTime,
    CancelSchedule,
    RejectIfAnchored,
}
```

The state components are:

```text
optional phase anchor
previous enabled observation
optional next eligible boundary
```

### 51.1 Standard rule

If period, first-emission policy, and re-enable phase policy are unchanged, `Standard` preserves all temporal state and pending boundaries.

If any of those parameters changes while an anchor or schedule exists, one explicit `PeriodicMigration` policy is required.

If the node remains fresh and anchorless, the target parameters may replace the old parameters without additional policy.

### 51.2 `PreserveNextDeadline`

If a next eligible deadline exists:

- that deadline is preserved;
- a boundary due exactly at `T` joins the patch-time due batch;
- after that preserved boundary, cadence proceeds using the target period;
- the preserved deadline becomes the phase reference for subsequent target cadence.

If the node has an anchor but no currently scheduled boundary because it is disabled under preserved phase:

- the anchor is preserved;
- future target boundaries are derived from that anchor and target period;
- missed disabled boundaries are not replayed.

Previous enabled observation is preserved.

### 51.3 `RecomputeFromExistingAnchor`

The existing anchor is preserved.

The first target boundary at or after `T` is computed from:

```text
anchor + n * target period
```

subject to target first-emission and re-enable policies.

Boundaries before `T` are not replayed.

A target boundary exactly at `T` becomes a due-at-`T` fact whose emission still depends on settled current `enable` in the patch-time reaction.

Previous enabled observation is preserved.

### 51.4 `ReanchorAtPatchTime`

The target anchor becomes `T`.

- if settled target enable is `High` and target first-emission policy is `Immediate`, the patch-time reaction receives an eligible current boundary;
- if settled target enable is `High` and policy is `AfterFirstPeriod`, the first boundary is `T + target period`;
- if settled target enable is `Low`, no boundary emits at `T`;
- under `PreservePhase`, the new anchor remains available while disabled;
- under `RestartPhase`, a later disabled-to-enabled transition may establish a new anchor normally.

The prior phase and pending schedule are discarded and reported as semantic loss.

Previous enabled observation is preserved so the ordinary patch-time reaction can distinguish a real enable transition from mere reanchoring.

### 51.5 `CancelSchedule`

Old anchor, next boundary, and previous-enabled scheduling state are cleared.

The patch-time reaction treats the periodic node as fresh and applies target initialization semantics to the fully settled target `enable` level.

Thus an enabled node may immediately establish a new anchor or emit under `Immediate`.

Discarding the old schedule is semantic loss.

### 51.6 `RejectIfAnchored`

Finalization rejects if an anchor or pending boundary exists. An anchorless fresh node may accept the parameter change.

## 52. Cross-kind temporal migration

The initial core defines no lossless migration between different temporal node kinds.

Examples such as:

```text
TransportDelay -> InertialDelay
InertialDelay -> TransportDelay
PulseDelay -> Periodic
```

must either:

- use explicit `Reset`, with source temporal state and pending work reported as loss; or
- reject.

A future cross-kind migration may be added only as a closed, separately specified rule with complete state, event, deadline, provenance, and loss semantics.

---

# Part VI — Other semantic state

## 53. External input valuation migration

For a ready machine:

- a preserved external `Level` input retains its authoritative value;
- a reassociated external `Level` input receives the source authoritative value;
- a new external `Level` input requires explicit target input establishment;
- a removed external `Level` input loses its authoritative value;
- external `Pulse` inputs have no persistent valuation to migrate.

A same-time target input `set` or `establish` is not applied during structural migration. It becomes an authoritative source fact in the patch-time reaction.

The migration report distinguishes inherited valuation from same-time caller-supplied target input.

## 54. External output baseline migration

For a ready machine:

- a preserved `Level` output retains its prior published baseline;
- a reassociated same-kind `Level` output carries the source baseline as migration evidence and causal continuity, but the new structural output key begins without an externally published baseline;
- a new `Level` output has no baseline;
- a removed `Level` output loses its baseline;
- `Pulse` outputs have no persistent value baseline.

Transferred output-baseline evidence does not force target signal values.

A same-key preserved output uses its prior baseline for change comparison. A reassociated output has a new stable endpoint identity and therefore emits `LevelEstablished` after target settlement, even when its value equals the source endpoint’s prior baseline.

## 55. Diagnostic episode migration

Every active diagnostic episode receives one outcome:

```text
Preserve
Transform
Resolve
Terminate
Reject
```

An episode may be preserved only when:

- its owning subject survives or is reassociated;
- its diagnostic code remains applicable;
- its condition discriminator retains the same semantic identity;
- its evidence can be translated without ambiguity.

Examples:

- a preserved `LevelSetResetLatch` under unchanged `RetainAndDiagnose` may preserve its active conflict episode;
- changing conflict policy may resolve or terminate the episode;
- changing from a level latch to a pulse latch cannot preserve a continuous level-conflict episode;
- dense-slot reuse must never transfer an episode.

The target patch-time reaction reevaluates persistent conditions and may begin, materially change, or resolve episodes normally.

## 56. Provenance migration

Migration is represented causally.

For every migrated current fact, the target provenance must identify:

- the source fact or checkpoint;
- the topology patch;
- the migration rule;
- any transformation of value, deadline, owner, or identity;
- any reset or loss consequence.

Preserved state across a topology revision SHOULD receive a migration/checkpoint derivation rather than silently reusing a revision-incompatible fact root.

Reset state receives an explicit reset or target-initialization cause.

Preserved pending events retain their original scheduling ancestry plus migration provenance.

Recomputed or restarted events retain original originating causes and add the patch rule that changed timing.

Required provenance ancestry may be replaced by an authoritative checkpoint only where the general provenance-retention specification permits equivalent future explanation and replay.

## 57. Current diagnostic metadata and paths

Metadata and hierarchy changes may alter rendered diagnostic paths without changing the semantic identity of preserved runtime facts.

The result should expose both old and new paths where useful.

Diagnostic prose is not migration state and is regenerated from structured evidence.

---

# Part VII — Module replacement and hierarchy

## 58. Module expansion model

Module-instance operations are prepared by expanding module structure into the same stable-keyed semantic graph model used for direct nodes and connections.

Preparation MUST preserve module hierarchy for:

- diagnostics;
- graph views;
- explanation;
- stable correspondence;
- migration reporting;
- snapshot identity.

Execution may remain internally flattened.

## 59. Compatible module revision

A module-instance revision is compatible when every surviving internal subject can be classified under the ordinary rules.

Compatibility is not one all-or-nothing flag for the entire module. One replacement may contain:

- preserved internal nodes;
- migrated internal nodes;
- new internals;
- removed internals;
- changed connections;
- changed interface ports;
- conditional temporal outcomes.

The module-level static plan summarizes the complete internal plan.

## 60. Module interface changes

When a module interface changes:

- removed module inputs or outputs must have all external incidence explicitly removed or redirected;
- new required module inputs must be bound explicitly;
- signal-kind changes require remove/add rather than same-key replacement;
- internal interface mapping must use stable module interface keys;
- external connections are not inferred from names or positional order.

An unbound required target module input prevents preparation.

## 61. Nested modules

Nested module replacement applies correspondence recursively through:

```text
outer instance identity
nested instance identity
internal stable key
```

Containment must remain acyclic.

Moving a nested module instance without replacing its internal semantics preserves compatible internal state and pending work.

## 62. Module metadata versus semantics

Changing module diagnostic metadata or presentation path alone does not alter internal semantic fingerprint or migration compatibility.

Changing the validated module definition, parameters, bindings, or state-relevant hierarchy may alter the target fingerprint and requires complete structural preparation.

---

# Part VIII — Transaction-time finalization and commitment

## 63. Effective-time ordering

For a ready machine and patch effective at logical time `T`, the outer transaction MUST perform:

1. validate transaction, expected revision, optional execution digest, target input artifact, prepared patch, and runtime policy;
2. process every pending deadline strictly earlier than `T` under the old topology;
3. obtain the actual candidate machine state immediately before `T`;
4. finalize state, pending-work, input-valuation, output-baseline, provenance, and diagnostic migration;
5. enforce the selected reconfiguration state-loss policy;
6. install the target topology in candidate state;
7. form the complete patch-time reaction batch from:
   - migrated obligations due exactly at `T`;
   - migration-created due-at-`T` obligations explicitly allowed by policy;
   - external target input at `T`;
   - topology-induced source invalidations and initialization facts;
8. evaluate and settle the target current-reaction graph at `T`;
9. commit topology, state, events, outputs, provenance, diagnostics, revision, and result atomically.

Events strictly before `T` always execute under the old topology.

## 64. Initialization patch ordering

For an uninitialized machine and patch effective at initial time `T0`:

1. validate the complete target-bound `InputSnapshot`;
2. finalize migration from base declared initial state into target declared or migrated state;
3. enforce state-loss policy;
4. install the target topology in candidate state;
5. evaluate one ordinary initialization reaction at `T0` using:
   - migrated or target initial node state;
   - complete target external level snapshot;
   - target external pulse batch;
   - no previously pending due obligations;
6. commit initialization and topology revision atomically.

## 65. State-dependent finalization

Static conditional rules are evaluated against the state reached immediately before `T`.

Examples include:

- whether an inertial candidate exists;
- whether a pulse-delay queue is empty;
- whether a periodic anchor exists;
- whether a removed node owns stored state;
- whether an active diagnostic episode survives;
- whether a pending event is due exactly at `T`;
- whether reset or cancellation causes actual loss.

Preparation-time machine state MUST NOT be substituted.

## 66. Same-time external input boundary

Migration reads pre-`T` source state.

Same-time target input is applied as part of the target reaction, not retroactively as source migration state.

This means:

- a preserved external input valuation is inherited before the caller’s `set` at `T` is applied;
- a new external level value is established as a target reaction source;
- topology-induced edge detection compares migrated previous observation with the fully settled target input at `T`;
- a periodic due boundary at `T` is permitted or suppressed by fully settled target `enable` at `T`;
- temporal migration policy does not inspect arbitrary host input before the target reaction.

## 67. Events due exactly at patch time

Every source event due exactly at `T` is first subject to migration under the prepared rule.

Possible results include:

- preserved and fired at `T` under the target owner;
- transformed and fired at `T`;
- recomputed to a later deadline;
- canceled;
- rejected.

An event due at `T` MUST NOT fire under the old topology merely because it was already in the old calendar.

After migration, all surviving due-at-`T` facts join one unordered patch-time reaction batch.

## 68. Target topology installation

Target topology installation in candidate state includes:

- new compiled topology root;
- target revision;
- migrated state-family storage;
- migrated event ownership and index;
- target external input valuation layout;
- target output-baseline layout;
- migrated diagnostic episode ownership;
- migrated provenance roots;
- target graph and inspection indices.

Installation remains unpublished until complete transaction success.

## 69. Topology-induced reevaluation

After installation, every potentially affected target reaction operation must be reevaluated.

The initial implementation MAY conservatively evaluate the complete target reaction graph.

Any incremental invalidation algorithm must be equivalent to complete target evaluation.

Potentially affected operations include those whose:

- node definition changed;
- current predecessor changed;
- input driver changed;
- source subject was added, removed, or reassociated;
- previous state migrated or reset;
- due-event root migrated;
- external input was added or changed;
- module expansion changed;
- current explanation support changed.

## 70. Topology as cause

The patch is a first-class causal fact.

It may explain:

- a level output change with no external signal change;
- an edge pulse caused by changed connectivity against preserved previous observation;
- state reset;
- event retiming;
- event cancellation;
- phase reanchoring;
- diagnostic episode resolution;
- output establishment or removal.

The patch MUST NOT be encoded as a synthetic `Level` or `Pulse` input.

## 71. Output publication

After target settlement:

- same-key preserved `Level` outputs compare target value against their prior published baseline;
- changed preserved values emit `LevelChanged`;
- unchanged preserved values emit no level event;
- new and reassociated `Level` outputs emit `LevelEstablished`;
- removed outputs produce `TopologyChange` entries;
- `Pulse` outputs emit only for genuine nonzero target reaction pulse counts.

Output events remain one deterministic chronological stream.

## 72. Atomic failure

Any failure during:

- freshness validation;
- earlier-deadline execution;
- migration finalization;
- checked time arithmetic;
- state-loss enforcement;
- target installation;
- target reaction evaluation;
- provenance construction;
- diagnostic update;
- result construction;
- digest calculation;
- runtime-budget checking;

rejects the entire outer transaction.

The published machine remains exactly unchanged, including topology revision and logical time.

---

# Part IX — Semantic loss and migration reporting

## 73. Semantic-loss definition

Semantic loss is the destruction, reset, truncation, or unrepresented disappearance of semantic information that existed immediately before the patch and could affect future execution or required observation.

It includes, where applicable:

```text
removed stored node state
reset stored node state
removed temporal state
canceled pending events
lost pulse multiplicity or event payload
lost event deadline or origin information not represented by migration
lost elapsed inertial qualification
lost periodic phase or schedule
removed external level valuation
removed external level output baseline
terminated active diagnostic episode state
terminated required provenance ancestry
lossy many-fact coalescing
```

The list is extensible as new semantic state categories are introduced.

## 74. Changes that are not semantic loss

The following are not by themselves semantic loss:

- topology revision advancing;
- dense-index reassignment;
- stale resolved handles;
- stale compiled inspection plans;
- region merge or split;
- metadata change;
- deterministic memory-layout change;
- event deadline change under an information-preserving explicit migration rule;
- migration provenance introducing a checkpoint that preserves all required semantics;
- target output changes produced by ordinary reevaluation;
- introduction of new target initial state where no source state existed.

A semantic change may be substantial without being a loss when the patch explicitly transforms every prior obligation and state fact into a complete target counterpart.

## 75. Loss identity

Every potential and actual loss has stable structured identity based on at least:

```text
loss category
source structural subject
source state or event identity
migration rule or removal operation
```

Loss entries MUST NOT be deduplicated merely because rendered prose is equal.

Every canceled pending event remains individually accountable unless the source event itself was a semantically aggregated group.

## 76. Reconfiguration policy

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReconfigurationPolicy {
    RejectStateLoss,
    AllowReportedStateLoss,
}
```

Under `RejectStateLoss`:

- any nonempty finalized semantic-loss set rejects the complete transaction;
- potential loss alone does not reject if the runtime predicate is false;
- no topology revision is committed.

Under `AllowReportedStateLoss`:

- commitment is permitted only if every possible loss category was classified during preparation;
- every actual loss appears in the committed migration report;
- no unclassified loss may be discovered and silently accepted at finalization.

## 77. Migration report

A successful patch transaction returns one complete `MigrationReport<D>`.

Representative shape:

```rust
pub struct MigrationReport<D> {
    pub base_revision: NetworkRevision,
    pub target_revision: NetworkRevision,
    pub base_fingerprint: NetworkFingerprint,
    pub target_fingerprint: NetworkFingerprint,
    pub subject_outcomes: Vec<SubjectMigrationOutcome<D>>,
    pub state_outcomes: Vec<StateMigrationRecord<D>>,
    pub pending_event_outcomes: Vec<PendingEventMigrationRecord<D>>,
    pub input_outcomes: Vec<ExternalInputMigrationRecord>,
    pub output_outcomes: Vec<ExternalOutputMigrationRecord>,
    pub diagnostic_outcomes: Vec<DiagnosticEpisodeMigrationRecord<D>>,
    pub provenance_outcomes: Vec<ProvenanceMigrationRecord>,
    pub losses: Vec<SemanticLoss<D>>,
    pub invalidated_artifacts: Vec<ArtifactInvalidation>,
    pub region_changes: Vec<RegionChange>,
}
```

The report MUST be complete enough to verify total classification without inspecting private machine internals.

## 78. Subject outcomes

Every base and target structural subject receives one canonical structural outcome:

```text
Preserved
Reassociated
Removed
Added
ReplacedInPlace
MetadataChanged
HierarchyChanged
```

A subject may have one primary existence outcome plus orthogonal metadata or hierarchy flags in the detailed record.

The representation must avoid contradictory duplicate primary outcomes.

## 79. Deterministic report order

Migration records are ordered deterministically by:

1. semantic category;
2. stable source identity where present;
3. stable target identity where present;
4. pending event identity or state component;
5. rule-specific canonical tie-breaker.

Ordering is representational and MUST NOT imply temporal causality among simultaneous migration facts.

---

# Part X — Public API responsibilities

## 80. Patch access

`NetworkPatch<D>` is an owned opaque value.

It SHOULD expose:

```rust
impl<D> NetworkPatch<D> {
    pub fn network_key(&self) -> NetworkKey;
    pub fn base_revision(&self) -> NetworkRevision;
    pub fn base_fingerprint(&self) -> NetworkFingerprint;
    pub fn operations(&self) -> PatchOperationIter<'_, D>;
}
```

The iterator exposes canonical normalized operations, not builder insertion order.

## 81. Prepared-patch access

`PreparedPatch<D>` exposes:

```rust
impl<D> PreparedPatch<D> {
    pub fn base_revision(&self) -> NetworkRevision;
    pub fn proposed_revision(&self) -> NetworkRevision;
    pub fn base_fingerprint(&self) -> NetworkFingerprint;
    pub fn resulting_fingerprint(&self) -> NetworkFingerprint;
    pub fn resulting_compiled(&self) -> &CompiledNetwork<D>;
    pub fn static_plan(&self) -> &StaticMigrationPlan<D>;
    pub fn input_snapshot(&self) -> InputSnapshotBuilder<D>;
    pub fn input_delta(&self) -> InputDeltaBuilder<D>;
}
```

## 82. Patch builder responsibilities

The builder MUST provide methods for every canonical operation family.

Representative methods include:

```rust
pub fn add_node(self, node: NodeDef<D>)
    -> Result<Self, PatchBuildFailure>;

pub fn remove_node(self, node: NodeKey)
    -> Result<Self, PatchBuildFailure>;

pub fn replace_node(
    self,
    node: NodeKey,
    replacement: NodeDef<D>,
    migration: NodeMigrationDirective<D>,
) -> Result<Self, PatchBuildFailure>;

pub fn add_connection(self, connection: ConnectionDef)
    -> Result<Self, PatchBuildFailure>;

pub fn replace_external_output(
    self,
    output: AnyExternalOutputKey,
    replacement: ExternalOutputDef,
) -> Result<Self, PatchBuildFailure>;

pub fn reassociate(self, mapping: SubjectReassociation<D>)
    -> Result<Self, PatchBuildFailure>;

pub fn finish(self) -> NetworkPatch<D>;
```

Exact ergonomic method grouping may differ.

## 83. Preparation report

Patch preparation returns:

```rust
Report<PreparedPatch<D>>
```

Independent structural diagnostics SHOULD be collected where safe.

A blocking diagnostic means no prepared artifact is returned.

## 84. Transaction attachment

A prepared patch commits only through a transaction:

```rust
let tx = Transaction::advance(
    at,
    machine.revision(),
    prepared.input_delta()
        .establish(new_input, LogicLevel::High)?
        .finish()?,
)
.with_patch(
    prepared,
    ReconfigurationPolicy::RejectStateLoss,
)?;
```

`with_patch` MUST validate where determinable:

- base revision binding;
- target input-schema binding;
- lifecycle-compatible snapshot versus delta;
- network identity;
- time domain.

## 85. Exact forecast

An exact future patch preview uses ordinary `Machine::forecast` with a patch-bearing transaction.

The result is bound to:

```text
base revision
base ExecutionStateDigest
effective time
RuntimePolicyId
```

Structural preparation alone MUST NOT claim exact state values, losses, output events, or final migration records.

## 86. No implicit commit of forecast

A forecast is not a reservation or prepared commit token.

Committing later requires a new explicit transaction against the then-current revision and, where desired, expected execution-state digest.

---

# Part XI — Diagnostics and failures

## 87. Patch-build failures

Patch construction must distinguish at least:

```text
foreign network artifact
duplicate conflicting operation
invalid replacement key
conflicting subject edit
invalid reassociation shape
contradictory hierarchy assignment
```

These are local construction failures, not runtime failures.

## 88. Preparation diagnostics

Preparation diagnostics must cover at least:

```text
base fingerprint mismatch
base revision mismatch where machine convenience is used
unknown base subject
duplicate target key
dangling connection or endpoint
kind or direction mismatch
unsupported multiple driver
missing fixed port
invalid variadic arity
invalid module binding
hierarchy cycle
current-reaction cycle
incompatible migration directive
incomplete temporal migration policy
non-injective reassociation
conditional semantic loss
unavoidable semantic loss
invalid target input schema
empty effective patch
```

Diagnostics should identify stable subjects and graph paths where available.

## 89. Finalization failures

Runtime reconfiguration failures must distinguish at least:

```rust
#[non_exhaustive]
pub enum ReconfigurationFailure {
    StalePreparedPatch,
    TargetInputSchemaMismatch,
    StateMigrationRejected,
    PendingEventMigrationRejected,
    StateLossRejected,
    TimeArithmeticFailure,
    MigrationBudgetExceeded,
}
```

Exact nested evidence types are defined by the diagnostic and failure catalogue.

Internal invariant violations are processor defects and are not ordinary caller-facing `ReconfigurationFailure` values.

## 90. Diagnostic accumulation

Preparation SHOULD continue after one independent error where safe so that callers receive a useful complete report.

Finalization may fail fast once the complete transaction must reject, but it SHOULD retain structured evidence sufficient to identify the exact state predicate or event that caused rejection.

## 91. Diagnostic episode behavior

Preparation warnings are occurrences, not machine diagnostic episodes.

A patch-time target condition may begin, transform, or resolve persistent runtime episodes only if the patch transaction commits.

Rejected patch transactions do not modify active episode state.

---

# Part XII — Verification obligations

## 92. Reference reconfiguration path

The verification suite must retain an independent correctness-oriented reference path that:

1. applies the normalized rewrite to the complete base definition;
2. validates and compiles the complete target from scratch;
3. derives stable correspondence independently of dense layout;
4. applies explicit migration laws to stable-keyed semantic state;
5. classifies every actual pending event;
6. performs complete target reaction evaluation;
7. compares the full semantic result with the production prepared-migration path.

## 93. Operation-order invariance

Tests must permute equivalent operation insertion order and compare:

- normalized patch;
- target fingerprint;
- preparation diagnostics;
- static migration plan;
- final committed machine and result.

## 94. Preparation state independence

For one base topology and patch, preparation performed against machines with different runtime states but the same topology revision must produce equivalent prepared artifacts.

The machine convenience may differ only by stale-revision acceptance.

## 95. Effective-time state correctness

A required scenario is:

1. prepare a patch;
2. execute state-only transactions and earlier temporal deadlines;
3. apply the patch later without an intervening topology revision;
4. verify migration uses the state actually reached immediately before the effective time.

## 96. Classification totality

For every successful finalization:

- every surviving state-owning subject has exactly one state outcome;
- every source pending event has exactly one event outcome;
- every external level valuation has one preservation, reassociation, removal, or addition outcome;
- every external level output baseline has one outcome;
- every active diagnostic episode has one outcome;
- every required provenance root has one outcome;
- every actual semantic loss appears exactly once.

## 97. State-loss policy tests

Tests must establish:

- any actual loss rejects under `RejectStateLoss`;
- potential but unrealized loss does not reject;
- every actual loss is reported under `AllowReportedStateLoss`;
- dense-handle invalidation is not mislabeled as state loss;
- reset counts as loss only where source semantic state or progress is discarded;
- initializing state where no source state existed is not loss.

## 98. Temporal boundary tests

Every temporal migration policy must be tested for source deadlines:

```text
strictly before T
exactly at T
strictly after T
```

Where recomputation is supported, tests must additionally cover target deadlines:

```text
less than T
equal to T
greater than T
overflow
```

Equal-time event order must be permuted without changing semantics.

## 99. Target input-schema tests

Tests must cover:

- preserved external level input inherited;
- preserved input changed with `set`;
- reassociated input valuation transferred;
- new level input established;
- missing establishment rejected;
- removed input reference rejected;
- pulse input absence interpreted as zero;
- initialization patch requiring complete target snapshot.

## 100. Topology-induced reaction equivalence

The production invalidation set after patch installation must be differentially compared with complete target-graph evaluation.

Required cases include:

- connection redirect changing a level output;
- preserved output unchanged;
- new output establishment;
- removed output consequence;
- edge detector pulse caused by changed input path;
- no synthetic pulse from topology change alone;
- stateful chain under migrated previous state;
- due-at-patch-time temporal events.

## 101. Module migration tests

Generated and focused tests must cover:

- module internal node preservation;
- internal key removal and addition;
- module interface addition and removal;
- nested instance replacement;
- hierarchy move without state reset;
- pending event preservation inside replaced module;
- state loss rejection inside one affected instance rejecting the entire patch.

## 102. Failure atomicity

Every preparation-time or runtime rejection category must preserve the complete published machine.

Fault injection should cover:

- migration allocation;
- event transformation;
- target topology installation;
- target reaction evaluation;
- provenance migration;
- diagnostic episode migration;
- migration report construction;
- digest computation;
- runtime budget checks.

## 103. Stable identity and dense reuse

Tests must verify:

- equal stable keys preserve identity only when category and kind permit;
- explicit reassociation is one-to-one;
- old reassociated keys become unavailable;
- dense-slot reuse cannot transfer state or episodes to another subject;
- stale handles and plans fail structurally;
- stable keys can be resolved again against the target revision.

## 104. Bounded exhaustive exploration

For small generated networks and patches, the harness SHOULD enumerate:

- all small valid operation subsets;
- all operation orders;
- small state valuations;
- empty and nonempty pending-event sets;
- both state-loss policies;
- relevant temporal migration choices.

The exploration should verify deterministic target construction and complete migration classification.

---

# Part XIII — Deliberately unspecified and excluded behavior

## 105. Internal representation freedom

This specification does not mandate:

- patch storage container;
- persistent versus copied target graph construction;
- dense state-layout migration algorithm;
- event-arena implementation;
- incremental compilation strategy;
- cache invalidation representation;
- allocation strategy.

Any implementation must refine the same declarative rewrite and migration semantics.

## 106. No arbitrary migration code

The initial core does not accept caller-defined migration functions.

This avoids making execution depend on:

- host callbacks;
- non-deterministic code;
- hidden external state;
- panic behavior;
- uninspectable state transformation;
- unserializable migration semantics.

New migration capabilities must enter as closed, versioned semantic operations.

## 107. No automatic structural matching

The core does not infer continuity from:

- names;
- descriptions;
- diagnostic paths;
- node position;
- graph similarity;
- connection neighborhood;
- insertion order;
- dense index;
- module display order.

Tooling may propose a patch, but the resulting correspondence must be explicit and validated.

## 108. No general patch merge

Collaborative editor merge, conflict-free replicated patches, three-way graph merge, and optimistic patch rebasing are outside this specification.

Such tooling may produce one ordinary explicit patch after resolving conflicts externally.

## 109. No automatic inverse patch

A structural inverse may be derivable for some patches, but semantic rollback is not generally automatic because the original transaction may have:

- discarded state;
- canceled events;
- advanced logical time;
- emitted outputs;
- transformed provenance;
- changed external authoritative input.

Rollback uses snapshots, replay, or a new explicit patch and migration policy.

## 110. Persistence boundary

This specification defines the semantic content of patch and migration artifacts but not their canonical wire encoding.

The persistence specification must encode every operation, directive, stable identity, version binding, and report field required here.

---

# Part XIV — Required guarantees

## 111. Patch-language guarantees

The implementation MUST provide:

```text
owned explicit patch values
a complete canonical operation language
operation-order-independent semantics
base network, fingerprint, and revision binding
one-to-one stable correspondence
explicit reassociation
no silent cascading deletion
complete target validation and compilation
state-independent structural preparation
state-dependent effective-time finalization
target-bound input artifacts
complete state classification
complete pending-event classification
closed temporal migration policies
explicit semantic-loss enforcement
atomic target reaction and publication
structured migration provenance
complete migration reports
deterministic diagnostics and ordering
reference-path differential verification
```

## 112. Coherence with the other specifications

This document refines the existing shared model:

- the API and semantics specification remains authoritative for observable machine behavior;
- the built-in node specification remains authoritative for ordinary node laws and standard compatibility;
- the processor architecture remains authoritative for internal transaction ordering and atomic publication;
- the concrete Rust API surface remains authoritative for general ownership and type organization, except where it explicitly deferred the exhaustive patch language to this document;
- the testing and verification policy remains authoritative for CI and release gates.

Where this specification defines the previously deferred exhaustive patch-operation and migration-policy language, this specification is authoritative for that language.

---

# Summary

A `mossignal` topology patch is a declarative, stable-keyed graph rewrite rather than an ordered mutation script.

The complete patch is normalized and applied structurally to produce one validated target topology. Stable-key equality and explicit one-to-one reassociation establish migration correspondence. Structural preparation compiles the target and builds a reusable migration program without reading machine state.

At the effective logical time, the machine first processes earlier deadlines under the old topology. It then finalizes migration against the state actually reached, classifies every state component and pending event, enforces the caller’s state-loss policy, installs the target topology in candidate state, evaluates one complete target reaction, and publishes everything atomically.

No node state, pending event, external valuation, output baseline, diagnostic episode, or required provenance fact may disappear silently. Every loss is either rejected or explicitly reported. Topology change is a causal fact, never a synthetic signal.
