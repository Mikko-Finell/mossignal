## docs/specs/persistence_canonical_encoding_and_compatibility_spec.md
- ``mossignal` Persistence, Canonical Encoding, and Compatibility Specification` [1-57]
  Preview: **Status:** Design specification, version 1 **Defines:** Persistable semantic artifacts, canonical binary encoding, digest and fingerprint construction, time-domain identity, snapshot representation, restoration validation, replay records and logs, topology-patch persistence, provenance encoding, schema and semantic compatibility, decoding limits, and persistence verification obligations **Does not define:** General processor execution, ordinary built-in node behavior, the exhaustive diagnostic-code catalogue, file-system or database APIs, network transport protocols, compression formats, encryption, digital-signature policy, caller-owned binding serialization, editor project formats, or automatic semantic migration between incompatible versions This specification defines how `mossignal` semantic artifacts are converted into durable bytes and recovered without depending on private runtime representation.
  Symbols: `mossignal`
  Normative: MUST NOT 1, MUST 1, SHOULD NOT 1, SHOULD 1, MAY 1

- ``mossignal` Persistence, Canonical Encoding, and Compatibility Specification > 1. Purpose` [9-37]
  Preview: This specification defines how `mossignal` semantic artifacts are converted into durable bytes and recovered without depending on private runtime representation.
  Symbols: `mossignal`

- ``mossignal` Persistence, Canonical Encoding, and Compatibility Specification > 2. Normative language` [38-57]
  Preview: The terms **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.
  Normative: MUST NOT 1, MUST 1, SHOULD NOT 1, SHOULD 1, MAY 1

- `Part I — Persistence model and boundaries` [58-200]
  Preview: The persistence design follows these axioms.
  Symbols: `TransactionResultArtifact`, `MigrationReportArtifact`, `NetworkPatchArtifact`, `mossignal`

- `Part I — Persistence model and boundaries > 3. Persistence axioms` [60-93]
  Preview: The persistence design follows these axioms.

- `Part I — Persistence model and boundaries > 3. Persistence axioms > 3.1 Stable-keyed representation` [64-69]
  Preview: Every structural or runtime fact that refers to authored structure uses stable keys.

- `Part I — Persistence model and boundaries > 3. Persistence axioms > 3.2 Complete future state` [70-75]
  Preview: A machine snapshot contains every semantic fact required to determine future execution and required future observation.

- `Part I — Persistence model and boundaries > 3. Persistence axioms > 3.3 Canonical bytes` [76-81]
  Preview: For one supported artifact value and one complete version vector, exactly one canonical byte sequence exists.

- `Part I — Persistence model and boundaries > 3. Persistence axioms > 3.4 Explicit compatibility` [82-87]
  Preview: Compatibility is determined by declared version rules and artifact validation.

- `Part I — Persistence model and boundaries > 3. Persistence axioms > 3.5 Integrity is not authenticity` [88-93]
  Preview: Canonical digests detect accidental corruption and support deterministic identity.

- `Part I — Persistence model and boundaries > 4. Artifact classes` [94-115]
  Preview: The initial persistence layer defines canonical encodings for: `TransactionResultArtifact` and `MigrationReportArtifact` are audit artifacts.
  Symbols: `TransactionResultArtifact`, `MigrationReportArtifact`

- `Part I — Persistence model and boundaries > 5. Canonical persisted forms versus in-memory types` [116-135]
  Preview: The persisted form of a public value need not match its private Rust layout.

- `Part I — Persistence model and boundaries > 6. Values not serialized directly` [136-156]
  Preview: The following are not canonical persistence artifacts: Equivalent source artifacts may be persisted instead.
  Symbols: `NetworkPatchArtifact`

- `Part I — Persistence model and boundaries > 7. Semantic, observational, and archival data` [157-180]
  Preview: Persisted facts are classified as: Data capable of changing future execution or whether a future transaction succeeds.

- `Part I — Persistence model and boundaries > 7. Semantic, observational, and archival data > 7.1 Execution-semantic data` [161-166]
  Preview: Data capable of changing future execution or whether a future transaction succeeds.

- `Part I — Persistence model and boundaries > 7. Semantic, observational, and archival data > 7.2 Required observational data` [167-172]
  Preview: Data required to reproduce current inspection, explanations, output baselines, or causal roots.

- `Part I — Persistence model and boundaries > 7. Semantic, observational, and archival data > 7.3 Optional archival data` [173-180]
  Preview: Data retained for history but not required for future execution or complete current observation.

- `Part I — Persistence model and boundaries > 8. Persistence trust boundary` [181-200]
  Preview: All decoded bytes are untrusted.
  Symbols: `mossignal`

- `Part II — Persistent time-domain identity` [201-268]
  Preview: The Rust marker `D` prevents accidental mixing of time domains within one process, but Rust type identity is not present in persisted bytes.
  Symbols: `TimeDomainId`, `TypeId`, `WrongTimeDomain`, `SimulationTicks`, `milliseconds`

- `Part II — Persistent time-domain identity > 9. Need for runtime time-domain identity` [203-208]
  Preview: The Rust marker `D` prevents accidental mixing of time domains within one process, but Rust type identity is not present in persisted bytes.

- `Part II — Persistent time-domain identity > 10. `TimeDomainId`` [209-226]
  Preview: The persistence layer defines: A `TimeDomainId`: - is supplied and owned by the caller; - identifies the intended interpretation of one logical tick; - is stable across processes and releases where artifacts are expected to remain compatible; - is independent of Rust type names, crate paths, compiler versions, and `TypeId`; - requires no global registry or singleton allocator.
  Symbols: `TimeDomainId`, `TypeId`

- `Part II — Persistent time-domain identity > 11. Persistence context` [227-241]
  Preview: Encoding and decoding `D`-parameterized artifacts requires an explicit context broadly equivalent to: The context associates the process-local marker `D` with one persistent `TimeDomainId`.
  Symbols: `TimeDomainId`

- `Part II — Persistent time-domain identity > 12. Time-domain compatibility` [242-254]
  Preview: Decoding into `D` requires exact equality between: A mismatch is a structured `WrongTimeDomain` failure.
  Symbols: `WrongTimeDomain`, `SimulationTicks`, `milliseconds`

- `Part II — Persistent time-domain identity > 13. Time-domain contribution to identity` [255-268]
  Preview: `TimeDomainId` contributes to: - network and module fingerprints; - machine snapshot compatibility; - transaction and patch binding; - replay compatibility; - all `D`-dependent canonical digests.
  Symbols: `TimeDomainId`

- `Part III — Version and compatibility model` [269-474]
  Preview: Every artifact envelope carries a complete version vector.
  Symbols: `n + 1`, `n`, `artifact_kind`

- `Part III — Version and compatibility model > 14. Version vector` [271-290]
  Preview: Every artifact envelope carries a complete version vector.

- `Part III — Version and compatibility model > 15. Version representation` [291-300]
  Preview: Each version component is an unsigned 32-bit identifier.
  Symbols: `n + 1`, `n`

- `Part III — Version and compatibility model > 16. Version responsibilities` [301-342]
  Preview: Defines the top-level artifact envelope fields.
  Symbols: `artifact_kind`

- `Part III — Version and compatibility model > 16. Version responsibilities > 16.1 Envelope schema` [303-306]
  Preview: Defines the top-level artifact envelope fields.

- `Part III — Version and compatibility model > 16. Version responsibilities > 16.2 Canonical encoding` [307-310]
  Preview: Defines the deterministic value grammar, record shape, collection ordering, and primitive encodings.

- `Part III — Version and compatibility model > 16. Version responsibilities > 16.3 Digest suite` [311-314]
  Preview: Defines the hash algorithm, output width, and domain-separation inputs.

- `Part III — Version and compatibility model > 16. Version responsibilities > 16.4 Artifact schema` [315-320]
  Preview: Defines fields and variants for one artifact family.
  Symbols: `artifact_kind`

- `Part III — Version and compatibility model > 16. Version responsibilities > 16.5 Core semantics` [321-324]
  Preview: Defines lifecycle, reaction, transaction, time, state, and output semantics shared by the system.

- `Part III — Version and compatibility model > 16. Version responsibilities > 16.6 Built-in node semantics` [325-328]
  Preview: Defines node laws, state schemas, pending-event payload meanings, and node compatibility behavior.

- `Part III — Version and compatibility model > 16. Version responsibilities > 16.7 Topology-patch semantics` [329-332]
  Preview: Defines patch operations, correspondence, migration directives, and state-loss rules.

- `Part III — Version and compatibility model > 16. Version responsibilities > 16.8 Provenance semantics` [333-336]
  Preview: Defines derivation record meanings, checkpoint rules, required roots, and retention boundaries.

- `Part III — Version and compatibility model > 16. Version responsibilities > 16.9 Diagnostic schema` [337-342]
  Preview: Defines stable diagnostic codes, structured evidence forms, episode identity, and persisted diagnostic shapes.

- `Part III — Version and compatibility model > 17. Compatibility levels` [343-359]
  Preview: The persistence layer distinguishes: These levels are strictly stronger in the order shown.

- `Part III — Version and compatibility model > 18. Syntactic decoding` [360-372]
  Preview: Syntactic decoding requires: - valid envelope prefix; - supported envelope schema; - canonical value encoding; - valid primitive lengths and UTF-8; - internally well-formed envelope fields; - matching integrity digest.

- `Part III — Version and compatibility model > 19. Schema compatibility` [373-378]
  Preview: Schema compatibility requires a reader for the exact artifact schema version or an explicit representation-only upgrader from that version.

- `Part III — Version and compatibility model > 20. Semantic compatibility` [379-384]
  Preview: Semantic compatibility requires every applicable semantic version component to be accepted by an explicit compatibility rule.

- `Part III — Version and compatibility model > 21. Restoration compatibility` [385-399]
  Preview: A machine snapshot is restoration-compatible only when all of the following hold:

- `Part III — Version and compatibility model > 22. Replay compatibility` [400-412]
  Preview: Replay compatibility additionally requires:

- `Part III — Version and compatibility model > 23. Exact artifact round trip` [413-420]
  Preview: Exact artifact round trip means decoding and re-encoding yields byte-for-byte identical canonical bytes.

- `Part III — Version and compatibility model > 24. Schema upgrades` [421-435]
  Preview: A schema upgrader must be: - explicit; - deterministic; - pure over the decoded artifact; - version-to-version specific; - semantics-preserving; - covered by golden vectors and historical artifact tests.

- `Part III — Version and compatibility model > 25. Semantic migrations` [436-451]
  Preview: A change in represented behavior is not a schema upgrade.

- `Part III — Version and compatibility model > 26. No implicit downgrade` [452-457]
  Preview: The library must not emit an older schema or semantic version unless an explicit downgrade implementation exists and proves that the current artifact is representable without semantic loss.

- `Part III — Version and compatibility model > 27. Compatibility support table` [458-474]
  Preview: Each release must publish and test a table identifying, per artifact kind and version vector: Unsupported combinations must fail with the exact incompatible component and encountered version.

- `Part IV — Canonical binary encoding` [475-720]
  Preview: The initial wire representation is a strict deterministic CBOR profile.
  Symbols: `null`, `snake_case`, `mossignal`

- `Part IV — Canonical binary encoding > 28. Encoding foundation` [477-489]
  Preview: The initial wire representation is a strict deterministic CBOR profile.

- `Part IV — Canonical binary encoding > 29. Artifact prefix` [490-502]
  Preview: Every standalone artifact begins with these eight bytes: The prefix is not part of any semantic digest.
  Symbols: `mossignal`

- `Part IV — Canonical binary encoding > 30. Allowed canonical values` [503-530]
  Preview: The canonical profile permits only: The profile forbids: All lengths and integer values use the shortest valid CBOR representation.

- `Part IV — Canonical binary encoding > 31. Records` [531-552]
  Preview: A record is encoded as an array of two-element field entries: Field names are schema-defined lowercase ASCII `snake_case` strings.
  Symbols: `snake_case`, `null`

- `Part IV — Canonical binary encoding > 32. Variants` [553-566]
  Preview: A closed enum variant is encoded as: The variant name is a schema-defined lowercase ASCII `snake_case` string.
  Symbols: `snake_case`, `null`

- `Part IV — Canonical binary encoding > 33. Options` [567-579]
  Preview: An optional value is represented by field absence whenever the containing record schema permits omission.
  Symbols: `null`

- `Part IV — Canonical binary encoding > 34. Sequences` [580-590]
  Preview: A semantically ordered sequence is encoded in semantic order.

- `Part IV — Canonical binary encoding > 35. Sets and maps` [591-610]
  Preview: The canonical profile does not use CBOR maps.

- `Part IV — Canonical binary encoding > 36. Canonical ordering` [611-635]
  Preview: The default order for fixed-width opaque identities is lexicographic order of their canonical bytes.

- `Part IV — Canonical binary encoding > 37. Text` [636-643]
  Preview: Text is valid UTF-8 and is persisted byte-for-byte.

- `Part IV — Canonical binary encoding > 38. Stable keys` [644-651]
  Preview: Every 128-bit stable key is encoded as one byte string of length 16 in unsigned big-endian order.

- `Part IV — Canonical binary encoding > 39. Time, span, pulse count, and revision` [652-665]
  Preview: The initial canonical forms are: Checked arithmetic rules remain unchanged after decoding.

- `Part IV — Canonical binary encoding > 40. Fixed digests and fingerprints` [666-671]
  Preview: Every initial digest and fingerprint is encoded as one byte string of length 32.

- `Part IV — Canonical binary encoding > 41. Diagnostic codes and enum names` [672-677]
  Preview: Stable diagnostic codes are encoded by their stable machine-readable code string, not rendered summary text.

- `Part IV — Canonical binary encoding > 42. Unknown fields and extensions` [678-700]
  Preview: Within a known schema version, unknown fields and variants are errors unless they occur inside an explicitly declared extension field.

- `Part IV — Canonical binary encoding > 43. Canonical decoding` [701-720]
  Preview: The decoder must reject: - non-shortest integers or lengths; - forbidden CBOR types; - indefinite lengths; - duplicate record fields; - unsorted record fields; - unsorted semantic sets or maps; - invalid UTF-8; - wrong fixed byte lengths; - unknown required variants; - trailing bytes after one standalone artifact; - a valid value whose re-encoding differs from the received canonical body.

- `Part V — Artifact envelope and integrity` [721-857]
  Preview: The canonical CBOR body is one variant: The envelope record contains: Fields are encoded in canonical record order.
  Symbols: `integrity_digest`, `SnapshotDigest`, `time_domain_id`, `null`, `artifact_kind`, `machine_snapshot`, `MachineSnapshot<D>`

- `Part V — Artifact envelope and integrity > 44. Envelope shape` [723-769]
  Preview: The canonical CBOR body is one variant: The envelope record contains: Fields are encoded in canonical record order.
  Symbols: `time_domain_id`, `null`

- `Part V — Artifact envelope and integrity > 44. Envelope shape > 44.1 Embedded artifacts` [753-769]
  Preview: When one artifact contains another artifact family, the child is embedded as a byte string containing the child's complete standalone canonical bytes, including its fixed prefix and envelope.

- `Part V — Artifact envelope and integrity > 45. Artifact kinds` [770-791]
  Preview: The initial `artifact_kind` strings are: An unknown artifact kind is a structured failure.
  Symbols: `artifact_kind`

- `Part V — Artifact envelope and integrity > 46. Envelope integrity digest` [792-813]
  Preview: The envelope contains an `integrity_digest` over the same envelope with that field omitted.
  Symbols: `integrity_digest`

- `Part V — Artifact envelope and integrity > 47. Snapshot digest placement` [814-821]
  Preview: For a `machine_snapshot` artifact, the envelope `integrity_digest` is the public `SnapshotDigest` and is computed under the snapshot-specific digest domain.
  Symbols: `SnapshotDigest`, `machine_snapshot`, `integrity_digest`, `MachineSnapshot<D>`

- `Part V — Artifact envelope and integrity > 48. Content identity of other artifacts` [822-836]
  Preview: Other artifact kinds may expose the envelope integrity digest as a generic persisted artifact content identifier, but it must not be confused with: Those identities use their own semantic projections and domain labels.

- `Part V — Artifact envelope and integrity > 49. Compression and wrappers` [837-847]
  Preview: The canonical v1 artifact body is uncompressed.

- `Part V — Artifact envelope and integrity > 50. Encryption and signatures` [848-857]
  Preview: Encryption, signatures, certificates, MACs, and access-control metadata are external wrappers.

- `Part VI — Digest and fingerprint suite` [858-1032]
  Preview: `DigestSuiteVersion = 1` uses unkeyed BLAKE3 with a 256-bit output.
  Symbols: `RuntimePolicyId`, `DigestSuiteVersion = 1`, `NetworkFingerprint`, `ModuleFingerprint`, `SnapshotDigest`, `integrity_digest`

- `Part VI — Digest and fingerprint suite > 51. Initial digest suite` [860-877]
  Preview: `DigestSuiteVersion = 1` uses unkeyed BLAKE3 with a 256-bit output.
  Symbols: `DigestSuiteVersion = 1`

- `Part VI — Digest and fingerprint suite > 52. Domain separation` [878-899]
  Preview: Distinct digest types must use distinct domain labels.

- `Part VI — Digest and fingerprint suite > 53. Digest version inputs` [900-907]
  Preview: Every semantic digest payload includes the version components that define interpretation of its fields.

- `Part VI — Digest and fingerprint suite > 54. Network fingerprint` [908-942]
  Preview: `NetworkFingerprint` is computed from a canonical semantic projection containing: It excludes: Semantically equivalent stable-keyed definitions must produce the same fingerprint.
  Symbols: `NetworkFingerprint`

- `Part VI — Digest and fingerprint suite > 55. Module fingerprint` [943-948]
  Preview: `ModuleFingerprint` follows the same rules for one reusable module definition and includes module interface keys and module-internal stable identity.
  Symbols: `ModuleFingerprint`

- `Part VI — Digest and fingerprint suite > 56. Runtime policy identifier` [949-956]
  Preview: `RuntimePolicyId` is computed from every policy field capable of changing whether an operation succeeds or fails.
  Symbols: `RuntimePolicyId`

- `Part VI — Digest and fingerprint suite > 57. Execution-state digest` [957-990]
  Preview: The execution-state digest covers every fact capable of affecting future execution, future structured failure, persistent diagnostic publication, or public pending-event identity.
  Symbols: `RuntimePolicyId`

- `Part VI — Digest and fingerprint suite > 58. Observable-state digest` [991-1006]
  Preview: The observable-state digest extends the execution-state projection with complete current required observation: Optional history outside the required current closure remains excluded.

- `Part VI — Digest and fingerprint suite > 59. Snapshot digest` [1007-1024]
  Preview: `SnapshotDigest` covers the complete versioned machine snapshot artifact, including: It excludes only its own digest field, represented by the omitted envelope `integrity_digest` during hashing.
  Symbols: `SnapshotDigest`, `integrity_digest`

- `Part VI — Digest and fingerprint suite > 60. Digest equality and collision handling` [1025-1032]
  Preview: Digest equality is a strong practical consistency check, not a mathematical proof of semantic equality.

- `Part VII — Canonical provenance persistence` [1033-1124]
  Preview: Persisted provenance uses a content-addressed Merkle DAG.
  Symbols: `CauseDigest`, `CauseRef`

- `Part VII — Canonical provenance persistence > 61. Provenance as a Merkle DAG` [1035-1053]
  Preview: Persisted provenance uses a content-addressed Merkle DAG.
  Symbols: `CauseDigest`, `CauseRef`

- `Part VII — Canonical provenance persistence > 62. Labeled predecessor relations` [1054-1081]
  Preview: Each predecessor edge carries a semantic role such as: Relations with different roles remain distinct even when they reference the same cause.

- `Part VII — Canonical provenance persistence > 63. Canonical provenance record order` [1082-1093]
  Preview: A snapshot or result artifact stores provenance records sorted by `CauseDigest` bytes.
  Symbols: `CauseDigest`

- `Part VII — Canonical provenance persistence > 64. Coalescing equivalent derivations` [1094-1101]
  Preview: Two provenance nodes with byte-identical canonical record content have the same `CauseDigest` and may be stored once.
  Symbols: `CauseDigest`

- `Part VII — Canonical provenance persistence > 65. Provenance validation` [1102-1116]
  Preview: Restoration validates: - every record digest; - every predecessor reference; - acyclicity; - required root closure; - role validity for the record kind; - subject existence and revision compatibility; - checkpoint authority and retention boundary; - absence of conflicting records sharing one digest.

- `Part VII — Canonical provenance persistence > 66. Cause references in other records` [1117-1124]
  Preview: A persisted cause reference is one 32-byte `CauseDigest`.
  Symbols: `CauseDigest`, `CauseRef`

- `Part VIII — Definition artifacts` [1125-1228]
  Preview: A `network_definition` payload contains: Collections are stable-key sorted.
  Symbols: `network_definition`, `module_definition`, `ValidatedNetwork<D>`, `CompiledNetwork<D>`, `KeyAllocator`

- `Part VIII — Definition artifacts > 67. Network definition artifact` [1127-1148]
  Preview: A `network_definition` payload contains: Collections are stable-key sorted.
  Symbols: `network_definition`

- `Part VIII — Definition artifacts > 68. Validation claim` [1149-1171]
  Preview: A definition artifact may contain: or: The claim is advisory until revalidation and recompilation reproduce the fingerprint under the claimed versions.

- `Part VIII — Definition artifacts > 69. Module definition artifact` [1172-1177]
  Preview: A `module_definition` payload contains the complete reusable module interface, internal stable-keyed definition, hierarchy, metadata, semantic parameters, and optional validation claim.
  Symbols: `module_definition`

- `Part VIII — Definition artifacts > 70. Persisting validated or compiled networks` [1178-1195]
  Preview: Encoding a `ValidatedNetwork<D>` or `CompiledNetwork<D>` emits the canonical network definition plus a validation claim.
  Symbols: `ValidatedNetwork<D>`, `CompiledNetwork<D>`

- `Part VIII — Definition artifacts > 71. Definition ordering` [1196-1210]
  Preview: Definition collection order is: For malformed duplicate keys, ties are broken by the complete canonical record bytes so encoding remains deterministic and validation can report the duplicates.

- `Part VIII — Definition artifacts > 72. Metadata` [1211-1218]
  Preview: Diagnostic metadata is encoded completely and byte-preservingly.

- `Part VIII — Definition artifacts > 73. Authoring allocator state` [1219-1228]
  Preview: Caller-owned `KeyAllocator` state is not part of network semantics and is not required in a definition artifact.
  Symbols: `KeyAllocator`

- `Part IX — Runtime policy, input, transaction, and patch artifacts` [1229-1369]
  Preview: A `runtime_policy` payload contains every semantically relevant named limit and its computed `RuntimePolicyId`.
  Symbols: `PreparedPatch<D>`, `runtime_policy`, `RuntimePolicyId`, `input_snapshot`, `input_delta`, `network_patch`, `NetworkPatchContentDigest`, `mossignal/network_patch_content/v1`, `transaction_record`, `Transaction<D>`, `TransactionMeta`

- `Part IX — Runtime policy, input, transaction, and patch artifacts > 74. Runtime policy artifact` [1231-1238]
  Preview: A `runtime_policy` payload contains every semantically relevant named limit and its computed `RuntimePolicyId`.
  Symbols: `runtime_policy`, `RuntimePolicyId`

- `Part IX — Runtime policy, input, transaction, and patch artifacts > 75. Input schema binding` [1239-1254]
  Preview: An input snapshot or delta artifact contains: A target-bound patch input also contains the expected target fingerprint and target input-schema fingerprint.

- `Part IX — Runtime policy, input, transaction, and patch artifacts > 76. Input snapshot artifact` [1255-1264]
  Preview: An `input_snapshot` payload contains exactly one level value for every required external level input in its bound schema.
  Symbols: `input_snapshot`

- `Part IX — Runtime policy, input, transaction, and patch artifacts > 77. Input delta artifact` [1265-1272]
  Preview: An `input_delta` payload contains changed or explicitly observed external levels and positive pulse counts.
  Symbols: `input_delta`

- `Part IX — Runtime policy, input, transaction, and patch artifacts > 78. Network patch artifact` [1273-1293]
  Preview: A `network_patch` payload contains: Operations are encoded in the canonical normalized order defined by the topology-patch specification, not builder insertion order.
  Symbols: `network_patch`

- `Part IX — Runtime policy, input, transaction, and patch artifacts > 79. Patch content digest` [1294-1301]
  Preview: A patch artifact may expose a `NetworkPatchContentDigest` using the `mossignal/network_patch_content/v1` domain.
  Symbols: `NetworkPatchContentDigest`, `mossignal/network_patch_content/v1`

- `Part IX — Runtime policy, input, transaction, and patch artifacts > 80. Prepared patches` [1302-1320]
  Preview: `PreparedPatch<D>` is not serialized directly.
  Symbols: `PreparedPatch<D>`

- `Part IX — Runtime policy, input, transaction, and patch artifacts > 81. Transaction record artifact` [1321-1344]
  Preview: A `transaction_record` payload contains: The patch attachment contains: It does not contain private `PreparedPatch<D>` bytes.
  Symbols: `transaction_record`, `PreparedPatch<D>`

- `Part IX — Runtime policy, input, transaction, and patch artifacts > 82. Transaction materialization` [1345-1359]
  Preview: A decoded transaction record becomes an executable `Transaction<D>` only after materialization against a machine or compiled topology context.
  Symbols: `Transaction<D>`

- `Part IX — Runtime policy, input, transaction, and patch artifacts > 83. Transaction metadata` [1360-1369]
  Preview: `TransactionMeta` is persisted exactly because it may appear in provenance roots, diagnostics, and replay audit records.
  Symbols: `TransactionMeta`

- `Part X — Machine snapshot schema` [1370-1794]
  Preview: A machine snapshot is a complete stable-keyed checkpoint of one committed machine version.
  Symbols: `PendingEventKey`, `SnapshotDigest`, `machine_snapshot`, `lifecycle`, `NodeKey`, `optional_history`, `next_pending_event_serial`, `CompiledNetwork::restore(snapshot, policy)`, `RuntimePolicyMismatch`, `checkpoint_bundle`

- `Part X — Machine snapshot schema > 84. Snapshot contract` [1372-1379]
  Preview: A machine snapshot is a complete stable-keyed checkpoint of one committed machine version.

- `Part X — Machine snapshot schema > 85. Snapshot common fields` [1380-1404]
  Preview: Every `machine_snapshot` payload contains: Lifecycle-specific fields are carried inside the `lifecycle` variant.
  Symbols: `machine_snapshot`, `lifecycle`

- `Part X — Machine snapshot schema > 86. Uninitialized snapshot` [1405-1429]
  Preview: The uninitialized lifecycle variant contains: It contains no fabricated: If a future implementation allows a pre-initialization semantic fact not listed here, that fact must be versioned and persisted explicitly.

- `Part X — Machine snapshot schema > 87. Ready snapshot` [1430-1445]
  Preview: The ready lifecycle variant contains: The schedule is derived from the pending event calendar and is not authoritative persisted state.

- `Part X — Machine snapshot schema > 88. Stable-keyed state tables` [1446-1462]
  Preview: Node and temporal state are encoded as sorted arrays keyed by `NodeKey`.
  Symbols: `NodeKey`

- `Part X — Machine snapshot schema > 89. Complete settled level facts` [1463-1481]
  Preview: A ready snapshot contains settled level facts for every inspectable current level port and endpoint required by the public inspection contract.

- `Part X — Machine snapshot schema > 90. Pulse state` [1482-1489]
  Preview: Pulse counts do not persist as current port state after a reaction.
  Symbols: `optional_history`

- `Part X — Machine snapshot schema > 91. Built-in state schemas` [1490-1508]
  Preview: The snapshot schema provides closed variants for every built-in state family.
  Symbols: `PendingEventKey`

- `Part X — Machine snapshot schema > 92. Pending event records` [1509-1560]
  Preview: Each pending event record contains: The initial event-kind payloads are: The event kind must agree with the owning built-in node kind.
  Symbols: `PendingEventKey`

- `Part X — Machine snapshot schema > 93. Public pending-event identity` [1561-1572]
  Preview: The public persisted `PendingEventKey` is a machine-local unsigned 64-bit monotonic serial.
  Symbols: `PendingEventKey`, `next_pending_event_serial`

- `Part X — Machine snapshot schema > 94. Private event storage reconstruction` [1573-1585]
  Preview: Restoration may place events into any private arena, heap, ordered map, or compact calendar representation.

- `Part X — Machine snapshot schema > 95. External output baselines` [1586-1602]
  Preview: For each external level output in a ready snapshot, the snapshot stores: A ready machine must have an established baseline for every current external level output.

- `Part X — Machine snapshot schema > 96. Active diagnostic episodes` [1603-1622]
  Preview: Each active episode record contains: Episode identity is recomputed from its stable semantic identity rule and verified.

- `Part X — Machine snapshot schema > 97. Required provenance section` [1623-1637]
  Preview: The required provenance section contains: The complete backward closure of every required root must be present.

- `Part X — Machine snapshot schema > 98. Optional history section` [1638-1654]
  Preview: Optional history is explicitly partitioned into named retention classes, for example: Each class records its retention boundary and completeness claim.

- `Part X — Machine snapshot schema > 99. Persistence metadata` [1655-1669]
  Preview: A snapshot may carry non-semantic persistence metadata such as: Wall-clock timestamps, when supplied by the caller, are metadata only and must not be confused with logical time.
  Symbols: `SnapshotDigest`

- `Part X — Machine snapshot schema > 100. Snapshot creation` [1670-1686]
  Preview: Snapshot creation observes one complete committed machine version.
  Symbols: `SnapshotDigest`

- `Part X — Machine snapshot schema > 101. Snapshot validation phases` [1687-1706]
  Preview: Restoration performs these phases in order: 1.

- `Part X — Machine snapshot schema > 102. Recomputed settled state` [1707-1722]
  Preview: For a ready snapshot, restoration must use the full reference evaluator or an equivalent complete consistency check to establish that persisted current settled values agree with: This check performs no new logical reaction, emits no events, and changes no state.

- `Part X — Machine snapshot schema > 103. Snapshot digest validation` [1723-1736]
  Preview: The decoder recomputes: Every persisted claimed digest must match.

- `Part X — Machine snapshot schema > 104. Runtime policy restoration` [1737-1753]
  Preview: The ordinary `CompiledNetwork::restore(snapshot, policy)` path requires: A mismatch is `RuntimePolicyMismatch`.
  Symbols: `CompiledNetwork::restore(snapshot, policy)`, `RuntimePolicyMismatch`

- `Part X — Machine snapshot schema > 105. No implicit topology migration` [1754-1767]
  Preview: Ordinary restoration requires exact compatible network fingerprint and topology revision context.

- `Part X — Machine snapshot schema > 106. Checkpoint bundle` [1768-1794]
  Preview: A `checkpoint_bundle` is a portable self-contained artifact containing: The bundle validates each component independently and then validates cross-component identity: A snapshot alone remains valid and intentionally requires a compatible compiled network and policy supplied externally.
  Symbols: `checkpoint_bundle`

- `Part XI — Replay persistence` [1795-1959]
  Preview: A persisted replay frame contains: `resulting_observable_digest` is required in the persisted v1 frame even if an older in-memory convenience type exposes only the execution digest.
  Symbols: `resulting_observable_digest`, `i`, `ReplayLogContentDigest`, `SnapshotDigest`

- `Part XI — Replay persistence > 107. Replay frame payload` [1797-1815]
  Preview: A persisted replay frame contains: `resulting_observable_digest` is required in the persisted v1 frame even if an older in-memory convenience type exposes only the execution digest.
  Symbols: `resulting_observable_digest`

- `Part XI — Replay persistence > 108. Replay frame construction` [1816-1823]
  Preview: A frame may be constructed only from a successful committed transaction or an explicitly recorded successful forecast whose hypothetical status remains marked.

- `Part XI — Replay persistence > 109. Patch-bearing replay frames` [1824-1838]
  Preview: A patch-bearing frame stores the normalized patch rather than private prepared data.

- `Part XI — Replay persistence > 110. Replay log payload` [1839-1857]
  Preview: A replay log contains: Frame order is semantic and must not be sorted or otherwise normalized by content.

- `Part XI — Replay persistence > 111. Replay chain validation` [1858-1876]
  Preview: Before applying frame `i`, replay verifies: After applying it, replay verifies: When a recorded transaction result is present, its canonical content must also match the actual result after normalization of representation-only identifiers.
  Symbols: `i`

- `Part XI — Replay persistence > 112. Replay log content digest` [1877-1889]
  Preview: A replay log may expose `ReplayLogContentDigest` computed over: Presentation metadata is excluded from the content digest but included in the envelope integrity digest.
  Symbols: `ReplayLogContentDigest`

- `Part XI — Replay persistence > 113. Replay concatenation` [1890-1906]
  Preview: Two logs may be concatenated only when: Concatenation preserves frame order and reindexes only a non-semantic local display index if necessary.

- `Part XI — Replay persistence > 114. Chunked replay` [1907-1925]
  Preview: A large replay log may be stored as chunks.

- `Part XI — Replay persistence > 115. Replay execution failure` [1926-1941]
  Preview: Non-atomic replay stops at the first failed or divergent frame and retains the machine state reached by all prior verified frames.

- `Part XI — Replay persistence > 116. Replay and optional history` [1942-1949]
  Preview: Replay is required to reproduce execution and required observable state.
  Symbols: `SnapshotDigest`

- `Part XI — Replay persistence > 117. Checkpoint and resume` [1950-1959]
  Preview: A replay stream may begin from any compatible machine snapshot checkpoint.

- `Part XII — Audit artifacts` [1960-2026]
  Preview: A `transaction_result` artifact contains the complete immutable semantic result: It also carries a `TransactionResultContentDigest` over the semantic result projection.
  Symbols: `transaction_result`, `TransactionResultContentDigest`, `migration_report`

- `Part XII — Audit artifacts > 118. Transaction result artifact` [1962-1982]
  Preview: A `transaction_result` artifact contains the complete immutable semantic result: It also carries a `TransactionResultContentDigest` over the semantic result projection.
  Symbols: `transaction_result`, `TransactionResultContentDigest`

- `Part XII — Audit artifacts > 119. Cause references in results` [1983-1988]
  Preview: Every cause referenced by a persisted result resolves through the result's own content-addressed provenance section or an explicit included checkpoint root.

- `Part XII — Audit artifacts > 120. Migration report artifact` [1989-2008]
  Preview: A `migration_report` artifact contains every structural, state, pending-event, output-baseline, provenance, diagnostic-episode, region, and invalidated-artifact outcome required by the topology-patch specification.
  Symbols: `migration_report`

- `Part XII — Audit artifacts > 121. Diagnostics in audit artifacts` [2009-2026]
  Preview: Diagnostics are persisted structurally by: Rendered prose may be persisted as presentation metadata but is not authoritative and does not participate in diagnostic semantic comparison.

- `Part XIII — Compatibility rules by artifact family` [2027-2130]
  Preview: A network or module definition may be syntax-decoded under an older supported schema and upgraded when the upgrade is representation-only.
  Symbols: `InvalidData`

- `Part XIII — Compatibility rules by artifact family > 122. Definition compatibility` [2029-2038]
  Preview: A network or module definition may be syntax-decoded under an older supported schema and upgraded when the upgrade is representation-only.

- `Part XIII — Compatibility rules by artifact family > 123. Snapshot compatibility` [2039-2046]
  Preview: Ordinary snapshot restoration requires exact compatibility of all state-interpreting semantic components.

- `Part XIII — Compatibility rules by artifact family > 124. Replay compatibility` [2047-2054]
  Preview: Replay requires stricter compatibility than one-time snapshot restoration.

- `Part XIII — Compatibility rules by artifact family > 125. Patch compatibility` [2055-2062]
  Preview: A patch artifact is compatible only when every operation and migration directive has the exact declared meaning.

- `Part XIII — Compatibility rules by artifact family > 126. Provenance compatibility` [2063-2078]
  Preview: A provenance schema upgrade may rename or reorganize representation only if it preserves: A change to what constitutes a valid explanation is a semantic change.

- `Part XIII — Compatibility rules by artifact family > 127. Diagnostic compatibility` [2079-2093]
  Preview: Changing rendered wording does not require a diagnostic semantic version change.

- `Part XIII — Compatibility rules by artifact family > 128. Digest-suite change` [2094-2106]
  Preview: A new digest suite changes every identity computed under that suite.

- `Part XIII — Compatibility rules by artifact family > 129. Canonical-encoding change` [2107-2112]
  Preview: A canonical-encoding change may preserve semantic fingerprints only if those fingerprints use an unchanged independent semantic projection and digest-domain version.

- `Part XIII — Compatibility rules by artifact family > 130. Compatibility failure structure` [2113-2130]
  Preview: A compatibility failure identifies: It must not collapse all incompatibility into `InvalidData`.
  Symbols: `InvalidData`

- `Part XIV — Public persistence API responsibilities` [2131-2301]
  Preview: The crate should expose a module broadly equivalent to: It owns canonical encoding, decoding, version inspection, compatibility policy, artifact envelopes, and persistence-specific failures.
  Symbols: `serde`, `ArtifactBytes`, `TransactionRecord<D>`, `DecodePolicy`

- `Part XIV — Public persistence API responsibilities > 131. Persistence module` [2133-2144]
  Preview: The crate should expose a module broadly equivalent to: It owns canonical encoding, decoding, version inspection, compatibility policy, artifact envelopes, and persistence-specific failures.

- `Part XIV — Public persistence API responsibilities > 132. Encoded artifact bytes` [2145-2156]
  Preview: The canonical owned byte value is broadly equivalent to: It provides immutable access to the complete prefixed canonical artifact.
  Symbols: `ArtifactBytes`

- `Part XIV — Public persistence API responsibilities > 133. Artifact header inspection` [2157-2172]
  Preview: A bounded header-inspection API may expose: Header inspection must still validate the fixed prefix, envelope framing, canonical header representation, and configured size limits.

- `Part XIV — Public persistence API responsibilities > 134. Encoding APIs` [2173-2189]
  Preview: Canonical encoding methods should exist for each persistable artifact family.

- `Part XIV — Public persistence API responsibilities > 135. Decoding APIs` [2190-2207]
  Preview: Decoding is artifact-specific.
  Symbols: `TransactionRecord<D>`

- `Part XIV — Public persistence API responsibilities > 136. Decode policy` [2208-2223]
  Preview: `DecodePolicy` contains resource bounds and accepted version policies.
  Symbols: `DecodePolicy`

- `Part XIV — Public persistence API responsibilities > 137. Decode limits` [2224-2247]
  Preview: The policy provides explicit limits for at least: The decoder performs checked arithmetic before allocation.

- `Part XIV — Public persistence API responsibilities > 138. Restoration report` [2248-2255]
  Preview: Restoration returns either one complete machine or a structured failure.

- `Part XIV — Public persistence API responsibilities > 139. Transaction record materialization API` [2256-2270]
  Preview: The persistence layer should expose a method broadly equivalent to: Replay may use an internal equivalent that reuses the same patch preparation and input-schema validation.

- `Part XIV — Public persistence API responsibilities > 140. Streaming replay` [2271-2282]
  Preview: A replay decoder may stream frames without loading the entire log when: - the envelope or chunk framing remains canonical; - each frame's integrity and schema are validated before application; - resource limits remain enforceable; - replay stops precisely at the first failure; - partial application semantics are explicit.

- `Part XIV — Public persistence API responsibilities > 141. Serde boundary` [2283-2293]
  Preview: The canonical wire format is not defined by Rust `serde` data-model defaults.
  Symbols: `serde`

- `Part XIV — Public persistence API responsibilities > 142. Human-readable projections` [2294-2301]
  Preview: Debug JSON, YAML, or textual diagnostic exports may exist for tooling.

- `Part XV — Robustness, security, and storage integration` [2302-2385]
  Preview: Malformed artifacts must produce structured failures without: - panics; - unchecked arithmetic; - uncontrolled recursion; - allocation from unvalidated lengths; - partial machine publication; - execution of host callbacks; - mutation of caller-owned state.
  Symbols: `RuntimePolicy`

- `Part XV — Robustness, security, and storage integration > 143. Hostile input handling` [2304-2315]
  Preview: Malformed artifacts must produce structured failures without: - panics; - unchecked arithmetic; - uncontrolled recursion; - allocation from unvalidated lengths; - partial machine publication; - execution of host callbacks; - mutation of caller-owned state.

- `Part XV — Robustness, security, and storage integration > 144. Resource exhaustion` [2316-2323]
  Preview: Decode budgets are separate from machine `RuntimePolicy`.
  Symbols: `RuntimePolicy`

- `Part XV — Robustness, security, and storage integration > 145. Integrity and truncation` [2324-2338]
  Preview: The fixed prefix, canonical envelope framing, definite lengths, and integrity digest provide deterministic detection of truncation and accidental byte modification with cryptographic-hash confidence.

- `Part XV — Robustness, security, and storage integration > 146. Confidentiality` [2339-2346]
  Preview: Snapshots and replay logs may contain sensitive caller metadata, network structure, inputs, and execution history.

- `Part XV — Robustness, security, and storage integration > 147. Authenticity` [2347-2352]
  Preview: An artifact with a valid integrity digest may still have been created or modified by an untrusted party who recomputed the digest.

- `Part XV — Robustness, security, and storage integration > 148. Atomic file replacement` [2353-2367]
  Preview: The core returns bytes and does not own file I/O.

- `Part XV — Robustness, security, and storage integration > 149. Partial or appended files` [2368-2373]
  Preview: A standalone canonical artifact contains exactly one envelope and no trailing data.

- `Part XV — Robustness, security, and storage integration > 150. Recovery tools` [2374-2385]
  Preview: A recovery tool may inspect syntactically valid substructures or report corruption locations.

- `Part XVI — Verification obligations` [2386-2651]
  Preview: For every supported artifact value `A`: For every accepted canonical byte sequence `B`: Noncanonical alternative encodings of the same abstract CBOR value must be rejected.
  Symbols: `M_r`

- `Part XVI — Verification obligations > 151. Canonical uniqueness` [2388-2404]
  Preview: For every supported artifact value `A`: For every accepted canonical byte sequence `B`: Noncanonical alternative encodings of the same abstract CBOR value must be rejected.

- `Part XVI — Verification obligations > 152. Golden vectors` [2405-2428]
  Preview: The project must retain versioned golden byte vectors for representative: Golden vectors must include expected fingerprints and every applicable digest.

- `Part XVI — Verification obligations > 153. Permutation invariance` [2429-2443]
  Preview: Tests must construct semantically equivalent artifacts under permutations of: Canonical bytes and semantic digests must remain equal wherever the order is non-semantic.

- `Part XVI — Verification obligations > 154. Order sensitivity` [2444-2456]
  Preview: Tests must also prove that semantically ordered sequences remain order-sensitive, including: Canonicalization must not sort away semantic order.

- `Part XVI — Verification obligations > 155. Snapshot round trip` [2457-2466]
  Preview: For every generated valid machine, including uninitialized machines: must produce a semantically equivalent machine under the compatible compiled topology, time-domain context, and runtime policy.

- `Part XVI — Verification obligations > 156. Snapshot sufficiency` [2467-2476]
  Preview: For original machine `M`, restored equivalent machine `M_r`, and every compatible future transaction sequence `T`: Comparison includes execution, outputs, state changes, pending work, diagnostic episodes, provenance roots, schedules, and execution and observable digests.
  Symbols: `M_r`

- `Part XVI — Verification obligations > 157. Settled-state consistency` [2477-2482]
  Preview: Property tests must corrupt persisted settled values while leaving other state and envelope integrity recomputed.

- `Part XVI — Verification obligations > 158. Event identity continuity` [2483-2496]
  Preview: Tests must verify that snapshot and restoration preserve: Private event arena placement may differ.

- `Part XVI — Verification obligations > 159. Provenance canonicalization` [2497-2509]
  Preview: Equivalent provenance DAGs built with different arena identifiers and insertion orders must produce identical: Tests must include shared subgraphs, joint support, multiplicity, checkpoints, and migration derivations.

- `Part XVI — Verification obligations > 160. Provenance corruption` [2510-2524]
  Preview: Required rejection cases include:

- `Part XVI — Verification obligations > 161. Definition validation after decode` [2525-2539]
  Preview: Persisted validated definitions must be revalidated and recompiled.

- `Part XVI — Verification obligations > 162. Patch persistence equivalence` [2540-2553]
  Preview: For every generated valid patch: must be equivalent to preparing the original patch.

- `Part XVI — Verification obligations > 163. Patch-bearing replay` [2554-2564]
  Preview: A recorded patch transaction replayed from the same starting snapshot must: - reconstruct the patch; - re-prepare it; - reproduce the target fingerprint; - reproduce migration outcomes; - reproduce output and diagnostic consequences; - reproduce final execution and observable digests.

- `Part XVI — Verification obligations > 164. Replay chain verification` [2565-2584]
  Preview: Tests must cover:

- `Part XVI — Verification obligations > 165. Historical artifact corpus` [2585-2598]
  Preview: For every supported version vector, CI must retain representative historical artifacts and verify the declared action: Removing support requires an explicit compatibility-policy change and release note.

- `Part XVI — Verification obligations > 166. Digest-domain tests` [2599-2604]
  Preview: Tests must prove that identical payload bytes under different digest domains yield different typed digests.

- `Part XVI — Verification obligations > 167. Malformed encoding corpus` [2605-2626]
  Preview: The fuzz and regression corpus must include:

- `Part XVI — Verification obligations > 168. Decode fault injection` [2627-2643]
  Preview: Fault injection should cover allocation and failure points during: No failure may publish a partial machine or prepared patch.

- `Part XVI — Verification obligations > 169. Deterministic encoder independence` [2644-2651]
  Preview: At least one test implementation or golden-vector validator should be sufficiently independent from the production encoder to detect shared ordering or field-omission defects.

- `Part XVII — Implementation guidance and prohibitions` [2652-2770]
  Preview: A recommended implementation pipeline is: Decoding reverses the pipeline but inserts validation before every trusted type transition.
  Symbols: `MachineSnapshotArtifact`

- `Part XVII — Implementation guidance and prohibitions > 170. Recommended internal pipeline` [2654-2675]
  Preview: A recommended implementation pipeline is: Decoding reverses the pipeline but inserts validation before every trusted type transition.

- `Part XVII — Implementation guidance and prohibitions > 171. Shared schema definitions` [2676-2687]
  Preview: The encoder, decoder, digest projection, golden-vector generator, and documentation should derive from one reviewed schema description where practical.

- `Part XVII — Implementation guidance and prohibitions > 172. No raw memory serialization` [2688-2703]
  Preview: The implementation must not persist:

- `Part XVII — Implementation guidance and prohibitions > 173. No derived-cache trust` [2704-2711]
  Preview: Persisted caches may exist only as optional accelerators in a separately named cache artifact.

- `Part XVII — Implementation guidance and prohibitions > 174. No hidden compatibility heuristics` [2712-2723]
  Preview: The implementation must not accept an artifact because: - a Rust deserializer happened to populate current fields; - unknown fields were ignored; - a fingerprint was absent; - names looked similar; - state record byte sizes matched; - a pending event variant had a familiar prefix; - a newer enum discriminant fit in the old integer type.

- `Part XVII — Implementation guidance and prohibitions > 175. No silent repair` [2724-2741]
  Preview: The canonical decoder and restorer must not silently: An explicit recovery tool may produce a new artifact and report every repair, but that artifact begins a new trust and replay boundary.

- `Part XVII — Implementation guidance and prohibitions > 176. Performance expectations` [2742-2749]
  Preview: Canonical persistence is optimized for correctness, deterministic identity, and durable compatibility before minimum byte size.
  Symbols: `MachineSnapshotArtifact`

- `Part XVII — Implementation guidance and prohibitions > 177. Deferred additions` [2750-2770]
  Preview: Future specifications may add: Each addition must preserve the canonical semantic artifact boundary defined here.

- `Part XVIII — Required guarantees` [2771-2831]
  Preview: The implementation must provide: The implementation must make these outcomes impossible through ordinary canonical APIs: This specification governs durable representation and compatibility.
  Symbols: `TimeDomainId`, `PersistenceContext<D>`, `TransactionRecord<D>`

- `Part XVIII — Required guarantees > 178. Persistence guarantees` [2773-2795]
  Preview: The implementation must provide:

- `Part XVIII — Required guarantees > 179. Prohibited outcomes` [2796-2813]
  Preview: The implementation must make these outcomes impossible through ordinary canonical APIs:

- `Part XVIII — Required guarantees > 180. Relationship to other specifications` [2814-2831]
  Preview: This specification governs durable representation and compatibility.
  Symbols: `TimeDomainId`, `PersistenceContext<D>`, `TransactionRecord<D>`

- `Appendix A — Canonical artifact field summary` [2832-2922]

- `Appendix A — Canonical artifact field summary > A.1 Definition artifacts` [2834-2847]

- `Appendix A — Canonical artifact field summary > A.2 Runtime input and transaction artifacts` [2848-2879]

- `Appendix A — Canonical artifact field summary > A.3 Runtime checkpoint and replay artifacts` [2880-2909]

- `Appendix A — Canonical artifact field summary > A.4 Audit artifacts` [2910-2922]

- `Appendix B — Digest scope summary` [2923-2936]
  Preview: | Identity | Covers | Excludes | |---|---|---| | `NetworkFingerprint` | Stable semantic topology and parameters | Revision, metadata, dense layout | | `ModuleFingerprint` | Stable semantic module interface and internals | Instance placement and metadata | | `RuntimePolicyId` | Semantically relevant runtime limits | Performance-only tuning | | `ExecutionStateDigest` | Future execution, failures, episode state, event identity cursor | Runtime policy, provenance, optional history | | `ObservableStateDigest` | Execution state plus complete required current observation | Optional history | | `SnapshotDigest` | Complete canonical snapshot artifact | Its own digest field | | Artifact integrity digest | Complete canonical artifact envelope content | Fixed prefix and its own field |
  Symbols: `NetworkFingerprint`, `ModuleFingerprint`, `RuntimePolicyId`, `ExecutionStateDigest`, `ObservableStateDigest`, `SnapshotDigest`

- `Appendix C — Restoration compatibility matrix` [2937-2956]
  Preview: | Condition | Ordinary restore | |---|---| | Same schema and semantic versions | Accept if all validation succeeds | | Older representation-only schema with supported upgrader | Upgrade, recompute, then validate | | Different `TimeDomainId` | Reject | | Different network fingerprint | Reject | | Different runtime policy id | Reject on ordinary restore | | Unknown node-state variant | Reject | | Unknown pending-event variant | Reject | | Missing required provenance | Reject | | Optional history omitted | Accept | | Optional history differs | Accept; snapshot digest differs | | Diagnostic wording differs outside authoritative fields | Accept where schema-compatible | | Semantic node law differs | Reject or use separately specified explicit migration | | Digest suite differs with supported artifact upgrader | Verify old, upgrade, recompute; begin new digest identity |
  Symbols: `TimeDomainId`

- `Appendix D — Replay identity` [2957-2974]
  Preview: Exact replay identity is conceptually: The replay frame chain provides incremental verification of this identity.

- `Appendix E — Required topology-patch variant coverage` [2975-3096]
  Preview: The `network_patch` schema must provide one exact persisted variant for every canonical patch operation: The `reassociate` variant must cover: Module migration directives must cover: Node migration directives must cover: Temporal migration policies must cover: Transaction patch attachments must persist the reconfiguration policy exactly: Migration report artifacts must encode the finalized state outcomes: and pending-event outcomes: No future patch or migration variant is persistence-compatible with schema version 1 until it receives a new explicit schema variant and compatibility rule.
  Symbols: `network_patch`, `reassociate`
