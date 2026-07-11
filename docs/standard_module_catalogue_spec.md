# `mossignal` Standard Module Catalogue

**Status:** Design specification, version 2  
**Defines:** The standard-module system; classification of conveniences; canonical catalogue identity and expansion; the complete initial standard-module inventory; typed and dynamic construction APIs; inspection, explanation, diagnostics, persistence, reconfiguration, migration, and verification requirements  
**Does not define:** New primitive node kinds, new signal kinds, the exhaustive global diagnostic catalogue, unrestricted user-defined automata, editor interaction design, application-domain modules, or future temporal capabilities absent from the current primitive language

---

## 1. Purpose

This specification defines the named, versioned, reusable behaviors supplied by the `mossignal` standard module catalogue.

A primitive is part of the evaluator's closed semantic language. A standard module is a canonical composition of primitives with:

- a stable machine-readable identity;
- a stable typed public interface;
- a normative primitive expansion;
- visible module-instance identity and hierarchy;
- public behavioral, inspection, and explanation semantics;
- explicit persistence and compatibility rules;
- explicit state and pending-work migration rules;
- reusable conformance obligations.

The central distinction is:

> A primitive defines an evaluator operation. A standard module defines a durable public abstraction over an exact stable-keyed primitive graph.

A standard module does not bypass ordinary primitive semantics. Its behavior is obtained by evaluating its canonical expansion through the same validation, compilation, reaction, temporal, transaction, provenance, persistence, and reconfiguration machinery used by any other module.

This specification has two responsibilities:

1. define the standard-module mechanism as a versioned semantic system;
2. apply that mechanism to a deliberately small initial catalogue.

The initial catalogue contains only modules whose behavior is precise and whose canonical expansion is expressible without hidden feedback, ad hoc runtime machinery, or unresolved temporal policy.

## 2. Normative language

The terms **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.

A **standard module descriptor** is the authoritative catalogue record for one exact standard module semantic and expansion version.

A **standard module declaration** selects one descriptor, supplies its parameters and public variadic-port identities, and thereby determines one canonical `ModuleDef<D>`.

A **canonical expansion** is the exact stable-keyed primitive graph generated from one standard module declaration.

A **module role** is a permanent symbolic identity assigned to one public port, internal node, internal port, connection, export, aggregate inspection field, or migration subject.

## 3. Relationship to the other specifications

The API and semantics specification remains authoritative for network, machine, transaction, inspection, diagnostic, and lifecycle semantics.

The built-in-node specification remains authoritative for every primitive used in a canonical expansion.

The processor specification remains authoritative for compilation, evaluation, temporal execution, atomicity, provenance, and flattened runtime representation.

The concrete Rust API specification remains authoritative for the general `ModuleDef`, `ModuleBuilder`, `ModuleInstanceKey`, `ModuleInputKey<S>`, `ModuleOutputKey<S>`, and `AddedModuleInstance` model. This specification extends that surface with standard catalogue identities, descriptors, and construction conveniences.

The topology-patch specification remains authoritative for graph rewrite, correspondence, state loss, pending-work migration, and patch-time reaction semantics. This specification provides the standard rules and internal correspondence for standard modules.

The persistence specification remains authoritative for canonical encoding, artifact envelopes, fingerprints, digests, restoration, and replay. This specification defines the additional standard-module fields and validation required inside module and network definitions.

The testing and verification policy remains authoritative for the overall verification strategy. This specification defines the conformance obligations particular to standard modules.

---

# Part I — Boundaries and classification

## 4. Closed semantic boundary

The standard catalogue introduces no evaluator extension mechanism.

A standard module MUST be composed entirely from:

- current built-in primitives; or
- other exact standard module descriptors whose identities and versions are included in the expansion.

The initial catalogue uses primitives only.

A standard module MUST NOT:

- introduce a new signal kind;
- introduce a hidden primitive node kind;
- contain an arbitrary callback or trait-object evaluator;
- consult host state;
- own a wall clock;
- emit host effects during propagation;
- use names or metadata as behavior;
- bypass transaction atomicity;
- bypass ordinary current-reaction dependency validation;
- use unspecified same-time microsteps or fixed-point iteration;
- erase pulse multiplicity unless an explicit primitive does so;
- hide state or pending work from inspection, snapshots, migration, or replay.

## 5. Exhaustive convenience classification

Every named convenience supplied by the crate MUST belong to exactly one of these categories.

### 5.1 Primitive alias

A primitive alias maps exactly to one existing primitive node kind and parameterization. The parameterization MAY be derived solely from the convenience arguments or variadic arity before that one primitive is created.

Examples:

```text
xor(a, b)         -> Parity([a, b])
debounce(x, span) -> InertialDelay(x, span)
```

A primitive alias:

- creates one primitive;
- creates no `ModuleInstance`;
- exposes the canonical primitive kind in inspection and persistence;
- uses the primitive's compatibility and diagnostic rules;
- may retain the alias spelling only as non-semantic authoring metadata.

### 5.2 Standard module

A standard module expands into a canonical graph containing more than one semantically relevant primitive operation, or otherwise provides a durable abstraction whose visible module boundary materially improves public semantics, explanation, inspection, persistence, or migration.

A standard module:

- creates a visible `ModuleInstance`;
- retains a standard catalogue identity;
- retains hierarchy even if execution is flattened;
- is persisted and reconfigured as a module instance;
- exposes module-level explanations and inspection in addition to primitive-level drill-down.

### 5.3 Metadata-only convenience

A metadata-only convenience annotates an existing semantic subject without changing signal behavior.

`annotate_signal` is metadata-only.

It creates no node, connection, module, causality barrier, runtime state, or fingerprint contribution beyond metadata scopes explicitly defined elsewhere.

### 5.4 Builder-only operation

A builder-only operation deterministically authors ordinary primitives but leaves no semantic object representing the operation itself.

A builder-only operation:

- creates the ordinary primitive nodes and connections in its expansion;
- creates no `ModuleInstance`;
- is inspected and persisted only as those primitives;
- has no independent state, fingerprint, version, migration boundary, or diagnostic subject;
- MAY allocate internal keys locally for concise authoring;
- SHOULD NOT provide a misleading single keyed form.

A caller requiring durable identity for the composition SHOULD author the primitives explicitly or wrap them in a user-defined `ModuleDef`.

## 6. Admission criteria

A behavior belongs in the standard catalogue only when it is:

- domain-neutral;
- broadly reusable;
- semantically complete;
- expressible through the supported semantic language;
- meaningfully clearer as a named public unit;
- valuable as an inspection or reconfiguration boundary;
- capable of a durable typed interface;
- capable of canonical stable internal identity;
- testable independently of application assumptions.

A behavior SHOULD remain a primitive alias, builder-only operation, or user-defined module when it:

- is merely a spelling difference;
- saves only one obvious expression step;
- would create a large polarity or priority family;
- has unresolved simultaneous-event or temporal policy;
- requires hidden state not naturally represented by its public law;
- requires a capability absent from the primitive language;
- has application-domain meaning;
- obscures the primitive behavior rather than clarifying it.

## 7. Explicit non-goals

The initial catalogue does not introduce:

```text
generic timers
counters
general finite-state machines
calendar sources
absolute-time sources
sinks
probes
assertions
payload transport
application actions
application entities
arbitrary callback-defined modules
```

User-authored `ModuleDef<D>` values remain supported and are not part of the standard namespace merely because they happen to match a standard module's behavior.

---

# Part II — Catalogue identity and canonical descriptors

## 8. Standard catalogue version

The initial catalogue version is:

```text
StandardCatalogueVersion(1)
```

The catalogue version identifies one released set of descriptors and discovery metadata.

Adding, deprecating, or changing a descriptor increments the catalogue version.

The catalogue version itself MUST NOT contribute to an individual module or network fingerprint. Adding an unrelated module to a later catalogue must not alter existing network identity.

Individual identity is determined by the selected module descriptor.

## 9. Standard module identifiers

Each standard module has one permanent machine-readable identifier.

Identifier syntax is:

```text
mossignal.standard.<segment>[.<segment>...]
```

Each segment MUST match:

```text
[a-z][a-z0-9_]*
```

Identifiers:

- are lowercase ASCII;
- are never localized;
- are independent of Rust type or function names;
- are independent of display names;
- are permanent once released;
- are not reassigned to another meaning.

The `mossignal.standard` namespace is reserved by the core crate. Third-party and user-defined modules MUST NOT claim identifiers in that namespace.

## 10. Descriptor versions

Each descriptor carries two independent versions:

```rust
pub struct StandardModuleSemanticVersion(pub u32);
pub struct StandardModuleExpansionVersion(pub u32);
```

Numeric adjacency implies no compatibility. Compatibility is defined by an explicit table.

### 10.1 Semantic version

The semantic version changes when any of these change materially:

- public input or output meaning;
- parameter meaning or valid domain;
- current output law;
- successor-state law;
- simultaneous-input law;
- pulse multiplicity law;
- current-reaction dependency signature;
- initialization law;
- exact-deadline or scheduling behavior;
- public explanation meaning where structurally authoritative;
- diagnostic condition or episode identity;
- public migration contract.

Documentation wording, display text, examples, and non-semantic metadata MUST NOT change semantic version.

### 10.2 Expansion version

The expansion version changes when any of these change:

- canonical internal primitive kinds;
- canonical internal connections;
- internal stable role assignment;
- internal state schema or ownership;
- internal pending-event ownership;
- canonical export wiring;
- nested standard-module versions;
- expansion fingerprint projection.

An expansion change may preserve public behavior while still requiring migration because internal identity, state, pending work, explanations, or diagnostics are observable.

### 10.3 Initial versions

Every module in catalogue version 1 has:

```text
semantic_version = 1
expansion_version = 1
```

## 11. Descriptor reference

The exact standard identity of a module definition is:

```rust
pub struct StandardModuleRef {
    pub id: StandardModuleId,
    pub semantic_version: StandardModuleSemanticVersion,
    pub expansion_version: StandardModuleExpansionVersion,
}
```

A display name or Rust constructor name is not part of identity.

Aliases resolve to one exact `StandardModuleRef` before construction. The alias itself may be retained as presentation metadata but does not affect fingerprints.

## 12. Authoritative descriptor

A standard module descriptor contains at least:

```text
standard module reference
catalogue version introduced
availability and deprecation state
display name and documentation
classification
public typed port schema
parameter schema
public behavioral law
public dependency signature
initialization law
inspection schema
explanation schema
diagnostic conditions
canonical expansion algorithm
internal role schema
migration compatibility table
persistence requirements
conformance vectors
```

The descriptor is authoritative. Handwritten Rust constructors, generated documentation, editor palettes, persistence validation, and conformance tests MUST derive from or be checked against the same descriptor source.

A descriptor MUST be immutable after publication under one exact `StandardModuleRef`.

## 13. Standard module origin

`ModuleDef<D>` is extended with an origin classification broadly equivalent to:

```rust
#[non_exhaustive]
pub enum ModuleOrigin<D> {
    User,
    Standard(StandardModuleDeclaration<D>),
}
```

A standard declaration contains:

```text
StandardModuleRef
canonical parameter values
public variadic-port stable keys
canonical expansion fingerprint
```

Fixed public interface keys are supplied by the descriptor and need not be repeated as caller choices, although they remain present in the expanded `ModuleDef<D>`.

A user-defined module with an identical primitive graph remains `ModuleOrigin::User` and has a distinct fingerprint because standard origin identity is semantic.

## 14. Parameter representation

Strongly typed constructors use module-specific configuration values.

Dynamic construction uses a closed parameter value family broadly equivalent to:

```rust
#[non_exhaustive]
pub enum StandardParameterValue<D> {
    LogicLevel(LogicLevel),
    U64(u64),
    Span(Span<D>),
    NonZeroSpan(NonZeroSpan<D>),
    Enum(StandardEnumValue),
}
```

The initial catalogue uses only `LogicLevel` and `U64`.

A parameter schema defines:

- stable parameter key;
- value kind;
- whether it is required;
- valid range;
- default, normally none;
- fingerprint participation;
- migration meaning.

The initial standard modules have no implicit semantic defaults. Typed convenience signatures require every semantic parameter directly.

## 15. Public port identity

Every fixed public port has one catalogue-defined stable role and one deterministic `ModuleInputKey<S>` or `ModuleOutputKey<S>`.

Fixed public keys are derived under:

```text
mossignal/standard_module_public_key/v1
```

from the standard module identifier, direction, signal kind, and permanent public role. They are independent of module instance identity and parameters.

Every variadic public input slot has a caller-visible stable `ModuleInputKey<S>` independent of display order.

For variadic modules:

- input order is non-semantic unless a descriptor explicitly says otherwise;
- the canonical semantic order is stable-key order;
- duplicate upstream sources remain meaningful and are counted once per public port;
- reordering ports does not alter behavior or fingerprint;
- adding or removing a port changes interface structure but preserves surviving port identity.

Concise builder methods MAY allocate variadic public keys locally in iterator order. Callers requiring durable reconstruction or state-preserving independent rebuilds SHOULD use explicit keyed forms.

## 16. Canonical internal expansion

For one declaration, the descriptor determines exactly one stable-keyed primitive graph.

Conceptually:

```text
CanonicalExpand(
    StandardModuleRef,
    parameters,
    public interface keys
) -> ModuleDef
```

The result determines:

- primitive nodes and configurations;
- typed primitive ports;
- connections;
- module exports;
- nested hierarchy;
- internal role metadata;
- public dependency signature;
- module fingerprint projection.

Implementations MUST NOT substitute another behaviorally equivalent graph.

Two equivalent Boolean formulas may differ in:

- internal stable identity;
- primitive diagnostics;
- explanation paths;
- provenance structure;
- state ownership;
- pending-event ownership;
- migration correspondence;
- module and network fingerprints.

Those differences make expansion choice observable.

## 17. Internal stable-key derivation

The logical network identity of an internal subject remains:

```text
(ModuleInstanceKey, module-internal stable key)
```

The module-local stable key for a generated subject is derived from:

```text
standard module id
internal-key derivation version
subject category and signal kind
permanent role string
optional qualifying public port key
```

It MUST NOT depend on:

- module instance key;
- dense runtime position;
- construction order;
- hash iteration order;
- display name;
- diagnostic metadata;
- ordinary parameter values unless the parameter creates a genuinely distinct role;
- catalogue version.

The initial derivation version is `1` and fixes BLAKE3-256 as its derivation primitive independently of any later persistence digest-suite change, using the domain:

```text
mossignal/standard_module_internal_key/v1
```

The canonical record is hashed and the first 16 bytes form the opaque 128-bit key payload. Subject category and signal kind provide domain separation between typed key families.

Generated keys MUST be checked for uniqueness within the module definition. A collision is an internal catalogue invariant failure, not a recoverable authoring ambiguity.

Permanent role strings MUST NOT be repurposed. When a later expansion assigns materially different meaning, it must use a new role or an explicit version migration rule.

Primitive fixed-port keys are derived from:

```text
internal node role
primitive semantic port role
port direction and signal kind
```

A canonical internal connection role is derived from the complete semantic incidence:

```text
source role and source port role
target role and target port role
optional qualifying public variadic-port key
```

Module-export identities are derived from the public output role and its canonical source incidence. Thus every node, port, connection, and export shown in a descriptor expansion has one deterministic key even when the pseudocode does not print the opaque 128-bit value.

## 18. Variadic internal identity

When one internal node has one corresponding input port for each public variadic input, its internal port key is qualified by the public `ModuleInputKey<S>`.

For example:

```text
role = "at_least_lower.input"
qualifier = public variadic ModuleInputKey<Level>
```

This preserves internal correspondence when public inputs are reordered and gives added or removed public inputs exact internal incidence identity.

Connection keys involving one variadic public input use the same qualifier.

## 19. Canonical expansion persistence

The canonical persisted representation contains both:

1. the standard module declaration; and
2. the complete expanded ordinary module structure.

On decoding or dynamic validation, the implementation MUST:

1. resolve the exact `StandardModuleRef`;
2. validate parameters and public interface keys;
3. regenerate the canonical expansion;
4. compare the persisted semantic expansion with the regenerated expansion;
5. reject any mismatch before validation or compilation can succeed.

This rule makes persisted structure self-describing while preventing an artifact from claiming a standard identity for a modified graph.

Diagnostic metadata overlays MAY differ and remain presentation-only where the persistence specification permits. Semantic nodes, ports, connections, parameters, roles, and exports MUST match exactly.

If the required descriptor is unavailable, the artifact MAY be exposed as an unchecked persistence value for diagnostics, but it MUST NOT become a validated module, compiled network, restored machine, or replay input.

## 20. Standard-module encapsulation

Semantic internals of a standard module are canonical and are not individually authorable.

A patch MUST NOT add, remove, replace, reconnect, or semantically reconfigure an internal standard-module subject while retaining the same standard declaration.

Permitted operations are:

- replace the whole module instance with another canonical declaration;
- change instance bindings;
- change hierarchy;
- change presentation metadata where allowed;
- replace the instance with a user-defined module and thereby end standard identity.

A caller wishing to customize a standard expansion must materialize it as a user-defined module with `ModuleOrigin::User` and a new module fingerprint.

## 21. Fingerprints

The standard-module system defines:

```rust
pub struct StandardModuleExpansionFingerprint(/* 32-byte digest */);
```

It uses the domain:

```text
mossignal/standard_module_expansion_fingerprint/v1
```

and covers the exact canonical semantic expansion, including module-local nodes, typed ports, connections, exports, internal role keys, parameters that select the expansion, public interface keys, semantic and expansion versions, and nested descriptor references. It excludes standard-origin display metadata and instance placement.

A standard `ModuleFingerprint` includes:

```text
TimeDomainId where applicable
StandardModuleId
semantic version
expansion version
canonical parameter values
fixed and variadic public interface keys
canonical public port roles
canonical semantic expansion
internal stable keys and roles
nested standard module references, if any
applicable semantic version components
```

It excludes:

```text
module instance key
instance placement
instance bindings
instance metadata
diagnostic wording
display names
dense indices
construction order
private caches
```

The containing `NetworkFingerprint` includes the module fingerprint, module instance key, bindings, and state-relevant hierarchy according to the persistence specification.

The execution-state and observable-state digests do not repeat standard descriptor data separately because the network fingerprint already fixes the module definition. They include all internal state, pending work, diagnostic episodes, and provenance required by their ordinary scopes.

---

# Part III — Initial catalogue and non-module conveniences

## 22. Complete catalogue version 1 inventory

Catalogue version 1 contains exactly these standard modules:

```text
mossignal.standard.exactly
mossignal.standard.at_most
mossignal.standard.all_equal
mossignal.standard.pulse_resettable_toggle
mossignal.standard.level_resettable_toggle
mossignal.standard.level_resettable_sample_hold
```

Each has semantic version `1` and expansion version `1`.

No temporal standard module is included in catalogue version 1.

## 23. Primitive aliases in version 1

The initial primitive aliases are:

| Convenience | Canonical primitive |
|---|---|
| `xor(a, b)` | `Parity([a, b])` |
| `debounce(input, delay, initial)` | `InertialDelay` with the supplied configuration |
| `level_gate(value, enable)` | `All([value, enable])` |
| `majority(inputs)` | `AtLeast(floor(arity / 2) + 1, inputs)` |

`debounce` MUST expose every semantic `InertialDelay` parameter. It MUST NOT invent an initial level or migration policy.

## 24. Builder-only operations in version 1

The initial builder-only operation set is:

```text
nand
nor
xnor
any_pulse
```

The typed builders SHOULD expose concise methods for this set. Their canonical authoring expansions are:

```text
nand(inputs)     = Not(All(inputs))
nor(inputs)      = Not(Any(inputs))
xnor(a, b)       = Not(Parity([a, b]))
any_pulse(inputs) = Coalesce(Merge(inputs))
```

Low-enabled gate variants are not separately named in version 1. They use explicit `Not` plus `level_gate` or `PulseGate`.

`majority([]) = Low` because the alias computes a strict-majority threshold of one and constructs `AtLeast(1, [])`.

These operations create no module instance. Their individual primitive nodes receive ordinary builder-allocated keys unless the caller authors them explicitly.

The crate SHOULD avoid adding separate named conveniences for every inversion, polarity, priority, or branch permutation.

## 25. Metadata-only operations in version 1

`annotate_signal` and equivalent metadata helpers remain metadata-only.

No identity-buffer or naming node is introduced.

---

# Part IV — Shared standard-module semantics

## 26. Public behavioral law and expansion equivalence

Every standard module has a direct public reference law independent of the internal implementation.

For every valid declaration, reachable previous state, current input batch, and due-obligation batch:

```text
PublicReferenceLaw
    ==
CanonicalPrimitiveExpansion
```

Comparison includes:

- current outputs;
- successor state;
- future work;
- public diagnostics;
- public dependency signature;
- normalized provenance and explanation;
- migration-relevant state.

The public law is authoritative for user understanding. The canonical expansion is authoritative for exact internal identity and execution structure.

## 27. Public current-reaction dependency signature

A descriptor states the conservative current public input-to-output dependency relation.

The relation MUST equal the transitive public boundary dependency induced by the canonical primitive reaction graph.

Parameter- or arity-dependent constant-result forms MAY omit dependencies that cannot exist for that exact declaration.

The public signature is used by:

- documentation;
- cycle diagnostics crossing a module boundary;
- graph views;
- generated tests;
- editor previews.

Compilation still validates the expanded primitive reaction graph.

## 28. Initialization

A newly instantiated standard module receives the declared initial state of every internal primitive.

The first reaction uses the ordinary module expansion under the complete initialization snapshot or patch-time target input.

A standard module MAY therefore produce output or successor state different from its declared initial aggregate state during initialization.

A standard module added by reconfiguration follows the same rule.

No standard module receives an implicit `Low` external input, synthetic pulse, hidden reset, or special reduced evaluator.

## 29. Aggregate state

A standard module may expose a coherent aggregate state distinct from its internal primitive state.

Aggregate state:

- is a derived public inspection and explanation view;
- does not replace internal state in snapshots;
- MUST be derivable deterministically from canonical internal state;
- MUST identify any additional internal observation state required by the public transition law.

A snapshot restores internal state exactly, not merely the aggregate output.

## 30. Cross-internal state invariants

A stateful standard descriptor defines every invariant required for its internal state to represent a reachable module state.

Restoration, migration finalization, debug verification, and canonical snapshot validation MUST check those invariants in addition to each primitive's local state schema.

At minimum:

- every expected internal state owner exists under the correct role;
- no unexpected internal state owner exists;
- aggregate state derives successfully from internal state;
- internal edge-detector observation is compatible with the settled input facts required by the primitive law;
- module-specific relations required by the public reference law hold.

For `LevelResettableToggle`, a settled state with remembered reset `High` MUST have equal `toggle_state` and `reset_baseline` stored levels, because toggle pulses are suppressed throughout the asserted interval and the rising edge establishes equality. A snapshot violating this relation is not a valid restorable state for the standard module even though each primitive state value is locally well-typed.

A state reached through a supported parameter migration remains valid even when a presentation expectation changes. For `LevelResettableSampleHold`, changing `initial` preserves the held state because `initial` governs fresh or migration-reset state only. Changing `reset_to` while reset is asserted instead updates the held state immediately, as required by the module-specific migration law.

## 31. Explanations

A standard module provides a module-level explanation that groups the primitive derivation into the public concept.

The default explanation MUST identify:

- public input facts relevant to the result;
- prior aggregate state where relevant;
- the public rule applied;
- current output or non-emission;
- successor aggregate state where relevant;
- suppression, reset, or conflict facts where relevant;
- the module instance and exact standard descriptor.

A caller MUST be able to drill down to the canonical primitive explanation graph.

Module-level grouping MUST NOT discard causal contributors, pulse multiplicity, selected and suppressed branches, or migration ancestry required by the ordinary provenance rules.

## 32. Why-not explanations

Each descriptor defines a public `why not` interpretation.

A why-not explanation should answer questions such as:

```text
Why is this level not High?
Why did this module not change?
Why was this toggle pulse ignored?
Why was the held value not sampled?
Why did reset win?
```

The explanation MUST distinguish:

- absent required support;
- present blockers;
- threshold deficit;
- threshold excess;
- even pulse parity;
- suppressed pulses;
- retained prior state;
- reset dominance.

## 33. Inspection

Every standard module instance exposes at least:

```text
ModuleInstanceKey
StandardModuleRef
catalogue display name
parameters
public port schema and stable keys
current public level inputs and outputs when initialized
aggregate state when applicable
pending obligations when applicable
next deadline when applicable
active module diagnostics
current explanation root
module fingerprint
expansion fingerprint
hierarchy parent and children
```

Primitive internals remain inspectable through an explicit expanded view containing:

```text
internal role
module-local stable key
qualified network identity
primitive kind and configuration
current primitive state
pending events
internal diagnostics
primitive explanation roots
```

Flattening for execution MUST NOT remove this hierarchy or role mapping.

## 34. Public pulse activity

Pulse values remain reaction-scoped.

A settled machine inspection MUST NOT present an old public pulse input or output count as a persistent current signal value.

Module-level transaction results and reaction inspection MAY report:

- pulse counts in the completed reaction;
- accepted and suppressed counts;
- reset presence;
- the resulting cause.

Optional retained history remains explicitly historical and follows the ordinary retention policy.

---

# Part V — Combinational standard modules

## 35. Shared variadic level rules

`Exactly`, `AtMost`, and `AllEqual` accept zero or more `Level` inputs.

For all three:

- public input order is non-semantic;
- each port has stable identity;
- duplicate sources are counted once per connected public port;
- reordering public ports does not change behavior;
- zero and unary arities are valid where the law is total;
- suspicious degenerate forms produce non-blocking module diagnostics;
- a canonical constant-result expansion MAY intentionally leave public inputs without internal incidence;
- such intentionally unused public inputs are part of the descriptor-defined expansion and MUST NOT produce a generic unused-module-input diagnostic;
- no internal state or pending work exists.

## 36. `Exactly`

### 36.1 Identity

```text
id:                mossignal.standard.exactly
semantic version:  1
expansion version: 1
introduced:        catalogue 1
category:          combinational standard module
```

### 36.2 Inclusion rationale

`Exactly` deserves a standard boundary because its public concept is exact cardinality, not merely the lower-and-upper threshold formula used internally. The module provides stable threshold-oriented inspection, deficit and excess explanations, arity-aware diagnostics, and canonical migration across expansion case branches.

### 36.3 Public interface

```text
Inputs:
  inputs: zero or more Level ports

Output:
  result: Level

Parameter:
  threshold: u64
```

No default threshold exists.

### 36.4 Behavioral law

Let:

```text
n = number of public input ports
h = number of those ports currently High
k = threshold
```

Then:

```text
result = High iff h == k
```

Total boundary cases include:

```text
Exactly(0, [])       = High
Exactly(k > 0, [])   = Low
Exactly(k > n, ...)  = Low
Exactly(0, inputs)   = High iff every input is Low
Exactly(n, inputs)   = High iff every input is High
```

Duplicate sources contribute once per public port.

### 36.5 Public dependency signature

When `0 <= k <= n` and `n > 0`, every public input may affect `result`.

When `k > n` or `n = 0`, `result` is constant for that declaration and has no current public input dependency.

### 36.6 Canonical expansion

The exact expansion is selected by arity and threshold.

#### Case A — impossible threshold

When `k > n`:

```text
constant_result = Constant(Low)
result          = constant_result
```

#### Case B — zero threshold

When `k = 0` and `n = 0`:

```text
constant_result = Constant(High)
result          = constant_result
```

When `k = 0` and `n = 1`:

```text
result = Not(the sole public input)
```

When `k = 0` and `n > 1`:

```text
any_input = Any(inputs)
not_any   = Not(any_input)
result    = not_any
```

#### Case C — threshold equals arity

When `k = n = 1`:

```text
result = the sole public input
```

When `k = n > 1`:

```text
all_input = All(inputs)
result    = all_input
```

#### Case D — interior threshold

When `0 < k < n`:

```text
at_least_lower = AtLeast(k, inputs)
at_least_upper = AtLeast(k + 1, inputs)
not_upper      = Not(at_least_upper)
combine        = All([at_least_lower, not_upper])
result         = combine
```

The case split avoids generating internally degenerate threshold warnings as an implementation artifact of a valid public module declaration.

### 36.7 Explanation

When `High`, the explanation reports:

```text
observed High count = threshold
all High contributors
all Low non-contributors
```

When `Low` because `h < k`, the why-not explanation reports the deficit `k - h` and the Low inputs that prevent the threshold from being reached.

When `Low` because `h > k`, it reports the excess `h - k` and the High inputs contributing to that excess.

When `k > n`, it reports that the threshold is impossible for the declared arity.

### 36.8 Inspection

The module summary exposes:

```text
threshold
arity
current High count
current Low count
result
constant-result classification
public dependency classification
```

### 36.9 Reconfiguration

Changing threshold or variadic arity is stateless and uses ordinary target reevaluation.

Surviving public port keys preserve identity. Internal stateless roles that remain present preserve identity; roles absent from one case branch are added or removed without state loss.

No migration policy is required beyond ordinary module replacement and public port correspondence.

## 37. `AtMost`

### 37.1 Identity

```text
id:                mossignal.standard.at_most
semantic version:  1
expansion version: 1
introduced:        catalogue 1
category:          combinational standard module
```

### 37.2 Inclusion rationale

`AtMost` is included because its public semantics are an upper-bound cardinality constraint with allowance and excess explanations, not merely a negated `AtLeast` spelling. The stable module boundary also owns arity-dependent constant cases, diagnostics, and threshold migration.

### 37.3 Public interface

```text
Inputs:
  inputs: zero or more Level ports

Output:
  result: Level

Parameter:
  threshold: u64
```

### 37.4 Behavioral law

With `n`, `h`, and `k` defined as above:

```text
result = High iff h <= k
```

Boundary cases include:

```text
AtMost(k, [])        = High
AtMost(k >= n, ...)  = High
AtMost(0, inputs)    = High iff every input is Low
```

### 37.5 Public dependency signature

When `k < n`, every public input may affect `result`.

When `k >= n`, the result is constant `High` and has no current public input dependency.

### 37.6 Canonical expansion

#### Case A — threshold covers arity

When `k >= n`:

```text
constant_result = Constant(High)
result          = constant_result
```

#### Case B — zero threshold below arity

When `k = 0` and `n = 1`:

```text
result = Not(the sole public input)
```

When `k = 0` and `n > 1`:

```text
any_input = Any(inputs)
not_any   = Not(any_input)
result    = not_any
```

#### Case C — interior threshold

When `0 < k < n`:

```text
at_least_upper = AtLeast(k + 1, inputs)
not_upper      = Not(at_least_upper)
result         = not_upper
```

### 37.7 Explanation

When `High`, the explanation reports the current High count and remaining allowance `k - h`.

When `Low`, it reports the excess `h - k` and every High input contributing to the violated upper bound.

When `k >= n`, it reports that the declared threshold covers every possible valuation.

### 37.8 Inspection

The summary exposes:

```text
threshold
arity
current High count
current Low count
remaining allowance or excess
result
constant-result classification
```

### 37.9 Reconfiguration

Threshold and arity changes are stateless. The same rules as `Exactly` apply.

## 38. `AllEqual`

### 38.1 Identity

```text
id:                mossignal.standard.all_equal
semantic version:  1
expansion version: 1
introduced:        catalogue 1
category:          combinational standard module
```

### 38.2 Inclusion rationale

`AllEqual` expresses consensus over a variadic set. Its public supporters and blockers, vacuous zero- and unary-arity law, and stable equality-oriented inspection are materially clearer than exposing a parallel `All`/`Any`/`Not` formula.

### 38.3 Public interface

```text
Inputs:
  inputs: zero or more Level ports

Output:
  result: Level

Parameters: none
```

### 38.4 Behavioral law

```text
result = High iff every public input has the same level
```

Total boundary cases are:

```text
AllEqual([])  = High
AllEqual([x]) = High
```

For arity at least two, `result` is `High` exactly when all inputs are `Low` or all are `High`.

### 38.5 Public dependency signature

For arity zero or one, the result is constant `High` and has no current public input dependency.

For arity at least two, every input may affect `result`.

### 38.6 Canonical expansion

When `n <= 1`:

```text
constant_true = Constant(High)
result        = constant_true
```

When `n >= 2`:

```text
all_high = All(inputs)
any_high = Any(inputs)
none_high = Not(any_high)
combine = Any([all_high, none_high])
result  = combine
```

### 38.7 Explanation

When `High`, the explanation identifies whether the common value is `Low`, `High`, or vacuous because arity is zero or one.

When `Low`, it reports both the complete High-input group and the complete Low-input group. Each nonempty group blocks equality with the other.

### 38.8 Inspection

The summary exposes:

```text
arity
High count
Low count
common level when one exists
result
constant-result classification
```

### 38.9 Reconfiguration

Arity changes are stateless and preserve surviving public-port identity.

---

# Part VI — Stateful standard modules

## 39. Shared resettable-module rules

The initial resettable modules use one fixed reset kind and one fixed priority law per descriptor. Reset kind is not a runtime parameter because it changes the typed public interface.

The initial priority rule is:

```text
reset dominates the action occurring in the same reaction
```

No configurable conflict-policy family is provided. A caller needing another policy must author a separate user module or await a separately specified standard descriptor.

Changing an `initial` parameter on a surviving module does not overwrite preserved runtime state. It affects:

- fresh construction;
- a topology-migration `Reset` outcome;
- newly created internal state.

This follows the ordinary built-in state migration rule.

## 40. `PulseResettableToggle`

### 40.1 Identity

```text
id:                mossignal.standard.pulse_resettable_toggle
semantic version:  1
expansion version: 1
introduced:        catalogue 1
category:          stateful standard module
```

### 40.2 Inclusion rationale

This module gives resettable parity state one durable public identity while preserving reset-dominant same-reaction behavior. Its boundary is valuable for aggregate-state inspection, reset explanations, and migration of the two internal stored levels as one coherent abstraction.

### 40.3 Public interface

```text
Inputs:
  toggle: Pulse
  reset:  Pulse

Output:
  state: Level

Parameter:
  initial: LogicLevel
```

Both ports are fixed and have catalogue-defined stable keys.

### 40.4 Public reference law

Let:

```text
q = previous aggregate state
n = toggle pulse count
r = reset pulse count
```

Then:

```text
if r > 0:
    current output = Low
    successor q    = Low
else if n is odd:
    current output = not q
    successor q    = not q
else:
    current output = q
    successor q    = q
```

Reset multiplicity beyond presence is irrelevant to the reset decision but all reset causes remain available.

Toggle multiplicity is interpreted by parity.

Simultaneous reset and toggle always produces `Low`, including when the toggle count is odd.

### 40.5 Public dependency signature

```text
toggle -> state
reset  -> state
```

Both dependencies are current-reaction dependencies.

### 40.6 Canonical expansion

Internal roles are:

```text
toggle_state
reset_baseline
relative_state
```

The expansion is:

```text
toggle_state = Toggle(
    toggle,
    initial = configured initial
)

reset_baseline = SampleHold(
    value   = toggle_state,
    sample  = reset,
    initial = Low
)

relative_state = Parity([
    toggle_state,
    reset_baseline,
])

state = relative_state
```

Let internal stored levels be `a` for `toggle_state` and `b` for `reset_baseline`.

The aggregate state is:

```text
q = a XOR b
```

On reset, `reset_baseline` samples the post-toggle current value of `toggle_state`, making `a XOR b = Low` in the same reaction. This is the canonical reason reset dominates simultaneous toggle.

### 40.7 Initialization

Initial internal state is:

```text
a = configured initial
b = Low
q = configured initial
```

An initialization reset pulse forces `q = Low` in the first reaction.

An initialization toggle batch without reset applies ordinary parity to the configured initial state.

### 40.8 Explanation

The module-level explanation reports:

```text
previous aggregate state
toggle count and parity
reset presence and contributing reset count
whether reset dominated
resulting aggregate state
```

When reset is present, the primitive drill-down shows the reset baseline sampling the post-toggle internal state.

A why-not-change explanation identifies zero or even toggle multiplicity. A why-not-High explanation identifies an active reset or the resulting parity.

### 40.9 Inspection

The aggregate summary exposes:

```text
current state
configured initial value
latest reset cause
latest accepted toggle cause
latest reaction toggle parity when available
```

Expanded inspection additionally exposes `a`, `b`, and their stable internal roles.

Neither pulse count remains persistent after the reaction.

### 40.10 Reconfiguration

Under an exact same descriptor replacement:

- `toggle_state` stored level is preserved;
- `reset_baseline` stored level is preserved;
- aggregate state is thereby preserved;
- changing `initial` preserves both stored levels;
- binding changes cause ordinary patch-time reevaluation;
- a topology-migration `Reset` outcome initializes `a = new initial` and `b = Low` and reports discarded source state as required.

No pending work exists.

## 41. `LevelResettableToggle`

### 41.1 Identity

```text
id:                mossignal.standard.level_resettable_toggle
semantic version:  1
expansion version: 1
introduced:        catalogue 1
category:          stateful standard module
```

### 41.2 Inclusion rationale

This module defines a level-held reset contract that suppresses toggle work while asserted, establishes a precise rising-reset transition, and resumes from `Low` after release. The named boundary materially improves explanation, inspection, and migration of its coordinated gate, edge, toggle, and baseline state.

### 41.3 Public interface

```text
Inputs:
  toggle: Pulse
  reset:  Level

Output:
  state: Level

Parameter:
  initial: LogicLevel
```

### 41.4 Public reference law

Let `q` be previous aggregate state and `n` the current toggle count.

```text
if reset = High:
    current output = Low
    successor q    = Low
    all toggle pulses are suppressed
else if n is odd:
    current output = not q
    successor q    = not q
else:
    current output = q
    successor q    = q
```

A reset that settles `High` in the same reaction as toggle suppresses the complete toggle batch.

While reset remains `High`, the module remains `Low` and does not accumulate ignored toggle work.

When reset returns `Low`, the module remains `Low` until a later odd toggle batch.

### 41.5 Public dependency signature

```text
toggle -> state
reset  -> state
```

### 41.6 Canonical expansion

Internal roles are:

```text
reset_inverter
accepted_toggle
low_constant
toggle_state
reset_edge
reset_baseline
relative_state
reset_select
```

The expansion is:

```text
reset_inverter = Not(reset)

accepted_toggle = PulseGate(
    pulses = toggle,
    enable = reset_inverter
)

low_constant = Constant(Low)

toggle_state = Toggle(
    toggle  = accepted_toggle,
    initial = configured initial
)

reset_edge = RisingEdge(
    input = reset,
    initialization = Assume(Low)
)

reset_baseline = SampleHold(
    value   = toggle_state,
    sample  = reset_edge,
    initial = Low
)

relative_state = Parity([
    toggle_state,
    reset_baseline,
])

reset_select = Select(
    selector  = reset,
    when_low  = relative_state,
    when_high = low_constant
)

state = reset_select
```

The aggregate stored state when the module is not held in reset is:

```text
q = toggle_state XOR reset_baseline
```

The rising reset edge samples the current toggle state into the reset baseline, making the relative state `Low`. The pulse gate prevents actions while reset is `High`, and the final select makes the held-reset output explicit.

### 41.7 Initialization

When initial reset is `Low`, the configured initial level is the previous aggregate state and the initialization toggle batch applies the ordinary parity law in the first reaction.

When initial reset is `High`:

- `RisingEdge(Assume(Low))` emits one reset edge;
- the complete toggle batch is suppressed;
- the reset baseline samples the toggle state;
- output and successor aggregate state are `Low`.

### 41.8 Explanation

When reset is `High`, the explanation reports:

```text
reset as the controlling blocker to High
the number of suppressed toggle pulses
the retained internal toggle state
the reset baseline establishing aggregate Low
```

When reset is `Low`, the explanation reports previous aggregate state and accepted toggle parity.

### 41.9 Inspection

The aggregate summary exposes:

```text
state
reset level
configured initial
whether held in reset
latest reset transition cause
latest accepted toggle cause
latest suppressed toggle count when available in the transaction result
```

Expanded inspection exposes every internal role, including the edge detector's remembered reset observation.

### 41.10 Reconfiguration

Exact same-descriptor replacement preserves:

```text
toggle_state stored level
reset_baseline stored level
reset_edge remembered observation state
```

Changing `initial` preserves those values.

Changing the reset binding may legitimately create a patch-time rising edge relative to the preserved remembered observation and thereby reset the module.

A topology-migration `Reset` outcome reinitializes all three state owners and reports source state loss as required.

## 42. `LevelResettableSampleHold`

### 42.1 Identity

```text
id:                mossignal.standard.level_resettable_sample_hold
semantic version:  1
expansion version: 1
introduced:        catalogue 1
category:          stateful standard module
```

### 42.2 Inclusion rationale

This module provides a stable, inspectable resettable storage abstraction whose public behavior is materially clearer than its edge-detection, selection, pulse-merging, and sample-hold expansion.

Its named boundary is valuable because it defines:

- reset-edge behavior;
- reset dominance over simultaneous sampling;
- separate fresh-state and reset-target parameters;
- coherent module-level explanations;
- state migration while reset is asserted.

### 42.3 Public interface

```text
Inputs:
  value:  Level
  sample: Pulse
  reset:  Level

Output:
  held: Level

Parameters:
  initial:  LogicLevel
  reset_to: LogicLevel
```

Neither parameter has a default.

`initial` defines fresh or migration-reset state. `reset_to` defines the value captured when the public reset control applies.

### 42.4 Public state

The public reference state is:

```text
q              held level
previous_reset remembered reset observation
```

For a fresh module:

```text
q = configured initial
previous_reset = Low
```

The remembered reset state corresponds to `RisingEdge(Assume(Low))`.

### 42.5 Public reference law

At one reaction, let:

```text
rising_reset = previous_reset = Low and reset = High
capture      = sample count > 0 or rising_reset
```

Then:

```text
if capture and reset = High:
    current held = configured reset_to
    successor q  = configured reset_to
else if capture and reset = Low:
    current held = settled value
    successor q  = settled value
else:
    current held = q
    successor q  = q

successor previous_reset = reset
```

Consequences:

- a rising reset immediately captures `reset_to`;
- simultaneous sample and rising reset captures `reset_to`;
- while reset remains `High`, any sample pulse captures `reset_to`;
- value changes without a sample or rising reset do not change the held output;
- falling reset does not sample the current value;
- multiple sample pulses are equivalent to one capture, while all causes remain available.

Changing `initial` on a surviving module never retroactively overwrites preserved `q`. It affects fresh construction, newly created state, or a topology-migration `Reset` outcome.

Changing `reset_to` while reset is `Low` preserves `q` and affects future reset-controlled captures. Changing `reset_to` while reset is `High` migrates `q` immediately to the new `reset_to` value so the preserved state remains coherent with the asserted public reset control.

### 42.6 Public dependency signature

```text
value  -> held
sample -> held
reset  -> held
```

All three are conservative current-reaction dependencies because each can affect the current output under a valid prior state and current batch.

### 42.7 Canonical expansion

Internal roles are:

```text
reset_value_constant
capture_value
reset_edge
capture_pulses
held_state
```

The expansion is:

```text
reset_value_constant = Constant(configured reset_to)

capture_value = Select(
    selector  = reset,
    when_low  = value,
    when_high = reset_value_constant
)

reset_edge = RisingEdge(
    input = reset,
    initialization = Assume(Low)
)

capture_pulses = Merge([
    sample,
    reset_edge,
])

held_state = SampleHold(
    value   = capture_value,
    sample  = capture_pulses,
    initial = configured initial
)

held = held_state
```

### 42.8 Initialization

With initial reset `Low`, an initialization sample captures the settled input value; otherwise the output begins at configured `initial`.

With initial reset `High`, `reset_edge` emits and the first reaction captures configured `reset_to` regardless of simultaneous sample pulses or current value.

### 42.9 Explanation

The module explanation reports:

```text
previous held value
configured initial value
configured reset target
current value input
sample count
current reset level
whether reset rose
whether capture occurred
which value was selected for capture
resulting held value
```

Why-not-change reports that no sample pulse and no rising reset occurred.

When reset is `High`, why-not-sample-value reports that reset selected `reset_to` instead.

### 42.10 Inspection

The aggregate summary exposes:

```text
held value
configured initial
configured reset_to
reset level
remembered previous reset observation
latest capture kind: sample, reset, both, or none
latest capture cause
```

Expanded inspection exposes the internal edge-detector and sample-hold state.

### 42.11 Reconfiguration

Exact same-descriptor replacement ordinarily preserves:

```text
held_state stored level
reset_edge remembered observation state
```

Parameter changes have these standard outcomes:

```text
initial changed:
  preserve held_state and reset_edge;
  use the new value only for fresh state or a migration Reset outcome

reset_to changed while settled reset = Low:
  preserve held_state and reset_edge;
  use the new value for later reset-controlled captures

reset_to changed while settled reset = High:
  migrate held_state immediately to the new reset_to value;
  preserve reset_edge remembered observation;
  report the parameter change as the cause of the held-state transition
```

Changing the reset binding may create a patch-time rising edge relative to the preserved remembered observation.

A topology-migration `Reset` outcome establishes:

```text
held_state = new configured initial
reset_edge = Assume(Low) unreacted declared state
```

The patch-time first reaction may then react immediately to a settled target reset level according to ordinary initialization semantics; if reset is `High`, it captures the target `reset_to` value.

# Part VII — Deferred and rejected initial candidates

## 43. Thin Boolean conveniences

`Nand`, `Nor`, and `Xnor` are not standard modules in catalogue version 1.

They are short, transparent multi-primitive compositions whose visible hierarchy would usually add more noise than semantic value. They remain builder-only operations.

`LevelGate` and `Majority` are primitive aliases because each creates exactly one existing primitive after its arguments determine that primitive's parameterization:

```text
level_gate(value, enable) = All([value, enable])
majority(inputs)          = AtLeast(floor(arity / 2) + 1, inputs)
```

A caller may wrap any of these conveniences in a user-defined module when a durable domain-specific boundary is useful.

## 44. Pulse algebra candidates

### 44.1 `AnyPulse`

`AnyPulse` is builder-only:

```text
Coalesce(Merge(inputs))
```

Its two-step expansion is direct and owns no state.

### 44.2 `PulseMultiply`

`PulseMultiply` is deferred.

The current primitive language can emulate a fixed factor only by connecting one source to many `Merge` ports. That gives expansion size proportional to the factor, creates impractical parameter-dependent topology for large factors, and lacks a natural checked multiplicative primitive.

The catalogue does not impose an arbitrary semantic factor limit merely to make this expansion manageable.

### 44.3 `PulseCap`

`PulseCap(limit)` is deferred because the current primitive language cannot express:

```text
min(input_count, limit)
```

for arbitrary limits greater than one.

`limit = 1` is `Coalesce`; `limit = 0` is a constant zero pulse behavior, for which no pulse constant exists and no module is needed.

### 44.4 `PulseParity`

`PulseParity` is deferred.

A stateful toggle-and-edge composition can reproduce odd/even output for reachable states, but it introduces retained internal state for a concept whose intended public law is reaction-local pulse arithmetic. The catalogue does not standardize that mismatch without a stronger reason or a direct pulse-count primitive.

## 45. Routing and selection candidates

Low-enabled variants remain explicit inversion plus the canonical high-enabled primitive.

Multi-way selection and routing are deferred because the core has no integer selector signal. A bit-vector selector would require additional policy for:

- bit significance;
- branch count not equal to a power of two;
- out-of-range selector values;
- static dependency reporting;
- stable branch and selector-port identity.

Binary `Select`, `PulseSelect`, and `PulseRoute` remain the canonical primitives.

## 46. `PulseResettableSampleHold`

A pulse-resettable sample-and-hold is not included.

The current primitive language cannot use the presence of a reset pulse as a reaction-local level selector without introducing extra retained state or a current-reaction feedback cycle. The catalogue therefore includes only the precise level-reset form.

## 47. Temporal candidates

The following remain deferred:

```text
FixedOneShot
RetriggerableOneShot
Timeout
Watchdog
PulseStretcher
RateLimiter
```

They are not merely awaiting names. Their exact behavior requires capabilities not safely expressible through the current primitive set.

### 47.1 Fixed one-shot

A fixed one-shot must accept a trigger only while idle and schedule exactly one expiration for the accepted trigger.

A naive `PulseDelay` reset for every trigger leaves delayed reset obligations for triggers that should have been ignored. Those stale obligations can later terminate a newly started shot prematurely.

Preventing that requires an acceptance decision based on previous active state without creating a current-reaction feedback cycle, or a primitive capable of canceling the inappropriate obligation.

### 47.2 Retriggerable one-shot and pulse stretcher

These require one active expiration that moves to `latest_trigger + duration`.

`PulseDelay` preserves every trigger deadline and does not cancel the earlier expiration. The current primitive language cannot express replacement of one pending obligation by a later pulse without a suitable temporal state primitive.

### 47.3 Timeout and watchdog

These require an obligation that is armed or restarted by arbitrary activity and fires only after a complete quiet interval.

Encoding activity through `Toggle` plus `InertialDelay` fails when an even number of activity pulses returns the toggle to the delayed output before expiry, eliminating the required later timeout.

A correct watchdog requires a generation, retriggerable candidate, or equivalent primitive capability.

### 47.4 Rate limiter

A rate limiter requires a refractory state whose acceptance decision observes previous availability and whose reset occurs at a future deadline.

Combinationally feeding the current refractory output back into the pulse acceptance path creates a reaction cycle; accepting all pulses and delaying resets creates stale-obligation errors.

### 47.5 Admission condition for future temporal modules

A temporal module may enter a later catalogue only after the primitive language or standard-module mechanism can express all of:

```text
accepted versus ignored trigger identity
one exact current active interval
cancellation or replacement of pending work
exact-deadline interaction
large-time-jump equivalence
reset and rearm semantics
parameter migration while active
complete pending-event ownership
```

No vague `Timer` module is permitted.

---

# Part VIII — Construction and discovery API

## 48. Public catalogue types

The crate SHOULD expose types broadly equivalent to:

```rust
pub struct StandardCatalogueVersion(pub u32);
pub struct StandardModuleId(/* opaque owned ASCII id */);
pub struct StandardModuleSemanticVersion(pub u32);
pub struct StandardModuleExpansionVersion(pub u32);

pub struct StandardModuleRef {
    pub id: StandardModuleId,
    pub semantic_version: StandardModuleSemanticVersion,
    pub expansion_version: StandardModuleExpansionVersion,
}

pub struct StandardModuleDescriptor<D> { /* immutable */ }
pub struct StandardModuleDeclaration<D> { /* validated selection */ }
pub struct StandardModuleRequest<D> { /* dynamic unchecked request */ }
pub struct StandardCatalogue<D> { /* immutable built-in catalogue view */ }
```

No global mutable registry is required.

`StandardCatalogue<D>` MAY be a zero-sized view over static descriptor data.

## 49. Descriptor discovery

The catalogue API MUST support:

```rust
impl<D> StandardCatalogue<D> {
    pub fn version(&self) -> StandardCatalogueVersion;

    pub fn descriptors(
        &self,
    ) -> StandardModuleDescriptorIter<'_, D>;

    pub fn descriptor(
        &self,
        module: &StandardModuleRef,
    ) -> Result<&StandardModuleDescriptor<D>, CatalogueFailure>;

    pub fn latest(
        &self,
        id: &StandardModuleId,
    ) -> Result<&StandardModuleDescriptor<D>, CatalogueFailure>;

    pub fn build(
        &self,
        request: StandardModuleRequest<D>,
    ) -> Report<ModuleDef<D>>;
}
```

Discovery metadata includes:

```text
id and versions
display name
category
availability and deprecation
typed input and output schemas
parameter schemas
public dependency signature
stateful and temporal classification
diagnostic code links
migration compatibility summaries
canonical diagram data
```

`latest` is an authoring convenience only. Persisted artifacts and reconfiguration MUST use an exact `StandardModuleRef`.

## 50. Dynamic construction

A dynamic request supplies:

```text
exact StandardModuleRef
parameter set
explicit variadic public port keys
```

Fixed public port keys come from the descriptor.

Dynamic construction MUST diagnose:

- unknown identifier;
- unavailable version;
- missing parameter;
- unknown parameter;
- wrong parameter kind;
- invalid parameter value;
- duplicate public port key;
- wrong signal kind;
- invalid arity where applicable;
- canonical key collision.

Successful construction returns an ordinary validated `ModuleDef<D>` carrying `ModuleOrigin::Standard`.

## 51. Typed construction result

Explicit typed standard-module construction SHOULD return:

```rust
pub struct AddedStandardModule<O> {
    instance: AddedModuleInstance,
    outputs: O,
    module_ref: StandardModuleRef,
}
```

with accessors for:

```text
ModuleInstanceKey
outputs
StandardModuleRef
```

A one-output module uses `Signal<S>` as `O`.

## 52. Keyed variadic bindings

Explicit variadic construction uses a shape broadly equivalent to:

```rust
pub struct KeyedModuleInput<S> {
    pub key: ModuleInputKey<S>,
    pub source: Signal<S>,
}
```

The iterator is consumed immediately and does not remain borrowed.

## 53. Typed convenience methods

`NetworkBuilder<D>` and `ModuleBuilder<D>` MUST both support the standard modules so user-defined modules may contain standard modules.

Concise methods MAY allocate the module instance and variadic public-port keys locally and return only the output signal:

```rust
pub fn exactly<I>(
    &mut self,
    threshold: u64,
    inputs: I,
) -> Result<Signal<Level>, AuthoringFailure>
where
    I: IntoIterator<Item = Signal<Level>>;

pub fn at_most<I>(
    &mut self,
    threshold: u64,
    inputs: I,
) -> Result<Signal<Level>, AuthoringFailure>
where
    I: IntoIterator<Item = Signal<Level>>;

pub fn all_equal<I>(
    &mut self,
    inputs: I,
) -> Result<Signal<Level>, AuthoringFailure>
where
    I: IntoIterator<Item = Signal<Level>>;

pub fn pulse_resettable_toggle(
    &mut self,
    toggle: Signal<Pulse>,
    reset: Signal<Pulse>,
    initial: LogicLevel,
) -> Result<Signal<Level>, AuthoringFailure>;

pub fn level_resettable_toggle(
    &mut self,
    toggle: Signal<Pulse>,
    reset: Signal<Level>,
    initial: LogicLevel,
) -> Result<Signal<Level>, AuthoringFailure>;

pub fn level_resettable_sample_hold(
    &mut self,
    value: Signal<Level>,
    sample: Signal<Pulse>,
    reset: Signal<Level>,
    initial: LogicLevel,
    reset_to: LogicLevel,
) -> Result<Signal<Level>, AuthoringFailure>;
```

## 54. Explicit keyed forms

Every standard module has an explicit form broadly equivalent to:

```rust
pub fn add_exactly<I>(
    &mut self,
    key: ModuleInstanceKey,
    threshold: u64,
    inputs: I,
    meta: DiagnosticMeta,
) -> Result<AddedStandardModule<Signal<Level>>, AuthoringFailure>
where
    I: IntoIterator<Item = KeyedModuleInput<Level>>;
```

Fixed-interface methods receive `ModuleInstanceKey`, typed signals, semantic configuration, and `DiagnosticMeta`.

They use catalogue-defined fixed public interface keys and return `AddedStandardModule<Signal<Level>>`.

The explicit forms MUST expose module instance identity and MUST NOT require the caller to know internal primitive keys.

## 55. Ordinary module instantiation

Callers MAY also obtain a generated `ModuleDef<D>` from the catalogue and use the existing generic `NetworkBuilder::instantiate` API.

The standard convenience methods MUST produce the same canonical definition and instance structure as generic instantiation of the corresponding generated module.

---

# Part IX — Diagnostics

## 56. Diagnostic ownership

A module-level condition belongs to the `ModuleInstanceKey` or standard `ModuleDef` as its primary subject.

Related evidence MAY identify public ports and canonical internal subjects.

Primitive diagnostics caused by the implementation graph remain primitive diagnostics but are grouped beneath the owning module in default presentation.

Expected internal forms chosen by the canonical case split MUST NOT create redundant warnings merely because the module was implemented from lower-level primitives.

## 57. Blocking catalogue diagnostics

The standard catalogue introduces these blocking conditions:

| Stable code | Stage | Meaning |
|---|---|---|
| `standard_module.unknown_id` | dynamic construction, decoding | No descriptor exists for the identifier |
| `standard_module.unsupported_version` | construction, validation, restoration, replay | The exact semantic or expansion version is unavailable |
| `standard_module.missing_parameter` | construction, validation | A required parameter is absent |
| `standard_module.unexpected_parameter` | construction, validation | The declaration contains an unknown parameter |
| `standard_module.parameter_kind_mismatch` | construction, decoding | A parameter has the wrong value kind |
| `standard_module.invalid_parameter` | construction, validation | A parameter value violates its schema |
| `standard_module.interface_mismatch` | validation, restoration, patch preparation | Public ports do not match the descriptor |
| `standard_module.expansion_mismatch` | validation, restoration, patch preparation | Persisted or patched internals differ from canonical expansion |
| `standard_module.noncanonical_internal_edit` | patch preparation | A semantic internal edit attempts to retain standard identity |
| `standard_module.incompatible_version_migration` | patch preparation or finalization | No declared migration exists between versions |
| `standard_module.internal_key_collision` | descriptor validation or compilation | Canonical key derivation produced a duplicate key |
| `standard_module.catalogue_invariant` | descriptor validation or compilation | Descriptor data and generated expansion disagree |

`internal_key_collision` and `catalogue_invariant` indicate a library defect when produced by a shipped descriptor. They remain structured failures rather than panics or silent corruption.

## 58. Non-blocking module diagnostics

The initial catalogue introduces these non-blocking static conditions:

| Stable code | Applies to | Meaning |
|---|---|---|
| `standard_module.deprecated_alias` | any | An authoring alias is deprecated |
| `standard_module.empty_variadic` | `Exactly`, `AtMost`, `AllEqual` | The module has zero public inputs |
| `standard_module.unary_degenerate` | `Exactly`, `AtMost`, `AllEqual` | The module has one input and simplifies to a constant or identity-like law |
| `standard_module.impossible_threshold` | `Exactly` | `threshold > arity`, so result is always `Low` |
| `standard_module.constant_result` | `Exactly`, `AtMost`, `AllEqual` | Parameters and arity make output constant |
| `standard_module.duplicate_source` | variadic modules | One upstream source is bound to several public ports |

These are validation or patch-preparation occurrences, not runtime diagnostic episodes.

Arity diagnostics and threshold diagnostics are independent dimensions. A declaration may therefore report both, for example `empty_variadic` together with `impossible_threshold`.

Only the generic `constant_result` diagnostic is suppressed when a more precise diagnostic already explains why the result is constant. The precise conditions are:

```text
empty_variadic
unary_degenerate
impossible_threshold
```

`constant_result` is used when no more specific constant-result condition applies, such as `AtMost(k >= arity)` with positive arity greater than one. `duplicate_source` is independent and may accompany any other static condition.

Descriptor-defined constant-result expansions may intentionally leave public inputs without internal incidence. Those inputs MUST NOT trigger a generic unused-public-input or unused-module-input diagnostic.

The exhaustive diagnostic catalogue may define their final evidence record shapes, but it MUST preserve the condition identity and semantic meaning established here.

## 59. Diagnostic evidence

Module diagnostics SHOULD provide structured evidence including, where applicable:

```text
StandardModuleRef
ModuleInstanceKey
parameter key and value
arity
threshold
public port keys
duplicate source and affected port keys
expected and encountered expansion fingerprints
expected and encountered interface schemas
base and target versions
```

Rendered prose is non-authoritative.

## 60. No new runtime episode family in catalogue 1

No catalogue version 1 module introduces a module-owned persistent runtime diagnostic episode.

Runtime diagnostics arising from internal primitives remain governed by the primitive specification. The current canonical expansions do not intentionally produce a persistent primitive conflict condition.

---

# Part X — Reconfiguration and migration

## 61. Module replacement boundary

A standard module parameter, version, or public variadic-port change is represented as replacement of the complete canonical module instance definition.

Preparation expands base and target declarations, derives internal role correspondence, and applies the ordinary topology-patch rules.

A standard module convenience MUST NOT conceal the resulting target graph, state outcomes, invalidated artifacts, or semantic loss.

## 62. Public compatibility dimensions

Preparation considers:

```text
module id
semantic version
expansion version
module instance key
fixed public interface keys
variadic public port keys
parameters
bindings
hierarchy
```

The standard outcome is determined as follows.

### 62.1 Same exact descriptor

The same `StandardModuleRef` is compatible subject to module-specific parameter and interface rules.

### 62.2 Different descriptor version

A version change is compatible only when the target descriptor contains an explicit migration entry from the source descriptor.

Catalogue version 1 declares only exact-version self-compatibility. No cross-version migration exists yet.

### 62.3 Different module id

No standard correspondence is inferred between different module identifiers.

A caller may use the topology-patch specification's explicit module and internal reassociation facilities, subject to canonical target validation and state-loss policy, but the catalogue supplies no automatic migration.

### 62.4 Changed module instance key

A key-changing replacement requires explicit `ModuleInstance` reassociation. Equal display names or identical descriptors do not imply continuity.

## 63. Public interface compatibility

For fixed-interface modules:

- all fixed public keys and signal kinds must match the descriptor;
- no port may be added or removed while claiming the same descriptor.

For variadic combinational modules:

- surviving equal public keys preserve identity;
- explicit same-kind reassociation may preserve a changed key;
- added ports are new;
- removed ports terminate;
- external incidence must be updated explicitly;
- order changes are non-semantic.

## 64. Internal correspondence

Internal correspondence is derived from equal permanent role keys and qualifying public port keys.

A role present in both expansions corresponds automatically when:

- subject category matches;
- signal kind and direction match;
- primitive kind and state schema are compatible under ordinary node migration rules.

A role that exists in only one parameter case is added or removed.

The migration report MUST expose module-level grouping and the complete internal outcomes.

## 65. Parameter migration matrix

| Module | Parameter change | Standard outcome |
|---|---|---|
| `Exactly` | threshold | Stateless replacement and reevaluation |
| `AtMost` | threshold | Stateless replacement and reevaluation |
| `AllEqual` | none | Not applicable |
| `PulseResettableToggle` | `initial` | Preserve internal state; affects fresh state or a migration `Reset` outcome only |
| `LevelResettableToggle` | `initial` | Preserve internal state; affects fresh state or a migration `Reset` outcome only |
| `LevelResettableSampleHold` | `initial` | Preserve held and edge state; affects fresh state or a migration `Reset` outcome only |
| `LevelResettableSampleHold` | `reset_to`, settled reset `Low` | Preserve held and edge state; affects future reset-controlled captures |
| `LevelResettableSampleHold` | `reset_to`, settled reset `High` | Migrate held state immediately to the new `reset_to`; preserve edge observation |

To apply a new `initial` immediately, the patch must select the topology-migration `Reset` outcome. Any discarded internal state is reported under the ordinary state-loss policy.

## 66. Binding changes

Changing a public input binding preserves compatible internal state but seeds ordinary patch-time reevaluation.

Consequences may include:

- immediate combinational output change;
- toggle pulse input at patch time;
- a reset edge relative to preserved edge-detector state;
- sample capture;
- new provenance roots;
- output establishment or change.

Topology change is a cause, not a synthetic signal value.

## 67. Topology-migration `Reset` for a stateful module

Selecting the topology-migration `Reset` outcome for a stateful standard module resets every internal state owner to the target descriptor's declared initial state.

The patch-time reaction then evaluates the target current inputs normally.

The migration report MUST list:

- every reset internal state cell;
- source aggregate state;
- target declared aggregate state;
- any resulting patch-time output and successor state;
- semantic loss under the selected policy.

## 68. Pending work

Catalogue version 1 standard modules own no temporal primitive and therefore no pending event.

The standard-module mechanism nevertheless requires future descriptors to provide a total pending-event migration rule for every temporal internal role.

## 69. Diagnostics and provenance migration

Static module diagnostics are recomputed for the target declaration.

Active primitive diagnostic episodes migrate only through ordinary internal subject correspondence and condition identity.

Current provenance roots are reevaluated under the target graph. Retained historical provenance remains immutable and may record module replacement ancestry according to the ordinary migration rules.

---

# Part XI — Persistence and replay

## 70. Persisted standard declaration

A standard-origin module definition persists:

```text
StandardModuleId
semantic version
expansion version
canonical parameters
public fixed and variadic port keys
canonical expansion fingerprint
complete expanded module definition
```

Display name and documentation text are not authoritative identity.

## 71. Restoration validation

Restoration and decoded definition validation MUST:

1. resolve the exact descriptor;
2. validate the declaration schema;
3. regenerate the canonical expansion;
4. verify the expansion fingerprint;
5. verify the complete persisted semantic expansion;
6. validate ordinary module structure;
7. validate containing network identity and snapshot state.

A standard declaration MUST NOT be silently reinterpreted using the latest descriptor for the same identifier.

## 72. Snapshot state

Snapshots persist internal state under qualified stable identity:

```text
(ModuleInstanceKey, module-internal stable key)
```

They also persist every ordinary state, pending event, diagnostic episode, and provenance root required by the persistence specification.

Restoration MUST reconstruct the exact internal ownership. Equivalent current public outputs are insufficient when internal state differs.

## 73. Replay

Exact replay requires the same standard module references and expansion meanings used by the recorded network and patches.

A replay chain cannot silently cross:

- semantic version change;
- expansion version change;
- internal key derivation change;
- parameter meaning change;
- migration-rule change.

A replay frame containing a standard module replacement re-prepares the canonical target descriptor and verifies the expected target fingerprint through the ordinary patch path.

## 74. Schema-only changes

A representation-only persistence schema upgrade may reorganize standard declaration fields only when it preserves:

```text
exact StandardModuleRef
parameters
public port keys
canonical semantic expansion
internal stable identity
fingerprints
state ownership
```

A behavior or expansion change is a semantic migration, not a schema upgrade.

---

# Part XII — Verification and generated artifacts

## 75. Required standard-module conformance matrix

Every standard module descriptor MUST classify and test:

```text
identifier and version
public port kinds and roles
fixed or variadic arity
parameter validation
public dependency signature
first reaction
complete public behavioral law
simultaneous input
pulse multiplicity
aggregate state
successor state
initialization
inspection summary
primitive drill-down
current explanation
why-not explanation
static diagnostics
snapshot round trip
replay
same-descriptor reconfiguration
parameter change
binding change
interface change
topology-migration `Reset`
incompatible version change
internal-key stability
module and network fingerprints
canonical persistence validation
```

Non-applicable fields must be marked explicitly.

## 76. Dual-view equivalence

For every descriptor:

```text
ModuleReferenceLaw(input, state)
    ==
Evaluate(CanonicalExpansion, input, internal_state)
```

The comparison MUST include public output, successor aggregate state, normalized internal-state projection, diagnostics, and normalized provenance.

The reference law SHOULD be implemented independently of the primitive expansion control flow.

## 77. Exhaustive combinational verification

For `Exactly`, `AtMost`, and `AllEqual`, tests MUST exhaustively enumerate all level valuations for bounded arities.

Required arity coverage includes at least:

```text
0
1
2
3
4
5
```

Threshold modules additionally enumerate:

```text
0
1
arity - 1 where defined
arity
arity + 1
u64::MAX
```

Larger generated arities use algebraic and property-based checks.

Tests MUST permute public port order and verify identical behavior and fingerprints when stable keys remain the same.

Duplicate-source cases MUST be included.

## 78. Exhaustive stateful verification

### 78.1 Pulse-resettable toggle

Tests enumerate:

```text
both aggregate states
all internal a/b combinations consistent with that aggregate
reset absent/present
bounded toggle counts including 0, 1, 2, 3, and larger parity representatives
simultaneous reset and toggle
initialization pulses
```

### 78.2 Level-resettable toggle

Tests cover:

```text
reset Low/High
reset rising/falling/steady
bounded toggle counts
simultaneous reset transition and toggle
initial reset High
suppressed toggle batches while reset High
release from reset
binding change creating a reset edge
```

### 78.3 Level-resettable sample hold

Tests cover:

```text
both held values
both value inputs
reset Low/High
reset rising/falling/steady
sample absent/present with several counts
simultaneous sample and reset rise
initial reset High
initial sample
initial change with preserved state
reset_to change while reset Low
reset_to change while reset High
```

## 79. Canonical expansion verification

Tests MUST verify that:

- descriptor generation is deterministic;
- insertion order does not change the expansion;
- variadic reordering with stable keys does not change the expansion fingerprint;
- every generated key is unique;
- every role maps to the required primitive kind;
- public dependency signatures equal expanded graph dependencies;
- persisted expansion mismatch is rejected;
- direct semantic internal edits are rejected while standard identity remains;
- generated documentation diagrams match the descriptor graph.

Golden canonical vectors SHOULD cover every descriptor and parameter case branch.

## 80. Reconfiguration verification

For every module, tests include:

```text
patch while uninitialized
patch while ready
same descriptor, same parameters
same descriptor, changed parameters
binding changes
variadic add/remove/reorder where applicable
instance-key reassociation
topology-migration `Reset`
RejectStateLoss
AllowReportedStateLoss
unsupported descriptor version
replacement with a user-defined module
```

The production path is compared with complete expansion, ordinary stable-key correspondence, and the reference topology-patch path.

## 81. Persistence verification

Required tests include:

```text
module definition canonical round trip
network definition canonical round trip
snapshot round trip with active state
replay from restored snapshot
unknown descriptor rejection
unsupported version rejection
parameter tampering rejection
public-key tampering rejection
internal-node tampering rejection
internal-connection tampering rejection
expansion-fingerprint tampering rejection
metadata-only difference acceptance where permitted
```

## 82. Machine-readable catalogue outputs

The authoritative descriptor source MUST generate or validate:

```text
Rust typed constructor metadata
dynamic descriptor API data
editor palette data
public port and parameter schemas
canonical diagrams
Markdown or rustdoc reference pages
diagnostic-code links
migration compatibility tables
conformance vectors
golden expansion fingerprints
```

Generated presentation artifacts are not semantic persistence artifacts. Persisted network and module definitions remain governed by the persistence specification.

## 83. Release gate

A descriptor may enter the released standard catalogue only when:

- its public law is complete;
- its canonical expansion is fixed;
- its internal roles and keys are fixed;
- all blocking and non-blocking conditions are classified;
- its persistence and migration rules are complete;
- its conformance suite passes;
- its generated documentation and descriptor data agree;
- its addition does not alter existing descriptor fingerprints.

---

# Appendix A — Catalogue version 1 summary

| ID | Public name | Category | State | Temporal work | Parameters |
|---|---|---|---|---|---|
| `mossignal.standard.exactly` | `Exactly` | Variadic level combinational | None | None | `threshold: u64` |
| `mossignal.standard.at_most` | `AtMost` | Variadic level combinational | None | None | `threshold: u64` |
| `mossignal.standard.all_equal` | `AllEqual` | Variadic level combinational | None | None | None |
| `mossignal.standard.pulse_resettable_toggle` | `PulseResettableToggle` | Stateful | Two stored levels | None | `initial: LogicLevel` |
| `mossignal.standard.level_resettable_toggle` | `LevelResettableToggle` | Stateful | Two stored levels plus reset observation | None | `initial: LogicLevel` |
| `mossignal.standard.level_resettable_sample_hold` | `LevelResettableSampleHold` | Stateful | Held level plus reset observation | None | `initial: LogicLevel`, `reset_to: LogicLevel` |

# Appendix B — Standard migration summary

| Change | Standard result |
|---|---|
| Same exact descriptor and parameters | Preserve compatible internal state |
| Combinational threshold change | Regenerate stateless expansion and reevaluate |
| Combinational variadic port reorder | Preserve by stable public keys |
| Combinational variadic port add/remove | Add/remove corresponding stateless incidence |
| Stateful `initial` change | Preserve runtime state; new value applies only to fresh state or a migration `Reset` outcome |
| Sample-hold `reset_to` change while reset is `Low` | Preserve runtime state; apply the new target to future reset-controlled captures |
| Sample-hold `reset_to` change while reset is `High` | Migrate held state immediately to the new target |
| Stateful topology-migration `Reset` | Reset all internal state; report discarded source state |
| Fixed public port change | Reject as descriptor mismatch |
| Expansion version change | Reject unless an explicit compatibility entry exists |
| Semantic version change | Reject unless an explicit compatibility entry exists |
| Module id change | No automatic correspondence |
| Instance key change | Require explicit module-instance reassociation |
| Semantic internal edit retaining standard origin | Reject |

# Appendix C — Convenience classification summary

| Convenience | Classification |
|---|---|
| `xor` | Primitive alias to `Parity` |
| `debounce` | Primitive alias to `InertialDelay` |
| `nand` | Builder-only composition |
| `nor` | Builder-only composition |
| `xnor` | Builder-only composition |
| `level_gate` | Primitive alias to `All` |
| low-enabled gate variants | Not separately supplied; explicit inversion plus canonical gate |
| `majority` | Primitive alias to `AtLeast` |
| `any_pulse` | Builder-only `Merge` plus `Coalesce` |
| `annotate_signal` | Metadata-only |
| `Exactly` | Standard module |
| `AtMost` | Standard module |
| `AllEqual` | Standard module |
| pulse-resettable toggle | Standard module |
| level-resettable toggle | Standard module |
| level-resettable sample-and-hold | Standard module |
| pulse multiplication | Deferred |
| pulse cap | Deferred |
| pulse parity | Deferred |
| multi-way selection and routing | Deferred |
| pulse-resettable sample-and-hold | Deferred |
| one-shots, timeout, watchdog, stretcher, limiter | Deferred |

# Appendix D — Design consequences

The initial catalogue is intentionally conservative.

Its combinational modules add public threshold and equality concepts that are materially clearer than their primitive formulas.

Its stateful modules demonstrate that useful resettable behavior can be composed without adding optional reset ports to primitives, while preserving exact same-reaction priority and inspectable internal state.

Its temporal omissions are deliberate correctness decisions. A later primitive or constrained temporal-state facility may make those modules admissible, but the standard catalogue does not pretend that approximate compositions are equivalent to precise retriggering, cancellation, rearming, or rate limiting.
