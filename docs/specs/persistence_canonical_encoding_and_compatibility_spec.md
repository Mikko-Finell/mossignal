# `mossignal` Persistence, Canonical Encoding, and Compatibility Specification

**Status:** Design specification, version 1  
**Defines:** Persistable semantic artifacts, canonical binary encoding, digest and fingerprint construction, time-domain identity, snapshot representation, restoration validation, replay records and logs, topology-patch persistence, provenance encoding, schema and semantic compatibility, decoding limits, and persistence verification obligations  
**Does not define:** General processor execution, ordinary built-in node behavior, the exhaustive diagnostic-code catalogue, file-system or database APIs, network transport protocols, compression formats, encryption, digital-signature policy, caller-owned binding serialization, editor project formats, or automatic semantic migration between incompatible versions

---

## 1. Purpose

This specification defines how `mossignal` semantic artifacts are converted into durable bytes and recovered without depending on private runtime representation.

Persistence must preserve the guarantees established by the API, built-in node, processor, reconfiguration, and verification specifications:

- deterministic execution;
- exact caller-owned logical time;
- stable structural identity;
- explicit machine lifecycle;
- complete future-determining snapshots;
- state-preserving compatible restoration;
- exact pending-work accounting;
- structured causal provenance;
- persistent diagnostic episodes;
- replay equivalence;
- canonical fingerprints and digests;
- structured rejection of malformed or incompatible data.

The central rule is:

> Persist semantic facts by stable identity, never private execution representation.

The second central rule is:

> Decoding bytes is not equivalent to accepting an artifact. Every decoded artifact must pass the structural, semantic, compatibility, and integrity checks applicable to its kind.

---

## 2. Normative language

The terms **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.

A **canonical payload** is the unique byte representation assigned by this specification to one versioned artifact value.

A **schema version** defines the fields and variants of one persisted artifact family.

A **semantic version component** identifies the observable laws used to interpret persisted facts.

An **encoding version** identifies the canonical value grammar and record representation.

A **digest suite** identifies the hash algorithm, output width, and domain-separation rules.

A **schema upgrade** changes persisted representation without changing represented semantics.

A **semantic migration** changes the meaning or behavior of an artifact and is never an ordinary restoration operation.

---

# Part I — Persistence model and boundaries

## 3. Persistence axioms

The persistence design follows these axioms.

### 3.1 Stable-keyed representation

Every structural or runtime fact that refers to authored structure uses stable keys.

Dense indices, slots, arena positions, pointer identities, and cache entries are never authoritative persisted identity.

### 3.2 Complete future state

A machine snapshot contains every semantic fact required to determine future execution and required future observation.

If an implementation introduces new machine state capable of affecting future execution, diagnostics, public event identity, or required explanation, that state must be added to the snapshot and the applicable digest scope.

### 3.3 Canonical bytes

For one supported artifact value and one complete version vector, exactly one canonical byte sequence exists.

The decoder rejects alternative encodings rather than silently normalizing them.

### 3.4 Explicit compatibility

Compatibility is determined by declared version rules and artifact validation.

The implementation must not infer compatibility from successful parsing, similar field names, matching Rust layouts, or best-effort deserialization.

### 3.5 Integrity is not authenticity

Canonical digests detect accidental corruption and support deterministic identity.

They do not establish who created an artifact. Authentication, authorization, signatures, and key management belong to a higher layer.

## 4. Artifact classes

The initial persistence layer defines canonical encodings for:

```text
ModuleDefinitionArtifact
NetworkDefinitionArtifact
RuntimePolicyArtifact
InputSnapshotArtifact
InputDeltaArtifact
NetworkPatchArtifact
TransactionRecordArtifact
MachineSnapshotArtifact
ReplayFrameArtifact
ReplayLogArtifact
TransactionResultArtifact
MigrationReportArtifact
CheckpointBundleArtifact
```

`TransactionResultArtifact` and `MigrationReportArtifact` are audit artifacts. They do not become machine state merely because they decode successfully.

## 5. Canonical persisted forms versus in-memory types

The persisted form of a public value need not match its private Rust layout.

Examples:

```text
ValidatedNetwork<D>   -> NetworkDefinitionArtifact
CompiledNetwork<D>    -> NetworkDefinitionArtifact
Machine<D>            -> MachineSnapshotArtifact
ForecastState<D>      -> MachineSnapshotArtifact
NetworkPatch<D>       -> NetworkPatchArtifact
Transaction<D>        -> TransactionRecordArtifact
TransactionResult<D>  -> TransactionResultArtifact
```

A persisted definition decodes to an unchecked semantic definition and must be validated again.

A persisted transaction containing a topology patch decodes to a transaction record and must re-prepare the patch against the actual base topology before it becomes executable.

## 6. Values not serialized directly

The following are not canonical persistence artifacts:

```text
ValidatedNetwork<D> as a trusted validated object
CompiledNetwork<D> private execution data
Machine<D> private runtime storage
PreparedPatch<D> private compiled target and migration program
resolved handles
compiled inspection plans
worklists and dirty sets
subscriber and observer state
borrowed graph or inspection views
allocator pointers or heap topology
```

Equivalent source artifacts may be persisted instead.

For example, a prepared patch is persisted as its normalized `NetworkPatchArtifact`, base binding, migration directives, and expected target fingerprint. It is re-prepared after decoding.

## 7. Semantic, observational, and archival data

Persisted facts are classified as:

### 7.1 Execution-semantic data

Data capable of changing future execution or whether a future transaction succeeds.

Examples include external levels, node state, pending work, event-identity allocation state, active diagnostic episodes, and topology revision.

### 7.2 Required observational data

Data required to reproduce current inspection, explanations, output baselines, or causal roots.

Examples include settled current values, required provenance derivations, checkpoint boundaries, and current diagnostic evidence.

### 7.3 Optional archival data

Data retained for history but not required for future execution or complete current observation.

Examples include old completed diagnostic episodes, prunable provenance ancestry beyond an authoritative checkpoint, and retained recent pulse history.

The three categories map to the execution-state, observable-state, and snapshot digest scopes defined later.

## 8. Persistence trust boundary

All decoded bytes are untrusted.

A decoder may produce a syntactically decoded artifact value, but only the appropriate validator may establish:

```text
valid network definition
valid runtime policy
valid input schema binding
valid topology patch
restorable machine snapshot
replay-compatible frame or log
valid audit artifact
```

No decoded value may bypass the ordinary validation, compilation, restoration, or patch-preparation paths merely because it was produced by a previous `mossignal` version.

---

# Part II — Persistent time-domain identity

## 9. Need for runtime time-domain identity

The Rust marker `D` prevents accidental mixing of time domains within one process, but Rust type identity is not present in persisted bytes.

Every persisted artifact parameterized by `D` must therefore carry an explicit stable time-domain identity.

## 10. `TimeDomainId`

The persistence layer defines:

```rust
pub struct TimeDomainId(/* opaque 128-bit value */);
```

A `TimeDomainId`:

- is supplied and owned by the caller;
- identifies the intended interpretation of one logical tick;
- is stable across processes and releases where artifacts are expected to remain compatible;
- is independent of Rust type names, crate paths, compiler versions, and `TypeId`;
- requires no global registry or singleton allocator.

The initial canonical representation is exactly 16 bytes in unsigned big-endian order.

## 11. Persistence context

Encoding and decoding `D`-parameterized artifacts requires an explicit context broadly equivalent to:

```rust
pub struct PersistenceContext<D> {
    pub time_domain: TimeDomainId,
    marker: PhantomData<fn() -> D>,
}
```

The context associates the process-local marker `D` with one persistent `TimeDomainId`.

The canonical API must not require `D` to implement a persistence trait.

## 12. Time-domain compatibility

Decoding into `D` requires exact equality between:

```text
artifact TimeDomainId
context TimeDomainId
```

A mismatch is a structured `WrongTimeDomain` failure.

Names such as `SimulationTicks`, labels such as `milliseconds`, or diagnostic metadata must not be used as compatibility evidence.

## 13. Time-domain contribution to identity

`TimeDomainId` contributes to:

- network and module fingerprints;
- machine snapshot compatibility;
- transaction and patch binding;
- replay compatibility;
- all `D`-dependent canonical digests.

This prevents two numerically identical temporal networks from being treated as the same semantic network when their caller-defined tick domains differ.

---

# Part III — Version and compatibility model

## 14. Version vector

Every artifact envelope carries a complete version vector.

The initial vector contains:

```text
EnvelopeSchemaVersion
CanonicalEncodingVersion
DigestSuiteVersion
ArtifactSchemaVersion
CoreSemanticsVersion
BuiltInNodeSemanticsVersion
TopologyPatchSemanticsVersion
ProvenanceSemanticsVersion
DiagnosticSchemaVersion
```

Artifact kinds that do not use one component still carry it so compatibility decisions are explicit and uniform.

## 15. Version representation

Each version component is an unsigned 32-bit identifier.

Numeric ordering alone has no compatibility meaning.

Compatibility is defined by an explicit support table maintained by each library release.

The implementation must not assume that version `n + 1` is readable or semantically compatible merely because it is numerically adjacent to `n`.

## 16. Version responsibilities

### 16.1 Envelope schema

Defines the top-level artifact envelope fields.

### 16.2 Canonical encoding

Defines the deterministic value grammar, record shape, collection ordering, and primitive encodings.

### 16.3 Digest suite

Defines the hash algorithm, output width, and domain-separation inputs.

### 16.4 Artifact schema

Defines fields and variants for one artifact family.

Different artifact kinds may evolve their schemas independently, but the envelope stores one schema version specific to its `artifact_kind`.

### 16.5 Core semantics

Defines lifecycle, reaction, transaction, time, state, and output semantics shared by the system.

### 16.6 Built-in node semantics

Defines node laws, state schemas, pending-event payload meanings, and node compatibility behavior.

### 16.7 Topology-patch semantics

Defines patch operations, correspondence, migration directives, and state-loss rules.

### 16.8 Provenance semantics

Defines derivation record meanings, checkpoint rules, required roots, and retention boundaries.

### 16.9 Diagnostic schema

Defines stable diagnostic codes, structured evidence forms, episode identity, and persisted diagnostic shapes.

Rendered diagnostic wording is not versioned semantic data.

## 17. Compatibility levels

The persistence layer distinguishes:

```text
SyntacticDecode
SchemaCompatible
SemanticCompatible
RestorationCompatible
ReplayCompatible
ExactArtifactRoundTrip
```

These levels are strictly stronger in the order shown.

Successful syntax decoding does not imply any stronger compatibility level.

## 18. Syntactic decoding

Syntactic decoding requires:

- valid envelope prefix;
- supported envelope schema;
- canonical value encoding;
- valid primitive lengths and UTF-8;
- internally well-formed envelope fields;
- matching integrity digest.

It does not establish that the payload is structurally valid or semantically usable.

## 19. Schema compatibility

Schema compatibility requires a reader for the exact artifact schema version or an explicit representation-only upgrader from that version.

Unknown required fields, variants, or record shapes reject the artifact.

## 20. Semantic compatibility

Semantic compatibility requires every applicable semantic version component to be accepted by an explicit compatibility rule.

A reader may accept an older artifact schema while rejecting its older node semantics.

## 21. Restoration compatibility

A machine snapshot is restoration-compatible only when all of the following hold:

```text
matching time domain
matching network identity and fingerprint
compatible topology revision rules
compatible state schemas
compatible semantic version vector
valid runtime policy relationship
complete and valid machine state
valid canonical digests
```

## 22. Replay compatibility

Replay compatibility additionally requires:

```text
compatible starting snapshot
exact transaction semantics
exact patch semantics for patch-bearing frames
matching RuntimePolicyId for exact replay
valid digest chain
valid frame sequence
```

## 23. Exact artifact round trip

Exact artifact round trip means decoding and re-encoding yields byte-for-byte identical canonical bytes.

It is required for every currently supported canonical artifact version.

An explicit schema upgrade produces a new artifact version and is not an exact round trip of the original version.

## 24. Schema upgrades

A schema upgrader must be:

- explicit;
- deterministic;
- pure over the decoded artifact;
- version-to-version specific;
- semantics-preserving;
- covered by golden vectors and historical artifact tests.

The upgrader must first verify the old artifact under its own encoding and digest rules.

After conversion, current-version digests and fingerprints are recomputed.

## 25. Semantic migrations

A change in represented behavior is not a schema upgrade.

Ordinary snapshot restoration and replay must not silently:

- reinterpret old node laws under new laws;
- reset incompatible state;
- change pending-event deadlines;
- alter pulse multiplicity;
- change conflict policy;
- rewrite provenance meaning;
- discard diagnostic episode state.

A semantic migration requires a separately named explicit operation and a migration report. The initial core may reject such artifacts instead of providing a migrator.

## 26. No implicit downgrade

The library must not emit an older schema or semantic version unless an explicit downgrade implementation exists and proves that the current artifact is representable without semantic loss.

Dropping unknown current fields to satisfy an older schema is forbidden.

## 27. Compatibility support table

Each release must publish and test a table identifying, per artifact kind and version vector:

```text
Decode
Upgrade
Validate
Restore
Replay
Reject
```

Unsupported combinations must fail with the exact incompatible component and encountered version.

---

# Part IV — Canonical binary encoding

## 28. Encoding foundation

The initial wire representation is a strict deterministic CBOR profile.

The profile uses established CBOR primitive encodings while forbidding representation choices that would permit multiple byte strings for one artifact.

The canonical artifact bytes are:

```text
fixed mossignal prefix
+ canonical CBOR envelope
```

## 29. Artifact prefix

Every standalone artifact begins with these eight bytes:

```text
4d 53 49 47 0d 0a 1a 0a
 M  S  I  G CR LF SUB LF
```

The prefix is not part of any semantic digest.

Embedded payload APIs may omit the prefix only when the enclosing protocol already provides unambiguous framing and explicitly declares that the body is a `mossignal` canonical envelope.

## 30. Allowed canonical values

The canonical profile permits only:

```text
unsigned integers
byte strings
UTF-8 text strings
arrays of definite length
false
true
null
```

The profile forbids:

```text
negative integers
floating-point values
indefinite-length strings or arrays
CBOR maps
CBOR tags
undefined and other simple values
non-shortest integer or length encodings
```

All lengths and integer values use the shortest valid CBOR representation.

## 31. Records

A record is encoded as an array of two-element field entries:

```text
[
    ["field_a", value_a],
    ["field_b", value_b],
    ...
]
```

Field names are schema-defined lowercase ASCII `snake_case` strings.

Entries are ordered by ascending raw UTF-8 byte sequence of the field name.

A field may appear at most once.

A required field must be present exactly once.

Optional absent fields are omitted unless the schema explicitly requires a present `null` value.

## 32. Variants

A closed enum variant is encoded as:

```text
["variant_name", payload]
```

The variant name is a schema-defined lowercase ASCII `snake_case` string.

A payloadless variant uses `null` as its payload.

Variant names are permanent within one schema version.

## 33. Options

An optional value is represented by field absence whenever the containing record schema permits omission.

Where an option appears inside a sequence or another non-record position:

```text
null        means None
non-null    means Some(value)
```

No semantic type in the initial schemas uses `null` as an ordinary non-optional value.

## 34. Sequences

A semantically ordered sequence is encoded in semantic order.

Examples include:

- replay frames;
- processed logical times;
- hierarchical diagnostic paths;
- ordered transport transition precedence by originating time where explicitly semantic.

## 35. Sets and maps

The canonical profile does not use CBOR maps.

A semantic mapping is encoded as a sorted array of key-value pairs:

```text
[
    [key_1, value_1],
    [key_2, value_2],
    ...
]
```

Keys are ordered by the domain-specific canonical order defined by the schema.

A semantic set is encoded as a sorted array without duplicates.

A semantic multiset is encoded as a sorted array of value-count pairs. Duplicate multiplicity must not be represented by accidental repeated entries when a count representation exists.

## 36. Canonical ordering

The default order for fixed-width opaque identities is lexicographic order of their canonical bytes.

The default order for heterogeneous structural subjects is:

```text
subject kind tag
then stable key bytes
then schema-defined role when required
```

For the initially supported `SubjectRef` variants, the concrete Rust API
specification's **Initial canonical diagnostic-subject order** is the
schema-specific order. Its comparison tags order values but are not persisted
enum representations. This reference does not define payload ordering for the
remaining `SubjectRef` variants and does not replace the generic ordering rules
for other schemas.

The default order for time-indexed facts is:

```text
logical time
then semantic subject order
then schema-defined variant order
then stable event identity where applicable
```

The default order for otherwise unordered complete canonical values is lexicographic order of their canonical encoded bytes.

Schema-specific order overrides the default where the order itself has semantic meaning.

## 37. Text

Text is valid UTF-8 and is persisted byte-for-byte.

The encoder and decoder must not apply Unicode normalization, case folding, whitespace normalization, locale transformation, or line-ending conversion.

Presentation metadata excluded from semantic fingerprints may still affect complete artifact and snapshot digests when it is present in those artifacts.

## 38. Stable keys

Every 128-bit stable key is encoded as one byte string of length 16 in unsigned big-endian order.

The key type is determined by the containing schema and must not be inferred from numeric value.

Typed keys of different categories may contain equal 16-byte payloads without becoming the same subject.

## 39. Time, span, pulse count, and revision

The initial canonical forms are:

```text
Time<D>          unsigned integer in [0, 2^64 - 1]
Span<D>          unsigned integer in [0, 2^64 - 1]
NonZeroSpan<D>   unsigned integer in [1, 2^64 - 1]
PulseCount       unsigned integer in [0, 2^64 - 1]
NetworkRevision unsigned integer in [0, 2^64 - 1]
```

Checked arithmetic rules remain unchanged after decoding.

## 40. Fixed digests and fingerprints

Every initial digest and fingerprint is encoded as one byte string of length 32.

Digest types remain distinct through schema position and domain-separated construction even though their byte widths match.

## 41. Diagnostic codes and enum names

Stable diagnostic codes are encoded by their stable machine-readable code string, not rendered summary text.

Closed semantic enums use schema variant names, not Rust discriminant numbers, debug formatting, or source declaration position.

## 42. Unknown fields and extensions

Within a known schema version, unknown fields and variants are errors unless they occur inside an explicitly declared extension field.

The initial general extension shape is:

```text
presentation_extensions: [
    [namespace_text, canonical_byte_string],
    ...
]
```

Presentation extensions:

- are opaque to the core;
- are ordered by namespace text;
- are included in the complete artifact integrity digest;
- are excluded from semantic fingerprints and execution-state digests;
- must not affect restoration, execution, migration, or replay.

Unknown semantic extensions are not accepted.

## 43. Canonical decoding

The decoder must reject:

- non-shortest integers or lengths;
- forbidden CBOR types;
- indefinite lengths;
- duplicate record fields;
- unsorted record fields;
- unsorted semantic sets or maps;
- invalid UTF-8;
- wrong fixed byte lengths;
- unknown required variants;
- trailing bytes after one standalone artifact;
- a valid value whose re-encoding differs from the received canonical body.

The final re-encoding check may be replaced by an equivalent streaming canonicality proof, but canonical acceptance must be identical.

---

# Part V — Artifact envelope and integrity

## 44. Envelope shape

The canonical CBOR body is one variant:

```text
["mossignal_artifact", envelope_record]
```

The envelope record contains:

```text
artifact_kind
artifact_schema_version
canonical_encoding_version
core_semantics_version
diagnostic_schema_version
digest_suite_version
envelope_schema_version
integrity_digest
node_semantics_version
patch_semantics_version
payload
provenance_semantics_version
time_domain_id
```

Fields are encoded in canonical record order.

`time_domain_id` is `null` only for a future artifact kind explicitly declared independent of `D`. Every initial artifact kind carries a 16-byte value.

### 44.1 Embedded artifacts

When one artifact contains another artifact family, the child is embedded as a byte string containing the child's complete standalone canonical bytes, including its fixed prefix and envelope.

Examples include:

```text
transaction record -> input snapshot or delta and optional network patch
replay frame       -> transaction record and optional transaction result
replay log         -> replay frames
checkpoint bundle  -> definition, policy, snapshot, and optional replay log
```

Each child therefore retains its own schema version and integrity digest and is validated independently before cross-artifact consistency is checked.

An implementation may avoid copying nested bytes internally, but the canonical external representation remains the complete child artifact byte string.

## 45. Artifact kinds

The initial `artifact_kind` strings are:

```text
module_definition
network_definition
runtime_policy
input_snapshot
input_delta
network_patch
transaction_record
machine_snapshot
replay_frame
replay_log
transaction_result
migration_report
checkpoint_bundle
```

An unknown artifact kind is a structured failure.

## 46. Envelope integrity digest

The envelope contains an `integrity_digest` over the same envelope with that field omitted.

The digest domain depends on artifact kind:

```text
machine_snapshot -> mossignal/snapshot_digest/v1
all other kinds  -> mossignal/artifact_integrity/v1
```

Conceptually:

```text
IntegrityDigest =
    Hash(selected_domain, CanonicalEncode(envelope_without_integrity_digest))
```

The fixed prefix is excluded.

The decoder verifies this digest before expensive semantic validation where practical.

## 47. Snapshot digest placement

For a `machine_snapshot` artifact, the envelope `integrity_digest` is the public `SnapshotDigest` and is computed under the snapshot-specific digest domain.

The snapshot payload does not contain a second self-referential digest field.

An in-memory `MachineSnapshot<D>` exposes the verified envelope digest as its `SnapshotDigest`.

## 48. Content identity of other artifacts

Other artifact kinds may expose the envelope integrity digest as a generic persisted artifact content identifier, but it must not be confused with:

```text
NetworkFingerprint
ModuleFingerprint
RuntimePolicyId
ExecutionStateDigest
ObservableStateDigest
SnapshotDigest
```

Those identities use their own semantic projections and domain labels.

## 49. Compression and wrappers

The canonical v1 artifact body is uncompressed.

Compression may be applied by an outer storage or transport layer, but:

- decompressed bytes must be exactly one canonical artifact;
- semantic digests are computed over the canonical uncompressed representation;
- compression parameters do not affect semantic identity;
- the core decoder need not accept compressed input.

## 50. Encryption and signatures

Encryption, signatures, certificates, MACs, and access-control metadata are external wrappers.

A signature layer should sign the complete canonical artifact bytes, including the fixed prefix and integrity digest.

The core must not present an integrity digest as proof of authenticity.

---

# Part VI — Digest and fingerprint suite

## 51. Initial digest suite

`DigestSuiteVersion = 1` uses unkeyed BLAKE3 with a 256-bit output.

Every digest input is a canonical record containing an explicit ASCII domain label and the relevant version and semantic payload.

Conceptually:

```text
Digest = BLAKE3_256(CanonicalEncode([
    ["domain", domain_label],
    ["payload", payload],
    ["version", domain_version],
]))
```

The exact record order follows the canonical encoding rules.

## 52. Domain separation

Distinct digest types must use distinct domain labels.

The initial labels are:

```text
mossignal/artifact_integrity/v1
mossignal/module_fingerprint/v1
mossignal/network_fingerprint/v1
mossignal/runtime_policy_id/v1
mossignal/execution_state_digest/v1
mossignal/observable_state_digest/v1
mossignal/snapshot_digest/v1
mossignal/provenance_record/v1
mossignal/replay_log_content/v1
mossignal/transaction_result_content/v1
mossignal/network_patch_content/v1
```

No digest type may reuse another type's label merely because their current payloads are similar.

## 53. Digest version inputs

Every semantic digest payload includes the version components that define interpretation of its fields.

A schema-only representation change need not change a semantic digest if the semantic projection, digest suite, and digest-domain version remain unchanged.

A change to canonical semantic projection requires a new domain version or digest suite version.

## 54. Network fingerprint

`NetworkFingerprint` is computed from a canonical semantic projection containing:

```text
time_domain_id
network_key
core semantics version
built-in node semantics version
patch-relevant structural semantics version where applicable
stable node keys and kinds
stable typed port keys and semantic roles
stable connection keys and endpoints
external endpoint keys and sources
module-instance identity and state-relevant hierarchy
semantic node parameters
state schemas and declared initial state
signal-semantics and timing parameters
```

It excludes:

```text
topology revision
dense indices
construction and insertion order
memory layout
diagnostic names and descriptions
diagnostic paths and origin metadata
presentation tags and extensions
compiled caches
```

Semantically equivalent stable-keyed definitions must produce the same fingerprint.

## 55. Module fingerprint

`ModuleFingerprint` follows the same rules for one reusable module definition and includes module interface keys and module-internal stable identity.

It excludes module-instance placement and instance metadata.

## 56. Runtime policy identifier

`RuntimePolicyId` is computed from every policy field capable of changing whether an operation succeeds or fails.

The canonical projection includes named limits rather than builder order.

Performance-only tuning that cannot affect semantic outcomes is excluded.

## 57. Execution-state digest

The execution-state digest covers every fact capable of affecting future execution, future structured failure, persistent diagnostic publication, or public pending-event identity.

Its canonical projection includes:

```text
time_domain_id
network fingerprint
machine lifecycle
current topology revision
current logical time when ready
authoritative external level valuation when ready
stateful-node state
temporal-node state
pending event calendar
active diagnostic episodes and current material evidence
next public pending-event serial
other future public-identity allocation state, if introduced
```

It excludes:

```text
runtime policy identity
current required provenance derivations
optional history
presentation metadata
subscriber state
private allocation and cache state
```

`RuntimePolicyId` remains a separate component of exact operational replay identity.

## 58. Observable-state digest

The observable-state digest extends the execution-state projection with complete current required observation:

```text
settled current level facts
external output baseline and establishment state
required current provenance roots
canonical required provenance records or checkpoint roots
current explanation checkpoint boundary
current diagnostic evidence
required current inspection facts not derivable from the execution projection
```

Optional history outside the required current closure remains excluded.

## 59. Snapshot digest

`SnapshotDigest` covers the complete versioned machine snapshot artifact, including:

```text
execution state
required observable state
optional retained provenance history
optional retained diagnostic or pulse history
checkpoint metadata
persistence metadata and presentation extensions
artifact and semantic version vector
```

It excludes only its own digest field, represented by the omitted envelope `integrity_digest` during hashing.

Two snapshots may share execution and observable digests while differing in optional retained history and therefore have different snapshot digests.

## 60. Digest equality and collision handling

Digest equality is a strong practical consistency check, not a mathematical proof of semantic equality.

Whenever two canonical records with the same digest are simultaneously available and their bytes differ, the implementation must treat the condition as a fatal digest collision or corruption failure. It must not choose one record silently.

---

# Part VII — Canonical provenance persistence

## 61. Provenance as a Merkle DAG

Persisted provenance uses a content-addressed Merkle DAG.

Each derivation record receives a `CauseDigest` computed from:

```text
record kind
logical time and revision where applicable
structural subject
semantic payload
labeled predecessor relations
predecessor CauseDigests
supporter multiplicities or contribution counts
provenance semantics version
```

Private arena identifiers and in-memory `CauseRef` values do not participate.

## 62. Labeled predecessor relations

Each predecessor edge carries a semantic role such as:

```text
previous_state
current_input
selected_branch
due_obligation
scheduling_origin
migration_source
checkpoint_premise
supporter
blocker
```

Relations with different roles remain distinct even when they reference the same cause.

Unordered supporters within one role are sorted by:

```text
role
predecessor CauseDigest
contribution payload
```

Ordered semantic relations retain their specified order.

## 63. Canonical provenance record order

A snapshot or result artifact stores provenance records sorted by `CauseDigest` bytes.

Every referenced predecessor digest must resolve to:

- another record in the same artifact;
- an explicit authoritative checkpoint record in the same artifact; or
- an externally declared checkpoint dependency of a bundle schema that explicitly permits it.

Ordinary machine snapshots are self-contained and do not use unresolved external provenance records.

## 64. Coalescing equivalent derivations

Two provenance nodes with byte-identical canonical record content have the same `CauseDigest` and may be stored once.

This is semantically valid because private provenance record identity is non-semantic and explanations are equivalent up to renaming of such identities.

Contributor multiplicity must remain explicit and must not be lost through coalescing.

## 65. Provenance validation

Restoration validates:

- every record digest;
- every predecessor reference;
- acyclicity;
- required root closure;
- role validity for the record kind;
- subject existence and revision compatibility;
- checkpoint authority and retention boundary;
- absence of conflicting records sharing one digest.

The implementation must not trust persisted topological order.

## 66. Cause references in other records

A persisted cause reference is one 32-byte `CauseDigest`.

During restoration, `CauseDigest` values are resolved into private in-memory `CauseRef` handles after the complete provenance graph has validated.

---

# Part VIII — Definition artifacts

## 67. Network definition artifact

A `network_definition` payload contains:

```text
network_key
time_domain_id confirmation
complete nodes and typed ports
complete connections
external inputs and outputs
module instances and hierarchy
semantic parameters
declared initial state
diagnostic metadata
presentation extensions
optional validation claim
```

Collections are stable-key sorted.

A malformed dynamic definition remains representable when its individual fields are encodable.

## 68. Validation claim

A definition artifact may contain:

```text
validation_claim = null
```

or:

```text
validation_claim = {
    network_fingerprint,
    validated_under_versions
}
```

The claim is advisory until revalidation and recompilation reproduce the fingerprint under the claimed versions.

A mismatched claim is a structured corruption or stale-claim failure.

The decoder must still be able to expose the unchecked definition and diagnostics through an explicit inspection API where safe.

## 69. Module definition artifact

A `module_definition` payload contains the complete reusable module interface, internal stable-keyed definition, hierarchy, metadata, semantic parameters, and optional validation claim.

Module internals use their module-local stable keys. Instance qualification is not added until module instantiation in a network.

## 70. Persisting validated or compiled networks

Encoding a `ValidatedNetwork<D>` or `CompiledNetwork<D>` emits the canonical network definition plus a validation claim.

It does not serialize:

```text
dense indices
topological ranks
SCC work arrays
compiled state slots
adjacency storage
query indices
private fingerprints caches
```

Decoding produces an unchecked definition, then ordinary validation and compilation reconstruct derived data.

## 71. Definition ordering

Definition collection order is:

```text
modules by ModuleInstanceKey
nodes by NodeKey
ports by typed port kind and stable key
connections by ConnectionKey
external inputs by signal kind and stable key
external outputs by signal kind and stable key
```

For malformed duplicate keys, ties are broken by the complete canonical record bytes so encoding remains deterministic and validation can report the duplicates.

## 72. Metadata

Diagnostic metadata is encoded completely and byte-preservingly.

Metadata is excluded from network and module fingerprints unless a future field is explicitly reclassified as semantic.

Metadata remains included in definition artifact integrity digests.

## 73. Authoring allocator state

Caller-owned `KeyAllocator` state is not part of network semantics and is not required in a definition artifact.

An editor may store allocation hints in a namespaced presentation extension.

Such hints must not affect network fingerprinting, validation, compilation, or restoration.

---

# Part IX — Runtime policy, input, transaction, and patch artifacts

## 74. Runtime policy artifact

A `runtime_policy` payload contains every semantically relevant named limit and its computed `RuntimePolicyId`.

Decoding rebuilds the policy through the ordinary validated policy constructor and verifies the identifier.

Unknown limits in a known schema version are rejected.

## 75. Input schema binding

An input snapshot or delta artifact contains:

```text
network_key
network fingerprint
input schema fingerprint
target topology revision context
stable typed endpoint observations
```

A target-bound patch input also contains the expected target fingerprint and target input-schema fingerprint.

The decoder must not bind observations by collection position.

## 76. Input snapshot artifact

An `input_snapshot` payload contains exactly one level value for every required external level input in its bound schema.

Pulse inputs are represented only when their count is positive. Absence means zero.

The canonical encoder sorts level and pulse entries by signal kind and stable endpoint key.

Completeness is revalidated after decoding.

## 77. Input delta artifact

An `input_delta` payload contains changed or explicitly observed external levels and positive pulse counts.

Unmentioned levels retain prior authoritative values only when applied to a compatible ready machine.

The artifact itself does not prove lifecycle compatibility.

## 78. Network patch artifact

A `network_patch` payload contains:

```text
network_key
base fingerprint
base revision
normalized canonical operations
all reassociations
all node and module migration directives
all temporal pending-work policies
all target-input establishment consequences derivable from structure
patch semantic version
optional expected target fingerprint
```

Operations are encoded in the canonical normalized order defined by the topology-patch specification, not builder insertion order.

Every canonical patch operation and migration enum has one closed persisted variant.

## 79. Patch content digest

A patch artifact may expose a `NetworkPatchContentDigest` using the `mossignal/network_patch_content/v1` domain.

It covers the normalized semantic patch content and base binding but excludes envelope presentation extensions.

Equivalent operation permutations must produce the same content digest.

## 80. Prepared patches

`PreparedPatch<D>` is not serialized directly.

A persisted patch-bearing workflow stores:

```text
normalized NetworkPatchArtifact
base fingerprint and revision
expected target fingerprint
reconfiguration policy when transaction-specific
```

After decoding, the patch is prepared again through the ordinary complete preparation path.

The resulting target fingerprint must equal the persisted expected target fingerprint when one is present.

No persisted migration table, compiled target, dense handle, or static compatibility cache is trusted as executable preparation.

## 81. Transaction record artifact

A `transaction_record` payload contains:

```text
requested logical time
expected topology revision
optional expected ExecutionStateDigest
input snapshot or input delta variant
optional patch attachment
transaction metadata
network and input-schema binding
```

The patch attachment contains:

```text
NetworkPatchArtifact
expected target fingerprint
ReconfigurationPolicy
```

It does not contain private `PreparedPatch<D>` bytes.

## 82. Transaction materialization

A decoded transaction record becomes an executable `Transaction<D>` only after materialization against a machine or compiled topology context.

Materialization validates:

- network and time-domain identity;
- current base fingerprint and revision;
- input schema binding;
- lifecycle shape;
- patch preparation and target fingerprint;
- transaction build invariants.

Patch-free records may materialize without compilation work when their schema binding remains valid.

## 83. Transaction metadata

`TransactionMeta` is persisted exactly because it may appear in provenance roots, diagnostics, and replay audit records.

It remains excluded from evaluator behavior and execution-state digest.

A metadata difference may change observable or snapshot digests when it changes required provenance content.

---

# Part X — Machine snapshot schema

## 84. Snapshot contract

A machine snapshot is a complete stable-keyed checkpoint of one committed machine version.

It must satisfy the machine-level Markov property:

> Given an equivalent restored snapshot, a compatible runtime policy, and the same future compatible transaction sequence, future behavior is independent of omitted earlier history.

## 85. Snapshot common fields

Every `machine_snapshot` payload contains:

```text
network_key
network_fingerprint
time_domain_id
topology_revision
lifecycle
semantic version vector
execution_state_digest
observable_state_digest
runtime_policy_id
next_pending_event_serial
node_state_table
temporal_state_table
active_diagnostic_episodes
provenance
optional_history
persistence_metadata
```

Lifecycle-specific fields are carried inside the `lifecycle` variant.

## 86. Uninitialized snapshot

The uninitialized lifecycle variant contains:

```text
declared stateful-node state
declared temporal-node state
next pending-event serial
```

It contains no fabricated:

```text
current logical time
external level valuation
settled current port values
external output baseline
pending event schedule
runtime provenance roots
runtime diagnostic episodes
schedule
```

If a future implementation allows a pre-initialization semantic fact not listed here, that fact must be versioned and persisted explicitly.

## 87. Ready snapshot

The ready lifecycle variant contains:

```text
current logical time
authoritative external level valuation
complete settled current level facts
external output baselines and establishment state
pending event calendar
required current provenance roots
current explanation checkpoint state
```

The schedule is derived from the pending event calendar and is not authoritative persisted state.

## 88. Stable-keyed state tables

Node and temporal state are encoded as sorted arrays keyed by `NodeKey`.

Each entry contains:

```text
node key
expected node kind
state schema variant
state value
state cause where required
state revision context where required
```

The node kind and state schema are redundant with the compiled topology by design. Restoration verifies them before installing private state slots.

## 89. Complete settled level facts

A ready snapshot contains settled level facts for every inspectable current level port and endpoint required by the public inspection contract.

The initial shape includes:

```text
external level inputs
node level input ports
node level output ports
external level outputs
```

Facts are keyed by typed stable identity.

These values are validated against connectivity, node state, and complete reference reevaluation during restoration.

The implementation must not trust them as an alternative execution path.

## 90. Pulse state

Pulse counts do not persist as current port state after a reaction.

Optional retained recent pulse history may appear only in `optional_history`, with an explicit retention boundary and logical time.

It does not contribute to execution-state or observable-state digest unless a future public inspection contract reclassifies it as required current observation.

## 91. Built-in state schemas

The snapshot schema provides closed variants for every built-in state family.

The initial families include:

```text
edge detector observation: unestablished | established(level)
boolean stored level
pulse delay temporal state
transport delay remembered input and output
inertial delay remembered input, output, and optional candidate
periodic anchor, previous enable state, and next eligible boundary
```

Pending event records remain in the separate event calendar rather than being duplicated as opaque node-owned queues.

References from temporal state to pending events use public `PendingEventKey` values and are validated bidirectionally.

## 92. Pending event records

Each pending event record contains:

```text
PendingEventKey
owner NodeKey
event kind
deadline
semantic payload
multiplicity where applicable
originating logical time
originating revision
originating CauseDigest
migration-relevant metadata
```

The initial event-kind payloads are:

```text
pulse_delay_group:
    pulse count
    grouped causal contribution records

transport_transition:
    target LogicLevel
    originating logical time used for same-deadline precedence

inertial_maturation:
    target LogicLevel
    qualification origin

periodic_boundary:
    phase anchor
    non-negative boundary ordinal
```

The event kind must agree with the owning built-in node kind. Node temporal state refers to these records by `PendingEventKey` where a candidate or next boundary is singular.

Records are ordered by:

```text
deadline
owner node key
event kind
PendingEventKey
```

Every deadline in a ready snapshot must be strictly later than snapshot time.

Canceled events are absent from the pending set.

## 93. Public pending-event identity

The public persisted `PendingEventKey` is a machine-local unsigned 64-bit monotonic serial.

It is distinct from private arena slots and generation counters.

The machine never reuses a public serial within one machine history.

A snapshot persists `next_pending_event_serial`, which must be greater than every pending or retained public event serial capable of colliding with future allocation.

This cursor contributes to the execution-state digest because future public event identity is observable.

## 94. Private event storage reconstruction

Restoration may place events into any private arena, heap, ordered map, or compact calendar representation.

Private storage position must not change:

- public event keys;
- deadline order;
- equal-deadline batch semantics;
- inspection;
- replay;
- semantic digests.

## 95. External output baselines

For each external level output in a ready snapshot, the snapshot stores:

```text
output key
established marker
published baseline level
current cause root
```

A ready machine must have an established baseline for every current external level output.

An uninitialized machine has none.

Restoration validates the baseline against current settled output state.

## 96. Active diagnostic episodes

Each active episode record contains:

```text
episode identity
diagnostic code
primary stable subject
condition discriminator
began_at
last_material_change
current structured evidence
current cause roots where required
revision context
```

Episode identity is recomputed from its stable semantic identity rule and verified.

Completed historical episodes appear only in optional history.

## 97. Required provenance section

The required provenance section contains:

```text
content-addressed provenance records
roots for current stateful state
roots for pending events
roots for external output baselines
roots for active diagnostic evidence
checkpoint records and retention boundary
```

The complete backward closure of every required root must be present.

## 98. Optional history section

Optional history is explicitly partitioned into named retention classes, for example:

```text
additional provenance records
completed diagnostic episodes
recent pulse activity
recent transition summaries
```

Each class records its retention boundary and completeness claim.

Absence means no claim that the history exists.

Optional history must not be consulted by the evaluator.

## 99. Persistence metadata

A snapshot may carry non-semantic persistence metadata such as:

```text
created_by implementation identifier
human label
source correlation
presentation extensions
```

Wall-clock timestamps, when supplied by the caller, are metadata only and must not be confused with logical time.

Persistence metadata contributes to `SnapshotDigest` but not execution or observable digests.

## 100. Snapshot creation

Snapshot creation observes one complete committed machine version.

It must not race with partial transaction publication.

The canonical snapshot builder:

1. projects stable-keyed semantic state;
2. derives content-addressed provenance records;
3. computes execution and observable digests;
4. constructs optional history according to retention policy;
5. canonicalizes the payload;
6. computes the `SnapshotDigest` through the envelope integrity rule.

Snapshot creation must not mutate the machine.

## 101. Snapshot validation phases

Restoration performs these phases in order:

1. canonical envelope and integrity validation;
2. version and time-domain compatibility;
3. network identity and fingerprint compatibility;
4. lifecycle-shape validation;
5. stable subject and state-schema validation;
6. pending-event and allocator validation;
7. diagnostic episode validation;
8. provenance graph and root validation;
9. settled-value and output-baseline consistency validation;
10. digest recomputation;
11. construction of private candidate runtime storage;
12. complete invariant validation;
13. publication of the restored machine.

No public machine exists before all phases succeed.

## 102. Recomputed settled state

For a ready snapshot, restoration must use the full reference evaluator or an equivalent complete consistency check to establish that persisted current settled values agree with:

```text
compiled topology
external levels
stored node state
temporal state
pending obligations not yet due
```

This check performs no new logical reaction, emits no events, and changes no state. It verifies the represented settled fixed point of the acyclic reaction graph.

A mismatch is corruption or incompatibility, not an instruction to repair the snapshot.

## 103. Snapshot digest validation

The decoder recomputes:

```text
ExecutionStateDigest
ObservableStateDigest
SnapshotDigest
```

Every persisted claimed digest must match.

The implementation must identify which digest scope failed where possible.

## 104. Runtime policy restoration

The ordinary `CompiledNetwork::restore(snapshot, policy)` path requires:

```text
policy.id() == snapshot.runtime_policy_id()
```

A mismatch is `RuntimePolicyMismatch`.

A future explicit policy-rebinding API may allow restoration under another policy, but it must:

- be separately named;
- report that exact operational replay identity changed;
- recompute any bundle identity that includes policy;
- never claim continuity of an old replay chain.

## 105. No implicit topology migration

Ordinary restoration requires exact compatible network fingerprint and topology revision context.

It must not:

- apply a topology patch;
- reassociate keys;
- reset incompatible nodes;
- cancel pending events;
- translate node state across semantic kinds.

Such changes require explicit reconfiguration or a separately specified offline semantic migration.

## 106. Checkpoint bundle

A `checkpoint_bundle` is a portable self-contained artifact containing:

```text
NetworkDefinitionArtifact
RuntimePolicyArtifact
MachineSnapshotArtifact
optional ReplayLogArtifact beginning after the snapshot
bundle metadata
```

The bundle validates each component independently and then validates cross-component identity:

```text
time domain
network key
network fingerprint
runtime policy id
starting execution and observable digests
semantic versions
```

A snapshot alone remains valid and intentionally requires a compatible compiled network and policy supplied externally.

---

# Part XI — Replay persistence

## 107. Replay frame payload

A persisted replay frame contains:

```text
frame_index
expected_previous_execution_digest
expected_revision
runtime_policy_id
transaction_record
resulting_execution_digest
resulting_observable_digest
optional recorded transaction result
```

`resulting_observable_digest` is required in the persisted v1 frame even if an older in-memory convenience type exposes only the execution digest.

This field permits replay validation to detect divergence in required provenance, output baselines, and current explanation state.

## 108. Replay frame construction

A frame may be constructed only from a successful committed transaction or an explicitly recorded successful forecast whose hypothetical status remains marked.

Ordinary replay logs use committed transactions.

The frame records the exact transaction metadata and normalized patch content required to reproduce the transition.

## 109. Patch-bearing replay frames

A patch-bearing frame stores the normalized patch rather than private prepared data.

During replay:

1. verify expected current revision and fingerprint;
2. decode the normalized patch;
3. prepare it through the ordinary current implementation;
4. verify the expected target fingerprint;
5. materialize the target-bound input artifact;
6. apply the ordinary transaction.

Any preparation divergence is a replay compatibility failure.

## 110. Replay log payload

A replay log contains:

```text
log identity metadata
starting network key and fingerprint
starting topology revision
starting ExecutionStateDigest
starting ObservableStateDigest
RuntimePolicyId
semantic version vector
ordered replay frames
optional final expected digests
optional presentation metadata
```

Frame order is semantic and must not be sorted or otherwise normalized by content.

## 111. Replay chain validation

Before applying frame `i`, replay verifies:

```text
current execution digest == frame.expected_previous_execution_digest
current revision == frame.expected_revision
current runtime policy id == frame.runtime_policy_id
```

After applying it, replay verifies:

```text
actual resulting execution digest == frame.resulting_execution_digest
actual resulting observable digest == frame.resulting_observable_digest
```

When a recorded transaction result is present, its canonical content must also match the actual result after normalization of representation-only identifiers.

## 112. Replay log content digest

A replay log may expose `ReplayLogContentDigest` computed over:

```text
starting identities
ordered canonical frame content digests
final expected identities
semantic version vector
```

Presentation metadata is excluded from the content digest but included in the envelope integrity digest.

## 113. Replay concatenation

Two logs may be concatenated only when:

```text
first final execution digest == second starting execution digest
first final observable digest == second starting observable digest
first final revision == second starting revision
network fingerprint and time domain match
RuntimePolicyId matches
semantic version vectors are replay-compatible
```

Concatenation preserves frame order and reindexes only a non-semantic local display index if necessary.

A canonical frame's original sequence position may be retained as audit metadata.

## 114. Chunked replay

A large replay log may be stored as chunks.

Each chunk contains:

```text
chunk sequence number
previous chunk content digest or null
starting execution and observable digests
ordered frames
ending execution and observable digests
chunk content digest
```

Chunk sequence and digest linkage are validated before replay.

Chunking must not change the semantic replay sequence.

## 115. Replay execution failure

Non-atomic replay stops at the first failed or divergent frame and retains the machine state reached by all prior verified frames.

The failure reports:

```text
frame index
logical time where available
expected identities
actual identities
structured underlying decode, compatibility, preparation, or runtime failure
```

An atomic replay convenience clones or stages the complete starting machine and publishes only if all frames succeed.

## 116. Replay and optional history

Replay is required to reproduce execution and required observable state.

It is not required to reproduce optional historical retention decisions unless the replay log explicitly records and applies the same retention policy.

Therefore final snapshots after replay may share execution and observable digests while differing in `SnapshotDigest` because optional history differs.

## 117. Checkpoint and resume

A replay stream may begin from any compatible machine snapshot checkpoint.

The first frame must expect the checkpoint's execution digest, observable digest, topology revision, network fingerprint, and runtime policy identity.

A new checkpoint may terminate an old digest-suite replay chain and begin a new explicitly versioned chain after a supported schema upgrade.

---

# Part XII — Audit artifacts

## 118. Transaction result artifact

A `transaction_result` artifact contains the complete immutable semantic result:

```text
requested time
before and after revisions
semantic change set
migration report if any
diagnostics
schedule
before and after execution digests
after observable digest
runtime policy id
content-addressed provenance view
```

It also carries a `TransactionResultContentDigest` over the semantic result projection.

The artifact is evidence of a result, not authority to mutate a machine.

## 119. Cause references in results

Every cause referenced by a persisted result resolves through the result's own content-addressed provenance section or an explicit included checkpoint root.

The result remains interpretable after the originating live machine compacts its own optional provenance.

## 120. Migration report artifact

A `migration_report` artifact contains every structural, state, pending-event, output-baseline, provenance, diagnostic-episode, region, and invalidated-artifact outcome required by the topology-patch specification.

It includes:

```text
base and target network identities
base and target revisions
base and target fingerprints
effective logical time
normalized patch content digest
state-loss policy
actual semantic loss set
complete subject and pending-event classifications
migration provenance roots
```

The report cannot be used as a substitute for patch preparation or transaction execution.

## 121. Diagnostics in audit artifacts

Diagnostics are persisted structurally by:

```text
stable code
severity
primary subject
related subjects
structured evidence
machine-readable suggestions where present
episode identity where applicable
```

Rendered prose may be persisted as presentation metadata but is not authoritative and does not participate in diagnostic semantic comparison.

---

# Part XIII — Compatibility rules by artifact family

## 122. Definition compatibility

A network or module definition may be syntax-decoded under an older supported schema and upgraded when the upgrade is representation-only.

Validation under current semantics is allowed only when the declared semantic version vector is explicitly compatible.

If behavior would change, the library must preserve the old semantic model through a supported compatibility implementation or reject the definition.

It must not silently reinterpret it.

## 123. Snapshot compatibility

Ordinary snapshot restoration requires exact compatibility of all state-interpreting semantic components.

A snapshot may tolerate representation-only schema upgrades.

A snapshot must be rejected when any persisted state or pending-event payload has no exact current meaning.

## 124. Replay compatibility

Replay requires stricter compatibility than one-time snapshot restoration.

In addition to snapshot compatibility, every transaction, node law, patch rule, runtime policy field, and digest domain used by the replay chain must remain exact.

A replay chain cannot silently cross a semantic version change.

## 125. Patch compatibility

A patch artifact is compatible only when every operation and migration directive has the exact declared meaning.

Unknown operations or policies reject the patch.

A representation-only patch schema upgrade must preserve normalized operation content and target rewrite semantics.

## 126. Provenance compatibility

A provenance schema upgrade may rename or reorganize representation only if it preserves:

```text
all current roots
supporter and blocker roles
joint support structure
multiplicity
checkpoint authority
retention boundary
migration ancestry
```

A change to what constitutes a valid explanation is a semantic change.

## 127. Diagnostic compatibility

Changing rendered wording does not require a diagnostic semantic version change.

Changing any of the following does:

```text
stable code identity
severity semantics
episode identity rule
condition discriminator meaning
structured evidence interpretation
resolution lifecycle
```

## 128. Digest-suite change

A new digest suite changes every identity computed under that suite.

An upgrader must:

1. verify old digests under the old suite;
2. decode and validate the old artifact;
3. compute new digests under the new suite;
4. emit a new artifact and explicit upgrade report.

Old and new digests must never be compared as though they occupied one namespace.

## 129. Canonical-encoding change

A canonical-encoding change may preserve semantic fingerprints only if those fingerprints use an unchanged independent semantic projection and digest-domain version.

Artifact integrity and snapshot digests necessarily change when their canonical artifact bytes change.

## 130. Compatibility failure structure

A compatibility failure identifies:

```text
artifact kind
compatibility stage
version component
encountered version
supported versions or required version
network, time-domain, revision, or policy context where applicable
whether explicit schema upgrade exists
```

It must not collapse all incompatibility into `InvalidData`.

---

# Part XIV — Public persistence API responsibilities

## 131. Persistence module

The crate should expose a module broadly equivalent to:

```rust
pub mod persistence;
```

It owns canonical encoding, decoding, version inspection, compatibility policy, artifact envelopes, and persistence-specific failures.

It does not own file-system access.

## 132. Encoded artifact bytes

The canonical owned byte value is broadly equivalent to:

```rust
pub struct ArtifactBytes(Vec<u8>);
```

It provides immutable access to the complete prefixed canonical artifact.

Constructing `ArtifactBytes` from arbitrary bytes requires canonical decoding and validation of the envelope. A separate raw byte input type may exist internally.

## 133. Artifact header inspection

A bounded header-inspection API may expose:

```text
artifact kind
version vector
time-domain id
payload byte length
integrity digest
```

Header inspection must still validate the fixed prefix, envelope framing, canonical header representation, and configured size limits.

It need not semantically decode the complete payload.

## 134. Encoding APIs

Canonical encoding methods should exist for each persistable artifact family.

Representative shape:

```rust
pub fn encode_snapshot<D>(
    context: &PersistenceContext<D>,
    snapshot: &MachineSnapshot<D>,
) -> Result<ArtifactBytes, EncodeFailure>;
```

Equivalent methods apply to definitions, policies, patches, transaction records, replay artifacts, results, reports, and bundles.

Encoding validates the in-memory artifact invariants needed to prevent emitting corrupt canonical bytes.

## 135. Decoding APIs

Decoding is artifact-specific.

Representative shape:

```rust
pub fn decode_snapshot<D>(
    context: &PersistenceContext<D>,
    bytes: &[u8],
    policy: &DecodePolicy,
) -> Result<MachineSnapshot<D>, DecodeFailure>;
```

Definition decoding may return a decoded unchecked artifact plus validation claim rather than pretending the definition is valid.

Transaction decoding returns a `TransactionRecord<D>` that requires later materialization.

## 136. Decode policy

`DecodePolicy` contains resource bounds and accepted version policies.

It must not contain switches such as:

```text
ignore digest mismatch
accept noncanonical bytes
fill missing levels with Low
skip unknown pending events
reset incompatible state
```

Unsafe best-effort recovery is outside the canonical core.

## 137. Decode limits

The policy provides explicit limits for at least:

```text
maximum total artifact bytes
maximum nesting depth
maximum text bytes
maximum byte-string bytes
maximum collection length
maximum nodes
maximum ports
maximum connections
maximum module instances
maximum pending events
maximum provenance records
maximum provenance edges
maximum diagnostic records
maximum replay frames per decode operation
maximum optional-history bytes
```

The decoder performs checked arithmetic before allocation.

## 138. Restoration report

Restoration returns either one complete machine or a structured failure.

A separate report may include non-blocking accepted metadata notices, but the machine artifact remains all-or-nothing.

Ordinary restoration must not return a partially populated machine with warnings.

## 139. Transaction record materialization API

The persistence layer should expose a method broadly equivalent to:

```rust
impl<D> TransactionRecord<D> {
    pub fn materialize(
        self,
        machine: &Machine<D>,
    ) -> Result<Transaction<D>, TransactionMaterializationFailure>;
}
```

Replay may use an internal equivalent that reuses the same patch preparation and input-schema validation.

## 140. Streaming replay

A replay decoder may stream frames without loading the entire log when:

- the envelope or chunk framing remains canonical;
- each frame's integrity and schema are validated before application;
- resource limits remain enforceable;
- replay stops precisely at the first failure;
- partial application semantics are explicit.

Machine snapshots and definitions must still be fully validated before publication of a restored or compiled artifact.

## 141. Serde boundary

The canonical wire format is not defined by Rust `serde` data-model defaults.

A `serde` feature may provide convenient noncanonical human-readable projections, but:

- JSON output is not canonical persistence;
- generic CBOR serializers are not canonical unless they implement this exact profile;
- serde field order or enum tagging must not define semantic identity;
- canonical digests must use the dedicated encoder.

## 142. Human-readable projections

Debug JSON, YAML, or textual diagnostic exports may exist for tooling.

They must be labeled noncanonical and must not be accepted as exact snapshot or replay evidence without an explicit conversion and validation step.

---

# Part XV — Robustness, security, and storage integration

## 143. Hostile input handling

Malformed artifacts must produce structured failures without:

- panics;
- unchecked arithmetic;
- uncontrolled recursion;
- allocation from unvalidated lengths;
- partial machine publication;
- execution of host callbacks;
- mutation of caller-owned state.

## 144. Resource exhaustion

Decode budgets are separate from machine `RuntimePolicy`.

Decode failure due to a persistence limit does not imply that the semantic artifact would violate runtime policy if decoded under a larger trusted budget.

The failure must identify the persistence limit that was exceeded.

## 145. Integrity and truncation

The fixed prefix, canonical envelope framing, definite lengths, and integrity digest provide deterministic detection of truncation and accidental byte modification with cryptographic-hash confidence.

The decoder must distinguish:

```text
truncated input
malformed canonical encoding
integrity mismatch
schema mismatch
semantic incompatibility
restoration invariant failure
```

## 146. Confidentiality

Snapshots and replay logs may contain sensitive caller metadata, network structure, inputs, and execution history.

The core persistence layer provides no confidentiality guarantee.

Applications requiring confidentiality must encrypt artifacts through an external authenticated-encryption layer.

## 147. Authenticity

An artifact with a valid integrity digest may still have been created or modified by an untrusted party who recomputed the digest.

Authorization decisions must rely on an external signature or authenticated transport and must still perform all ordinary artifact validation.

## 148. Atomic file replacement

The core returns bytes and does not own file I/O.

A host writing checkpoints should use its platform's durable replacement discipline, typically:

```text
write new temporary file
flush file contents
atomically replace destination
flush containing directory where required
```

This is host integration guidance, not part of artifact semantics.

## 149. Partial or appended files

A standalone canonical artifact contains exactly one envelope and no trailing data.

Append-only storage uses an explicitly framed replay chunk container or an external record store. Concatenating standalone artifact byte strings does not create one valid artifact.

## 150. Recovery tools

A recovery tool may inspect syntactically valid substructures or report corruption locations.

It must not label repaired or partially recovered data as a canonical valid snapshot unless it emits a new explicit artifact with:

- a recovery report;
- new digest identity;
- no claim of uninterrupted replay continuity.

---

# Part XVI — Verification obligations

## 151. Canonical uniqueness

For every supported artifact value `A`:

```text
Decode(Encode(A)) == A
Encode(Decode(Encode(A))) == Encode(A)
```

For every accepted canonical byte sequence `B`:

```text
Encode(Decode(B)) == B
```

Noncanonical alternative encodings of the same abstract CBOR value must be rejected.

## 152. Golden vectors

The project must retain versioned golden byte vectors for representative:

```text
module definitions
network definitions
runtime policies
initialization snapshots
ready snapshots
snapshots with every built-in state family
snapshots with pending temporal work
provenance checkpoints
active diagnostic episodes
network patches and every migration directive
patch-bearing transaction records
replay frames and logs
transaction results
migration reports
checkpoint bundles
```

Golden vectors must include expected fingerprints and every applicable digest.

## 153. Permutation invariance

Tests must construct semantically equivalent artifacts under permutations of:

```text
definition insertion order
patch operation insertion order
hash-map iteration
unordered supporter order
pending equal-deadline storage order
diagnostic collection construction order
```

Canonical bytes and semantic digests must remain equal wherever the order is non-semantic.

## 154. Order sensitivity

Tests must also prove that semantically ordered sequences remain order-sensitive, including:

```text
replay frame order
processed logical time order
transport originating-time precedence
hierarchical path order
```

Canonicalization must not sort away semantic order.

## 155. Snapshot round trip

For every generated valid machine, including uninitialized machines:

```text
Restore(Decode(Encode(Snapshot(M))))
```

must produce a semantically equivalent machine under the compatible compiled topology, time-domain context, and runtime policy.

## 156. Snapshot sufficiency

For original machine `M`, restored equivalent machine `M_r`, and every compatible future transaction sequence `T`:

```text
Replay(M, T) == Replay(M_r, T)
```

Comparison includes execution, outputs, state changes, pending work, diagnostic episodes, provenance roots, schedules, and execution and observable digests.

## 157. Settled-state consistency

Property tests must corrupt persisted settled values while leaving other state and envelope integrity recomputed.

Restoration must reject the semantic inconsistency rather than accept or silently repair it.

## 158. Event identity continuity

Tests must verify that snapshot and restoration preserve:

```text
PendingEventKey values
next_pending_event_serial
future event key allocation
no key reuse
replay equality of event identities
```

Private event arena placement may differ.

## 159. Provenance canonicalization

Equivalent provenance DAGs built with different arena identifiers and insertion orders must produce identical:

```text
CauseDigests
canonical provenance records
ObservableStateDigest
SnapshotDigest when optional history is equal
```

Tests must include shared subgraphs, joint support, multiplicity, checkpoints, and migration derivations.

## 160. Provenance corruption

Required rejection cases include:

```text
missing predecessor
wrong record digest
cycle
invalid subject
invalid role
incomplete required root closure
conflicting records sharing one digest
false checkpoint completeness claim
```

## 161. Definition validation after decode

Persisted validated definitions must be revalidated and recompiled.

Tests must demonstrate that tampering with a definition and recomputing only its envelope digest cannot bypass:

```text
key uniqueness
kind checking
driver rules
current-reaction acyclicity
state-schema validation
fingerprint verification
```

## 162. Patch persistence equivalence

For every generated valid patch:

```text
Prepare(base, Decode(Encode(patch)))
```

must be equivalent to preparing the original patch.

Equivalent operation permutations must encode identically after normalization.

Every migration directive and target input-schema consequence must survive round trip.

## 163. Patch-bearing replay

A recorded patch transaction replayed from the same starting snapshot must:

- reconstruct the patch;
- re-prepare it;
- reproduce the target fingerprint;
- reproduce migration outcomes;
- reproduce output and diagnostic consequences;
- reproduce final execution and observable digests.

## 164. Replay chain verification

Tests must cover:

```text
valid complete chain
wrong starting digest
wrong observable digest
wrong revision
wrong runtime policy
wrong time domain
wrong target fingerprint
corrupted frame
missing frame
reordered frames
duplicated frame
chunk boundary before and after temporal deadlines
chunk boundary before and after topology patches
```

## 165. Historical artifact corpus

For every supported version vector, CI must retain representative historical artifacts and verify the declared action:

```text
exact decode
schema upgrade
restore
replay
structured rejection
```

Removing support requires an explicit compatibility-policy change and release note.

## 166. Digest-domain tests

Tests must prove that identical payload bytes under different digest domains yield different typed digests.

They must also prove that changing a version component included in one digest projection changes that digest unless a specified schema-only compatibility rule deliberately preserves it.

## 167. Malformed encoding corpus

The fuzz and regression corpus must include:

```text
non-shortest integers
indefinite lengths
CBOR maps
floating-point values
invalid UTF-8
duplicate fields
unsorted fields
wrong fixed byte lengths
unknown variants
trailing bytes
length overflow
truncation
integrity mismatch
excessive nesting
oversized collections
```

## 168. Decode fault injection

Fault injection should cover allocation and failure points during:

```text
envelope decode
schema decode
provenance reconstruction
definition validation
patch preparation
snapshot candidate construction
digest recomputation
private runtime-store construction
```

No failure may publish a partial machine or prepared patch.

## 169. Deterministic encoder independence

At least one test implementation or golden-vector validator should be sufficiently independent from the production encoder to detect shared ordering or field-omission defects.

Where a second implementation is impractical, a declarative schema registry and hand-authored golden vectors must supplement round-trip tests.

---

# Part XVII — Implementation guidance and prohibitions

## 170. Recommended internal pipeline

A recommended implementation pipeline is:

```text
public semantic artifact
        ↓
stable-keyed persistence projection
        ↓
artifact schema validation
        ↓
canonical value tree or streaming events
        ↓
strict deterministic CBOR encoder
        ↓
domain-separated digest calculation
        ↓
canonical artifact envelope
```

Decoding reverses the pipeline but inserts validation before every trusted type transition.

## 171. Shared schema definitions

The encoder, decoder, digest projection, golden-vector generator, and documentation should derive from one reviewed schema description where practical.

Generated code is acceptable when:

- field and variant names remain explicit and stable;
- generation is deterministic;
- generated output is reviewable;
- compatibility tests prevent accidental renumbering or renaming;
- the schema cannot silently expose private Rust fields.

## 172. No raw memory serialization

The implementation must not persist:

```text
repr(C) structs by memory copy
Rust enum discriminants
usize values
pointer addresses
hash-table buckets
Arc or Rc internals
private arena slots
platform endianness
compiler-dependent layout
```

## 173. No derived-cache trust

Persisted caches may exist only as optional accelerators in a separately named cache artifact.

They must be validated against canonical source artifacts and may be discarded without semantic effect.

The initial core defines no canonical compiled-network or prepared-patch cache format.

## 174. No hidden compatibility heuristics

The implementation must not accept an artifact because:

- a Rust deserializer happened to populate current fields;
- unknown fields were ignored;
- a fingerprint was absent;
- names looked similar;
- state record byte sizes matched;
- a pending event variant had a familiar prefix;
- a newer enum discriminant fit in the old integer type.

## 175. No silent repair

The canonical decoder and restorer must not silently:

```text
sort noncanonical input
remove duplicates
truncate counts
replace invalid text
invent missing state
infer Low
cancel invalid pending events
recompute a mismatched output baseline and continue
prune required provenance to make a cycle disappear
```

An explicit recovery tool may produce a new artifact and report every repair, but that artifact begins a new trust and replay boundary.

## 176. Performance expectations

Canonical persistence is optimized for correctness, deterministic identity, and durable compatibility before minimum byte size.

Implementations should support streaming and bounded allocation, but must not weaken canonicality or validation to gain speed.

Private snapshots used only for short-lived in-process rollback may use another representation, but they are not persisted `MachineSnapshotArtifact` values and must refine the same semantic state.

## 177. Deferred additions

Future specifications may add:

```text
compiled cache artifacts
signed artifact envelopes
encrypted bundle profiles
standard compressed wrappers
semantic migration tools
remote replay transport
content-addressed artifact stores
partial snapshot or region export
caller binding codecs
editor project containers
```

Each addition must preserve the canonical semantic artifact boundary defined here.

---

# Part XVIII — Required guarantees

## 178. Persistence guarantees

The implementation must provide:

```text
stable-keyed artifact representations
explicit persistent time-domain identity
strict canonical binary encoding
one canonical byte sequence per supported artifact value
domain-separated 256-bit digests
complete machine snapshots
content-addressed canonical provenance
persistent public pending-event identity allocation state
all-or-nothing restoration
normalized topology-patch persistence
re-preparation of patch-bearing transactions
ordered and digest-chained replay logs
explicit schema and semantic compatibility
bounded hostile-input decoding
historical compatibility tests
golden canonical vectors
```

## 179. Prohibited outcomes

The implementation must make these outcomes impossible through ordinary canonical APIs:

```text
restoring against the wrong time domain
trusting a dense index from persisted bytes
accepting noncanonical alternate bytes as equivalent
silently resetting incompatible state
silently dropping pending work
reusing a public pending-event identity after restoration
replaying a private PreparedPatch representation
accepting an unknown semantic field
claiming replay continuity across an unacknowledged version change
using artifact integrity as authentication
publishing a partially restored machine
```

## 180. Relationship to other specifications

This specification governs durable representation and compatibility.

The API and semantics specification remains authoritative for observable machine behavior.

The built-in node specification remains authoritative for node state and pending-event meanings.

The processor architecture remains authoritative for runtime invariants and atomic publication.

The concrete Rust API surface remains authoritative for ordinary in-memory ownership and entry points, except where this specification introduces persistence-only records such as `TimeDomainId`, `PersistenceContext<D>`, and decoded `TransactionRecord<D>` materialization.

The topology-patch specification remains authoritative for normalized operations and migration outcomes.

The testing and verification policy remains authoritative for the overall verification strategy; the obligations in this document specialize it for persistence and compatibility.

---

# Appendix A — Canonical artifact field summary

## A.1 Definition artifacts

```text
module_definition:
    module definition
    optional validation claim
    persistence metadata

network_definition:
    network definition
    optional validation claim
    persistence metadata
```

## A.2 Runtime input and transaction artifacts

```text
runtime_policy:
    named semantic limits
    RuntimePolicyId

input_snapshot:
    network and schema binding
    complete external level observations
    positive pulse counts

input_delta:
    network and schema binding
    changed level observations
    positive pulse counts

network_patch:
    base binding
    normalized operations
    reassociations
    migration directives
    expected target fingerprint

transaction_record:
    time and preconditions
    input artifact
    optional normalized patch attachment
    reconfiguration policy
    transaction metadata
```

## A.3 Runtime checkpoint and replay artifacts

```text
machine_snapshot:
    common identity and versions
    lifecycle-specific semantic state
    required current observation
    content-addressed provenance
    optional history
    runtime policy identity

replay_frame:
    expected previous execution state and revision
    runtime policy identity
    transaction record
    resulting execution and observable digests
    optional recorded result

replay_log:
    starting identities
    ordered frames
    optional final identities

checkpoint_bundle:
    network definition
    runtime policy
    machine snapshot
    optional replay log
```

## A.4 Audit artifacts

```text
transaction_result:
    complete committed semantic result
    self-contained result provenance

migration_report:
    complete patch finalization and loss accounting
```

---

# Appendix B — Digest scope summary

| Identity | Covers | Excludes |
|---|---|---|
| `NetworkFingerprint` | Stable semantic topology and parameters | Revision, metadata, dense layout |
| `ModuleFingerprint` | Stable semantic module interface and internals | Instance placement and metadata |
| `RuntimePolicyId` | Semantically relevant runtime limits | Performance-only tuning |
| `ExecutionStateDigest` | Future execution, failures, episode state, event identity cursor | Runtime policy, provenance, optional history |
| `ObservableStateDigest` | Execution state plus complete required current observation | Optional history |
| `SnapshotDigest` | Complete canonical snapshot artifact | Its own digest field |
| Artifact integrity digest | Complete canonical artifact envelope content | Fixed prefix and its own field |

---

# Appendix C — Restoration compatibility matrix

| Condition | Ordinary restore |
|---|---|
| Same schema and semantic versions | Accept if all validation succeeds |
| Older representation-only schema with supported upgrader | Upgrade, recompute, then validate |
| Different `TimeDomainId` | Reject |
| Different network fingerprint | Reject |
| Different runtime policy id | Reject on ordinary restore |
| Unknown node-state variant | Reject |
| Unknown pending-event variant | Reject |
| Missing required provenance | Reject |
| Optional history omitted | Accept |
| Optional history differs | Accept; snapshot digest differs |
| Diagnostic wording differs outside authoritative fields | Accept where schema-compatible |
| Semantic node law differs | Reject or use separately specified explicit migration |
| Digest suite differs with supported artifact upgrader | Verify old, upgrade, recompute; begin new digest identity |

---

# Appendix D — Replay identity

Exact replay identity is conceptually:

```text
TimeDomainId
+ NetworkFingerprint
+ SemanticVersionVector
+ Starting ExecutionStateDigest
+ Starting ObservableStateDigest
+ RuntimePolicyId
+ Ordered TransactionRecord sequence
```

The replay frame chain provides incremental verification of this identity.

---

# Appendix E — Required topology-patch variant coverage

The `network_patch` schema must provide one exact persisted variant for every canonical patch operation:

```text
add_node
remove_node
replace_node
add_connection
remove_connection
replace_connection
add_external_input
remove_external_input
replace_external_input
add_external_output
remove_external_output
replace_external_output
add_module_instance
remove_module_instance
replace_module_instance
set_parent
set_diagnostic_meta
reassociate
```

The `reassociate` variant must cover:

```text
node
module_instance
in_port
out_port
external_input
external_output
```

Module migration directives must cover:

```text
standard
explicit {
    node_overrides
    internal_reassociations
}
```

Node migration directives must cover:

```text
standard
require_preserve
reset
transfer_stored_level
pulse_delay
transport_delay
inertial_delay
periodic
```

Temporal migration policies must cover:

```text
overdue:
    reject
    mature_at_patch_time

pulse_delay:
    preserve_deadlines
    recompute_from_origin
    restart_from_patch_time
    cancel_pending
    reject_if_pending

transport_delay:
    preserve_deadlines
    recompute_from_origin
    restart_from_patch_time
    cancel_pending
    reject_if_pending

inertial_delay:
    preserve_deadline
    recompute_from_origin
    restart_from_patch_time
    cancel_candidate
    reject_if_candidate

periodic:
    preserve_next_deadline
    recompute_from_existing_anchor
    reanchor_at_patch_time
    cancel_schedule
    reject_if_anchored
```

Transaction patch attachments must persist the reconfiguration policy exactly:

```text
reject_state_loss
allow_reported_state_loss
```

Migration report artifacts must encode the finalized state outcomes:

```text
preserve
migrate
reset
reject
```

and pending-event outcomes:

```text
preserve_deadline
recompute_deadline
transform_payload
cancel
reject
```

No future patch or migration variant is persistence-compatible with schema version 1 until it receives a new explicit schema variant and compatibility rule.
