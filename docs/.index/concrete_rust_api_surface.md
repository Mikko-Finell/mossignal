## docs/specs/concrete_rust_api_surface.md
- ``mossignal` Concrete Rust API Surface` [1-45]
  Preview: **Status:** Design specification, version 2 **Defines:** Concrete public Rust types, ownership model, construction APIs, standard-module discovery and construction, standard catalogue API, lifecycle transitions, runtime transaction APIs, diagnostic-code model, inspection, explanation, bindings, snapshots, replay, topology-patch operation language, and reconfiguration entry points<br> **Does not define:** Processor internals, serialized wire encodings, performance targets, application integration, or rendered diagnostic prose This specification translates the semantic and architectural model of `mossignal` into a coherent public Rust API.
  Symbols: `mossignal`
  Normative: MUST NOT 1, MUST 1, SHOULD NOT 1, SHOULD 1, MAY 1

- ``mossignal` Concrete Rust API Surface > 1. Purpose` [9-33]
  Preview: This specification translates the semantic and architectural model of `mossignal` into a coherent public Rust API.
  Symbols: `mossignal`

- ``mossignal` Concrete Rust API Surface > 2. Normative language` [34-45]
  Preview: The terms **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.
  Normative: MUST NOT 1, MUST 1, SHOULD NOT 1, SHOULD 1, MAY 1

- `Part I — API design principles` [46-182]
  Preview: The API MUST represent the following statically where practical: The API MAY validate the following dynamically: The library MUST NOT introduce lifetime-heavy or typestate-heavy APIs merely to make rare invalid operations unrepresentable when a precise structured runtime failure is simpler and more composable.
  Symbols: `Level`, `Pulse`
  Normative: MUST NOT 5, MUST 6, MAY 2

- `Part I — API design principles > 3. Static versus dynamic invariants` [48-80]
  Preview: The API MUST represent the following statically where practical: The API MAY validate the following dynamically: The library MUST NOT introduce lifetime-heavy or typestate-heavy APIs merely to make rare invalid operations unrepresentable when a precise structured runtime failure is simpler and more composable.
  Normative: MUST NOT 1, MUST 1, MAY 1

- `Part I — API design principles > 4. Closed semantic universe` [81-96]
  Preview: The core signal and built-in-node universe is closed.
  Symbols: `Level`, `Pulse`
  Normative: MUST NOT 1, MUST 1

- `Part I — API design principles > 5. Owned semantic artifacts` [97-134]
  Preview: The canonical public forms of the following MUST be owned values: This permits storage, replay, persistence, transfer between subsystems, and deterministic testing without borrowing a live builder or machine.
  Normative: MUST NOT 1, MUST 1, MAY 1

- `Part I — API design principles > 6. Opaque representation` [135-164]
  Preview: Types that rely on validated internal invariants MUST have private fields.
  Normative: MUST 2

- `Part I — API design principles > 7. Allocation boundary` [165-182]
  Preview: The public API permits ordinary heap allocation during: - authoring; - validation; - compilation; - patch preparation; - snapshots and replay construction; - inspection and explanation; - creation of committed semantic artifacts such as events, provenance, and transaction results.
  Normative: MUST NOT 2, MUST 1

- `Part II — Crate organization and prelude` [183-235]
  Preview: The initial crate SHOULD expose a module organization broadly equivalent to: The crate SHOULD provide a curated prelude: The prelude SHOULD contain common authoring and execution types only.
  Normative: MUST NOT 1, SHOULD NOT 1, SHOULD 3

- `Part II — Crate organization and prelude > 8. Public module organization` [185-225]
  Preview: The initial crate SHOULD expose a module organization broadly equivalent to: The crate SHOULD provide a curated prelude: The prelude SHOULD contain common authoring and execution types only.
  Normative: SHOULD NOT 1, SHOULD 3

- `Part II — Crate organization and prelude > 9. One cohesive crate` [226-235]
  Preview: The initial public API belongs to one cohesive library crate.
  Normative: MUST NOT 1

- `Part III — Fundamental signal types` [236-333]
  Preview: The signal kinds are zero-sized markers: They MUST NOT be instantiated as runtime signal values.
  Symbols: `LogicLevel`
  Normative: MUST NOT 3, MUST 2, SHOULD 1, MAY 2

- `Part III — Fundamental signal types > 10. Signal-kind markers` [238-272]
  Preview: The signal kinds are zero-sized markers: They MUST NOT be instantiated as runtime signal values.
  Normative: MUST NOT 1, MUST 1, MAY 1

- `Part III — Fundamental signal types > 11. Logic levels` [273-301]
  Preview: `LogicLevel` SHOULD provide: The core signal API MUST NOT use `bool` where the value semantically denotes a signal level.
  Symbols: `LogicLevel`
  Normative: MUST NOT 1, SHOULD 1, MAY 1

- `Part III — Fundamental signal types > 12. Pulse counts` [302-333]
  Preview: The initial public representation is a checked fixed-width unsigned count: It MUST provide: Ordinary arithmetic traits that can silently wrap MUST NOT be implemented.
  Normative: MUST NOT 1, MUST 1

- `Part IV — Exact logical time` [334-429]
  Preview: A caller defines a zero-sized marker type: The marker has no required trait implementation beyond those naturally induced by its use in `PhantomData`.
  Normative: MUST NOT 2, MUST 1

- `Part IV — Exact logical time > 13. Time-domain markers` [336-348]
  Preview: A caller defines a zero-sized marker type: The marker has no required trait implementation beyond those naturally induced by its use in `PhantomData`.

- `Part IV — Exact logical time > 14. Time types` [349-385]
  Preview: The initial public representation is: These types MUST implement value semantics independent of `D`: Their trait implementations MUST NOT require corresponding traits on `D`.
  Normative: MUST NOT 1, MUST 1

- `Part IV — Exact logical time > 15. Time constructors and accessors` [386-429]
  Preview: Infallible `Add` or `Sub` implementations that could overflow or subtract in the wrong direction MUST NOT be provided.
  Normative: MUST NOT 1

- `Part V — Stable identity` [430-733]
  Preview: Canonical stable keys include: Every key MUST: - be an owned copyable value; - carry no borrow of a builder or network; - preserve signal kind where applicable; - have deterministic equality, hashing, and canonical ordering; - expose no semantic meaning through numeric ordering; - be constructible without global mutable state.
  Symbols: `SubjectRef`, `KeyAllocator`, `RegionId`, `InputSchemaFingerprint`, `DiagnosticMeta`, `NetworkFingerprint`, `InspectionQueryDigest`, `PersistedArtifactRef`, `CauseDigest`, `BindingSubjectRef`, `TransactionSubjectRef`, `TransactionRecordArtifact`
  Normative: MUST NOT 1, MUST 6, SHOULD 2, MAY 1

- `Part V — Stable identity > 16. Stable keys` [432-459]
  Preview: Canonical stable keys include: Every key MUST: - be an owned copyable value; - carry no borrow of a builder or network; - preserve signal kind where applicable; - have deterministic equality, hashing, and canonical ordering; - expose no semantic meaning through numeric ordering; - be constructible without global mutable state.
  Normative: MUST 1, SHOULD 1

- `Part V — Stable identity > 17. Key construction` [460-491]
  Preview: The API MUST support both local allocation and explicit durable construction.
  Normative: MUST NOT 1, MUST 3, SHOULD 1, MAY 1

- `Part V — Stable identity > 18. Erased stable keys` [492-543]
  Preview: Heterogeneous tooling uses closed erased enums: Typed keys and typed signal-source keys MUST provide explicit conversion into their erased forms.
  Normative: MUST 2

- `Part V — Stable identity > 19. Subject references` [544-733]
  Preview: Diagnostics, graph results, migration reports, and explanations use: `SubjectRef` is semantic identity, not a rendered label.
  Symbols: `SubjectRef`, `KeyAllocator`, `RegionId`, `InputSchemaFingerprint`, `DiagnosticMeta`, `NetworkFingerprint`, `InspectionQueryDigest`, `PersistedArtifactRef`, `CauseDigest`, `BindingSubjectRef`, `TransactionSubjectRef`, `TransactionRecordArtifact`

- `Part V — Stable identity > 19. Subject references > 19.1 Non-structural diagnostic subject identities` [594-733]
  Preview: Canonical stable keys defined in Part V represent authored structural keys allocated via `KeyAllocator`.
  Symbols: `KeyAllocator`, `SubjectRef`, `RegionId`, `InputSchemaFingerprint`, `DiagnosticMeta`, `NetworkFingerprint`, `InspectionQueryDigest`, `PersistedArtifactRef`, `CauseDigest`, `BindingSubjectRef`, `TransactionSubjectRef`, `TransactionRecordArtifact`

- `Part VI — Metadata and human-readable identity` [734-780]
  Preview: The standard owned metadata type is: The initial public runtime types MUST NOT become generic over arbitrary metadata payloads.
  Symbols: `OriginRef`, `DiagnosticMeta`
  Normative: MUST NOT 3, SHOULD 4

- `Part VI — Metadata and human-readable identity > 20. Diagnostic metadata` [736-770]
  Preview: The standard owned metadata type is: The initial public runtime types MUST NOT become generic over arbitrary metadata payloads.
  Symbols: `OriginRef`
  Normative: MUST NOT 2, SHOULD 2

- `Part VI — Metadata and human-readable identity > 21. Metadata inputs` [771-780]
  Preview: Builder APIs SHOULD accept borrowed or owned text through `impl Into<String>` or `DiagnosticMeta` values.
  Symbols: `DiagnosticMeta`
  Normative: MUST NOT 1, SHOULD 2

- `Part VII — Typed network authoring` [781-1827]
  Preview: A builder owns: - one network identity; - a caller-local stable-key allocator; - authored nodes, ports, endpoints, modules, and connections; - builder-scoped signal handles; - authoring diagnostics.
  Symbols: `NetworkBuilder<D>`, `Signal<S>`, `ModuleDef<D>`, `NodeKey`, `StandardCatalogue::build`, `ModuleBuilder<D>`, `finish`, `AuthoringFailure::ForeignSignal`, `constant`, `low`, `high`, `Constant`, `add_<method>`, `DiagnosticMeta`, `AddedNode<O>`, `xor`, `debounce`, `Parity`, `InertialDelay`, `StandardModuleId`, `mossignal.standard.*`, `latest`, `StandardModuleRef`, `Report<ModuleDef<D>, D>`, `ModuleInputKey<Level>`, `ModuleInputKey<S>`, `ModuleOutputKey<S>`, `ModuleOrigin::User`, `ModuleFingerprint`, `ModuleInstanceKey`
  Normative: MUST NOT 9, MUST 27, SHOULD 7, MAY 7

- `Part VII — Typed network authoring > 22. `NetworkBuilder<D>`` [783-808]
  Preview: A builder owns: - one network identity; - a caller-local stable-key allocator; - authored nodes, ports, endpoints, modules, and connections; - builder-scoped signal handles; - authoring diagnostics.
  Symbols: `NetworkBuilder<D>`
  Normative: MUST NOT 1

- `Part VII — Typed network authoring > 23. Builder-scoped signals` [809-841]
  Preview: `Signal<S>` SHOULD implement `Clone` and `Copy`.
  Symbols: `Signal<S>`, `AuthoringFailure::ForeignSignal`
  Normative: MUST NOT 1, MUST 1, SHOULD 1, MAY 1

- `Part VII — Typed network authoring > 24. External input authoring` [842-879]
  Preview: Convenience construction allocates stable identity locally: Explicit durable identity uses metadata-bearing forms: The explicit forms MUST reject duplicate keys immediately where determinable.
  Normative: MUST 1

- `Part VII — Typed network authoring > 25. External output authoring` [880-919]
  Preview: Convenience forms: Explicit forms:

- `Part VII — Typed network authoring > 26. Added-node handles` [920-950]
  Preview: Explicit node construction returns: For a one-output node, `O` MAY be `Signal<S>`.
  Symbols: `Signal<S>`, `NodeKey`
  Normative: MUST 1, MAY 2

- `Part VII — Typed network authoring > 27. Primitive construction style` [951-999]
  Preview: Simple nodes SHOULD use direct arguments: Variadic nodes accept `IntoIterator`: The iterator is consumed immediately.
  Normative: MUST NOT 1, SHOULD 1

- `Part VII — Typed network authoring > 28. Explicit keyed primitive construction` [1000-1019]
  Preview: Every primitive MUST have an explicit keyed form broadly equivalent to: Port keys for fixed-shape nodes MAY be derived deterministically from the node key or allocated as part of the node definition, provided they remain stable and inspectable.
  Normative: MUST 2, MAY 1

- `Part VII — Typed network authoring > 29. Built-in configuration values` [1020-1114]
  Preview: The initial public configuration types are: Configuration structs SHOULD remain small value types.
  Normative: MUST 1, SHOULD 1

- `Part VII — Typed network authoring > 30. Complete primitive constructor family` [1115-1289]
  Preview: The typed builder MUST expose the complete initial primitive family with names broadly equivalent to: `constant`, `low`, and `high` are infallible because they consume no foreign signal handles.
  Symbols: `constant`, `low`, `high`, `Constant`, `add_<method>`, `NodeKey`, `DiagnosticMeta`, `AddedNode<O>`
  Normative: MUST NOT 1, MUST 2, SHOULD 1, MAY 1

- `Part VII — Typed network authoring > 31. Primitive aliases and standard modules` [1290-1615]
  Preview: A primitive alias such as `xor` or `debounce` MAY have a concise method: The resulting canonical node kind remains `Parity` or `InertialDelay` respectively.
  Symbols: `NetworkBuilder<D>`, `ModuleBuilder<D>`, `xor`, `debounce`, `Parity`, `InertialDelay`, `StandardModuleId`, `mossignal.standard.*`, `latest`, `StandardModuleRef`, `StandardCatalogue::build`, `Report<ModuleDef<D>, D>`, `Signal<S>`, `ModuleInputKey<Level>`, `ModuleDef<D>`
  Normative: MUST NOT 3, MUST 11, SHOULD 1, MAY 1

- `Part VII — Typed network authoring > 31. Primitive aliases and standard modules > 31.1 Primitive aliases` [1292-1307]
  Preview: A primitive alias such as `xor` or `debounce` MAY have a concise method: The resulting canonical node kind remains `Parity` or `InertialDelay` respectively.
  Symbols: `xor`, `debounce`, `Parity`, `InertialDelay`
  Normative: MAY 1

- `Part VII — Typed network authoring > 31. Primitive aliases and standard modules > 31.2 Catalogue identity` [1308-1330]
  Preview: The public identity types are: The numeric version wrappers MUST provide checked constructors or crate-generated constants and read-only numeric accessors.
  Symbols: `StandardModuleId`, `mossignal.standard.*`
  Normative: MUST NOT 1, MUST 2

- `Part VII — Typed network authoring > 31. Primitive aliases and standard modules > 31.3 Catalogue and descriptors` [1331-1401]
  Preview: The current catalogue is obtained explicitly: `latest` is an authoring convenience only.
  Symbols: `latest`, `StandardModuleRef`
  Normative: MUST NOT 1, MUST 3

- `Part VII — Typed network authoring > 31. Primitive aliases and standard modules > 31.4 Dynamic requests and parameter values` [1402-1458]
  Preview: Construction MUST preserve malformed combinations long enough for catalogue validation to report independent findings: The request contains one exact descriptor reference, explicit parameter assignments, and explicit variadic public input keys.
  Symbols: `StandardCatalogue::build`, `Report<ModuleDef<D>, D>`
  Normative: MUST 1

- `Part VII — Typed network authoring > 31. Primitive aliases and standard modules > 31.5 Typed construction results` [1459-1483]
  Preview: A one-output standard module uses `Signal<S>` as `O`.
  Symbols: `Signal<S>`

- `Part VII — Typed network authoring > 31. Primitive aliases and standard modules > 31.6 Concise standard-module constructors` [1484-1541]
  Preview: `NetworkBuilder<D>` MUST provide: `ModuleBuilder<D>` MUST provide the same inherent methods with the same signatures so user-defined modules may contain standard modules.
  Symbols: `NetworkBuilder<D>`, `ModuleBuilder<D>`
  Normative: MUST 2, SHOULD 1

- `Part VII — Typed network authoring > 31. Primitive aliases and standard modules > 31.7 Explicit keyed standard-module constructors` [1542-1609]
  Preview: `NetworkBuilder<D>` MUST also provide: `ModuleBuilder<D>` MUST expose the same explicit family.
  Symbols: `NetworkBuilder<D>`, `ModuleBuilder<D>`, `ModuleInputKey<Level>`
  Normative: MUST 2

- `Part VII — Typed network authoring > 31. Primitive aliases and standard modules > 31.8 Canonical equivalence of construction paths` [1610-1615]
  Preview: Typed convenience methods, explicit keyed methods, and generic instantiation of a catalogue-generated `ModuleDef<D>` MUST converge on the same exact standard declaration and canonical expanded module structure for equivalent identities, parameters, and public keys.
  Symbols: `ModuleDef<D>`
  Normative: MUST NOT 1, MUST 1

- `Part VII — Typed network authoring > 32. Signal and port metadata` [1616-1635]
  Preview: Intermediate signals may be named or annotated without inserting a semantic identity node.
  Normative: MUST NOT 1, SHOULD 1

- `Part VII — Typed network authoring > 33. Reusable module definitions` [1636-1741]
  Preview: Reusable modules use the typed stable `ModuleInputKey<S>` and `ModuleOutputKey<S>` interface keys defined with the other structural identities.
  Symbols: `ModuleDef<D>`, `ModuleInputKey<S>`, `ModuleOutputKey<S>`, `NetworkBuilder<D>`, `ModuleOrigin::User`, `ModuleFingerprint`
  Normative: MUST 3, SHOULD 1

- `Part VII — Typed network authoring > 34. Module instantiation` [1742-1812]
  Preview: A module instance is identified by `ModuleInstanceKey` and binds module inputs to builder signals.
  Symbols: `ModuleInstanceKey`, `finish`, `ModuleDef<D>`, `StandardCatalogue::build`
  Normative: MUST 5, MAY 1

- `Part VII — Typed network authoring > 35. Builder completion` [1813-1827]
  Preview: `finish` consumes the builder, materializes the canonical dynamic definition, runs complete validation, and returns a report.
  Symbols: `finish`
  Normative: MUST NOT 1

- `Part VIII — Dynamic network definitions` [1828-1945]
  Preview: It is the canonical public form for: - deserialized network definitions; - editor-authored definitions; - generated definitions; - explicit low-level network construction; - fuzzing malformed structures.
  Symbols: `UncheckedNetwork<D>`, `NodeKind<D>`, `ModuleBindingSet`
  Normative: MUST 6, SHOULD 2, MAY 1

- `Part VIII — Dynamic network definitions > 36. `UncheckedNetwork<D>`` [1830-1845]
  Preview: It is the canonical public form for: - deserialized network definitions; - editor-authored definitions; - generated definitions; - explicit low-level network construction; - fuzzing malformed structures.
  Symbols: `UncheckedNetwork<D>`
  Normative: MUST 1

- `Part VIII — Dynamic network definitions > 37. Dynamic node definitions` [1846-1890]
  Preview: The closed node definition surface SHOULD be equivalent to: `NodeKind<D>` MUST remain closed and non-callback-based.
  Symbols: `NodeKind<D>`
  Normative: MUST 1, SHOULD 1

- `Part VIII — Dynamic network definitions > 38. Dynamic ports and connections` [1891-1930]
  Preview: Dynamic definitions use stable typed keys where the kind is known and erased forms where heterogeneous storage is necessary.
  Symbols: `ModuleBindingSet`
  Normative: MUST 2, MAY 1

- `Part VIII — Dynamic network definitions > 39. Validation` [1931-1945]
  Preview: The consuming form SHOULD be preferred when the caller no longer needs the unchecked definition.
  Normative: MUST 2, SHOULD 1

- `Part IX — Reports and diagnostics` [1946-2110]
  Preview: Required methods: `ReportFailure<D>` MUST retain the complete diagnostic set.
  Symbols: `ReportFailure<D>`, `require_artifact`, `Problem<D>`, `DiagnosticCode`, `ProblemEvidence<D>`, `HashMap<String, String>`, `Diagnostic<D>`, `ReportFinding`, `Display`, `DiagnosticOccurrence<D>`, `RuntimeOccurrence`, `standard_module.*`, `SubjectRef::StandardModule`, `SubjectRef::ModuleDefinition`, `SubjectRef::Module`, `&Diagnostic<D>`, `len`, `is_empty`
  Normative: MUST NOT 5, MUST 4, SHOULD 2, MAY 2

- `Part IX — Reports and diagnostics > 40. `Report<T, D>`` [1948-1974]
  Preview: Required methods: `ReportFailure<D>` MUST retain the complete diagnostic set.
  Symbols: `ReportFailure<D>`, `require_artifact`
  Normative: MUST 1

- `Part IX — Reports and diagnostics > 41. Unified problem records and report findings` [1975-2096]
  Preview: The common owned problem record is: `Problem<D>` MUST expose accessors broadly equivalent to: Its fields SHOULD remain private so arbitrary code-to-evidence, severity, responsibility, and delivery combinations cannot be constructed accidentally.
  Symbols: `Problem<D>`, `DiagnosticCode`, `ProblemEvidence<D>`, `HashMap<String, String>`, `Diagnostic<D>`, `ReportFinding`, `Display`, `DiagnosticOccurrence<D>`, `RuntimeOccurrence`, `standard_module.*`, `SubjectRef::StandardModule`, `SubjectRef::ModuleDefinition`, `SubjectRef::Module`
  Normative: MUST NOT 4, MUST 2, SHOULD 1, MAY 2

- `Part IX — Reports and diagnostics > 42. Diagnostic collections` [2097-2110]
  Preview: It SHOULD provide iteration over `&Diagnostic<D>` in the catalogue's canonical order, filtering by severity or code, and ordinary collection accessors such as `len` and `is_empty`.
  Symbols: `&Diagnostic<D>`, `len`, `is_empty`
  Normative: MUST NOT 1, MUST 1, SHOULD 1

- `Part X — Validation and compilation artifacts` [2111-2206]
  Preview: It MUST be constructible only by successful validation or an internal trusted test path.
  Symbols: `spawn`, `RuntimePolicy`
  Normative: MUST NOT 2, MUST 3, SHOULD 4

- `Part X — Validation and compilation artifacts > 43. `ValidatedNetwork<D>`` [2113-2134]
  Preview: It MUST be constructible only by successful validation or an internal trusted test path.
  Normative: MUST 1, SHOULD 2

- `Part X — Validation and compilation artifacts > 44. `CompiledNetwork<D>`` [2135-2177]
  Preview: It SHOULD implement cheap `Clone` through hidden immutable shared ownership.
  Symbols: `spawn`, `RuntimePolicy`
  Normative: MUST NOT 1, MUST 1, SHOULD 1

- `Part X — Validation and compilation artifacts > 45. Revision and fingerprints` [2178-2206]
  Preview: These types MUST be distinct and non-interchangeable.
  Normative: MUST NOT 1, MUST 1, SHOULD 1

- `Part XI — Resolved handles` [2207-2251]
  Preview: Stable keys remain the canonical public identity.
  Symbols: `Machine<D>`
  Normative: MUST NOT 1, MUST 1, MAY 1

- `Part XI — Resolved handles > 46. Revision-bound resolution` [2209-2227]
  Preview: Stable keys remain the canonical public identity.
  Normative: MAY 1

- `Part XI — Resolved handles > 47. Resolution APIs` [2228-2251]
  Preview: Resolution belongs to `Machine<D>` because topology revision is machine-local.
  Symbols: `Machine<D>`
  Normative: MUST NOT 1, MUST 1

- `Part XII — Runtime policy` [2252-2286]
  Preview: Construction uses a builder: The plain builder MUST require every semantically relevant limit to be set or must reject `build` with a missing-field failure.
  Symbols: `build`, `RuntimePolicy::conservative()`, `RuntimePolicy::development()`, `RuntimePolicyId`
  Normative: MUST NOT 1, MUST 2, MAY 1

- `Part XII — Runtime policy > 48. `RuntimePolicy`` [2254-2286]
  Preview: Construction uses a builder: The plain builder MUST require every semantically relevant limit to be set or must reject `build` with a missing-field failure.
  Symbols: `build`, `RuntimePolicy::conservative()`, `RuntimePolicy::development()`, `RuntimePolicyId`
  Normative: MUST NOT 1, MUST 2, MAY 1

- `Part XIII — Machine lifecycle and access` [2287-2374]
  Preview: The canonical machine uses runtime lifecycle state rather than a typestate parameter.
  Symbols: `Dormant`, `Machine<D>`
  Normative: MUST NOT 1, MUST 2, MAY 1

- `Part XIII — Machine lifecycle and access > 49. `Machine<D>`` [2289-2305]
  Preview: The canonical machine uses runtime lifecycle state rather than a typestate parameter.

- `Part XIII — Machine lifecycle and access > 50. Machine status` [2306-2335]
  Preview: Required accessors:

- `Part XIII — Machine lifecycle and access > 51. Schedule access` [2336-2355]
  Preview: An uninitialized machine MUST return a not-initialized failure rather than `Dormant`.
  Symbols: `Dormant`
  Normative: MUST 1

- `Part XIII — Machine lifecycle and access > 52. No lifecycle typestate requirement` [2356-2374]
  Preview: The canonical API MUST NOT require: Optional borrowed wrappers MAY be added later: Such wrappers MUST remain convenience views over the same canonical `Machine<D>`.
  Symbols: `Machine<D>`
  Normative: MUST NOT 1, MUST 1, MAY 1

- `Part XIV — Input schemas and values` [2375-2484]
  Preview: `InputSnapshot<D>` and `InputDelta<D>` are owned, opaque, and bound to an exact expected input schema.
  Symbols: `set`, `establish`, `InputSnapshot<D>`, `InputDelta<D>`, `finish`
  Normative: MUST NOT 1, MUST 3

- `Part XIV — Input schemas and values > 53. Network-bound input artifacts` [2377-2389]
  Preview: `InputSnapshot<D>` and `InputDelta<D>` are owned, opaque, and bound to an exact expected input schema.
  Symbols: `InputSnapshot<D>`, `InputDelta<D>`
  Normative: MUST NOT 1

- `Part XIV — Input schemas and values > 54. `InputSnapshot<D>`` [2390-2421]
  Preview: Representative API: The builder MUST diagnose duplicate, unknown, wrong-kind, foreign-schema, and missing required level observations.
  Normative: MUST 1

- `Part XIV — Input schemas and values > 55. `InputDelta<D>`` [2422-2469]
  Preview: Representative API: For an ordinary current-topology delta: - `set` changes or reasserts existing external levels; - `establish` is invalid because no input is new.
  Symbols: `set`, `establish`, `finish`
  Normative: MUST 2

- `Part XIV — Input schemas and values > 56. Prepared-patch input builders` [2470-2484]
  Preview: The snapshot builder targets the resulting topology and is used when the patch participates in machine initialization.

- `Part XV — Transactions` [2485-2568]
  Preview: A transaction is representable as data.
  Symbols: `InputDelta`, `InputSnapshot`, `with_patch`
  Normative: MUST NOT 3, MUST 4, MAY 1

- `Part XV — Transactions > 57. Owned explicit transaction values` [2487-2494]
  Preview: A transaction is representable as data.
  Normative: MUST NOT 1

- `Part XV — Transactions > 58. Distinct constructors` [2495-2516]
  Preview: These constructors make the ordinary lifecycle distinction visible without splitting the machine into typestates.

- `Part XV — Transactions > 59. Transaction options` [2517-2550]
  Preview: When a patch is attached: - the transaction input MUST be bound to the prepared patch’s target input schema; - the prepared patch MUST be bound to the transaction’s expected base revision; - a ready-machine patch transaction uses a target-bound `InputDelta`; - an initialization patch transaction uses a target-bound `InputSnapshot`.
  Symbols: `InputDelta`, `InputSnapshot`, `with_patch`
  Normative: MUST 3

- `Part XV — Transactions > 60. Transaction metadata` [2551-2568]
  Preview: Transaction metadata MUST remain non-semantic: It MAY appear in diagnostics, provenance roots, and replay artifacts but MUST NOT affect evaluator behavior.
  Normative: MUST NOT 2, MUST 1, MAY 1

- `Part XVI — Apply and forecast` [2569-2646]
  Preview: On success: - the complete candidate machine becomes current; - one immutable result is returned.
  Symbols: `RuntimeFailure`, `ForecastState<D>`, `apply`
  Normative: MUST NOT 2, SHOULD 1

- `Part XVI — Apply and forecast > 61. Applying transactions` [2571-2594]
  Preview: On success: - the complete candidate machine becomes current; - one immutable result is returned.
  Symbols: `RuntimeFailure`
  Normative: MUST NOT 1

- `Part XVI — Apply and forecast > 62. Forecasting` [2595-2607]
  Preview: A forecast consumes its transaction and leaves the original machine unchanged.

- `Part XVI — Apply and forecast > 63. Forecast result and hypothetical state` [2608-2646]
  Preview: Required accessors: `ForecastState<D>` SHOULD support the same read-only inspection, explanation, schedule, digest, and snapshot projections as a committed machine.
  Symbols: `ForecastState<D>`, `apply`
  Normative: MUST NOT 1, SHOULD 1

- `Part XVII — Transaction results and changes` [2647-2728]
  Preview: Every `CauseRef` contained in the result's change set MUST resolve through the result's `ProvenanceView<D>`, even if the originating machine later compacts optional provenance.
  Symbols: `CauseRef`, `ProvenanceView<D>`, `OutputEvent<D>`
  Normative: MUST 4, SHOULD 1, MAY 2

- `Part XVII — Transaction results and changes > 64. `TransactionResult<D>`` [2649-2671]
  Preview: Every `CauseRef` contained in the result's change set MUST resolve through the result's `ProvenanceView<D>`, even if the originating machine later compacts optional provenance.
  Symbols: `CauseRef`, `ProvenanceView<D>`
  Normative: MUST 2, MAY 1

- `Part XVII — Transaction results and changes > 65. Semantic change set` [2672-2688]
  Preview: The collection order MUST be deterministic.
  Normative: MUST 1

- `Part XVII — Transaction results and changes > 66. Output events` [2689-2720]
  Preview: `OutputEvent<D>` MUST remain one flat chronological network-wide event stream.
  Symbols: `OutputEvent<D>`
  Normative: MUST 1

- `Part XVII — Transaction results and changes > 67. Change typing` [2721-2728]
  Preview: Heterogeneous state and topology changes SHOULD use closed enums with typed variants rather than unstructured key-value maps.
  Normative: SHOULD 1, MAY 1

- `Part XVIII — Runtime failures` [2729-2800]
  Preview: Runtime execution uses: The public failure hierarchy SHOULD be layered: Wrapper variants are ergonomic categories, not additional semantic conditions.
  Symbols: `Problem<D>`, `Display`, `Other(String)`, `UnknownFailure { message: String }`, `RuntimeFailure`, `internal.*`, `LibraryDefect`, `InternalDefect`, `DefectContext<D>`, `InternalDefect<D>`, `#[non_exhaustive]`, `LogicLevel`
  Normative: MUST NOT 2, MUST 5, SHOULD 2, MAY 4

- `Part XVIII — Runtime failures > 68. Failure layering` [2731-2764]
  Preview: Runtime execution uses: The public failure hierarchy SHOULD be layered: Wrapper variants are ergonomic categories, not additional semantic conditions.
  Symbols: `Problem<D>`, `Display`, `Other(String)`, `UnknownFailure { message: String }`
  Normative: MUST NOT 1, MUST 3, SHOULD 1, MAY 1

- `Part XVIII — Runtime failures > 69. Internal defects` [2765-2792]
  Preview: Internal invariant violations MUST NOT be represented as ordinary `RuntimeFailure` values intended to blame caller-controlled data.
  Symbols: `RuntimeFailure`, `internal.*`, `LibraryDefect`, `InternalDefect`, `DefectContext<D>`, `InternalDefect<D>`
  Normative: MUST NOT 1, MUST 2, MAY 2

- `Part XVIII — Runtime failures > 70. Non-exhaustive public enums` [2793-2800]
  Preview: Public failure, problem-evidence, inspection, explanation, and change enums SHOULD be marked `#[non_exhaustive]` unless exhaustive matching is an intentional long-term compatibility promise.
  Symbols: `#[non_exhaustive]`, `LogicLevel`
  Normative: SHOULD 1, MAY 1

- `Part XIX — Inspection` [2801-2920]
  Preview: Equivalent methods SHOULD exist on `ForecastState<D>`.
  Symbols: `ForecastState<D>`, `ModuleInspection<D>`, `ModuleOrigin<D>`, `CompiledNetwork<D>`, `InspectionFailure::NotInitialized`
  Normative: MUST NOT 1, MUST 6, SHOULD 1

- `Part XIX — Inspection > 71. Direct inspection` [2803-2830]
  Preview: Equivalent methods SHOULD exist on `ForecastState<D>`.
  Symbols: `ForecastState<D>`
  Normative: SHOULD 1

- `Part XIX — Inspection > 72. Owned inspection values` [2831-2862]
  Preview: Canonical inspection results are owned: They MUST identify the observed revision and logical time where applicable.
  Symbols: `ModuleInspection<D>`, `ModuleOrigin<D>`
  Normative: MUST 4

- `Part XIX — Inspection > 73. Stable queries` [2863-2886]
  Preview: Representative builder methods:

- `Part XIX — Inspection > 74. Compiled plans` [2887-2912]
  Preview: Plan compilation and execution belong to the machine because topology revision is machine-local: The plan MUST fail structurally when its fingerprint or revision is stale.
  Normative: MUST NOT 1, MUST 1

- `Part XIX — Inspection > 75. Structural inspection before initialization` [2913-2920]
  Preview: Structural graph and definition inspection belongs to `CompiledNetwork<D>` and remains available before machine initialization.
  Symbols: `CompiledNetwork<D>`, `InspectionFailure::NotInitialized`
  Normative: MUST 1

- `Part XX — Explanation and provenance` [2921-3001]
  Preview: A cause reference is meaningful only together with the machine or owned artifact that retains its provenance.
  Symbols: `TransactionResult<D>`, `ProvenanceView<D>`, `ForecastState<D>`
  Normative: MUST NOT 1, MUST 5, SHOULD 1

- `Part XX — Explanation and provenance > 76. Cause references` [2923-2944]
  Preview: A cause reference is meaningful only together with the machine or owned artifact that retains its provenance.
  Symbols: `TransactionResult<D>`, `ProvenanceView<D>`
  Normative: MUST NOT 1, MUST 1

- `Part XX — Explanation and provenance > 77. Explanation requests` [2945-2977]
  Preview: Execution: Equivalent read-only execution SHOULD exist on `ForecastState<D>`.
  Symbols: `ForecastState<D>`
  Normative: SHOULD 1

- `Part XX — Explanation and provenance > 78. Explanation values` [2978-3001]
  Preview: The explanation API MUST return structured data independent from rendered prose.
  Normative: MUST 4

- `Part XXI — Graph access` [3002-3051]
  Preview: These views borrow immutable artifacts and MUST NOT permit mutation.
  Symbols: `ModuleOrigin<D>`, `NetworkSlice`
  Normative: MUST NOT 2, MUST 1, SHOULD 1, MAY 1

- `Part XXI — Graph access > 79. Graph views` [3004-3016]
  Preview: These views borrow immutable artifacts and MUST NOT permit mutation.
  Symbols: `ModuleOrigin<D>`
  Normative: MUST NOT 2, MUST 1, MAY 1

- `Part XXI — Graph access > 80. Graph queries` [3017-3051]
  Preview: Representative compiled queries: `NetworkSlice` SHOULD be an owned stable-keyed result suitable for later use after the borrowed graph view is dropped.
  Symbols: `NetworkSlice`
  Normative: SHOULD 1

- `Part XXII — External bindings` [3052-3133]
  Preview: Construction: Representative methods: Bindings are outside semantic machine state.
  Symbols: `InputSnapshot<D>`, `InputDelta<D>`
  Normative: MUST NOT 2, MUST 1, SHOULD 1, MAY 1

- `Part XXII — External bindings > 81. Binding sets` [3054-3097]
  Preview: Construction: Representative methods: Bindings are outside semantic machine state.

- `Part XXII — External bindings > 82. Input projection` [3098-3116]
  Preview: The binding set SHOULD provide: The projector converts caller-owned observations into `InputSnapshot<D>` or `InputDelta<D>` while diagnosing missing, duplicate, unknown, ambiguous, wrong-kind, and stale-schema observations.
  Symbols: `InputSnapshot<D>`, `InputDelta<D>`
  Normative: SHOULD 1

- `Part XXII — External bindings > 83. Optional bound-machine façade` [3117-3133]
  Preview: An ergonomic façade MAY exist: It MUST delegate to the same core machine semantics and MUST NOT introduce callbacks into propagation.
  Normative: MUST NOT 2, MUST 1, MAY 1

- `Part XXIII — Reconfiguration` [3134-3698]
  Preview: The public patch model is an owned declarative graph rewrite: The canonical operation language is: Supporting structural references are: A patch is interpreted all at once.
  Symbols: `Low`, `ModuleOrigin::Standard`, `standard_module.noncanonical_internal_edit`, `reconfiguration.*`, `finish`, `StaticMigrationPlan<D>`, `PreparedPatch<D>`, `CompiledNetwork<D>`, `establish`, `ModuleMigrationDirective::Explicit`, `NodeMigrationDirective::Reset`, `initial`, `LevelResettableSampleHold.reset_to`, `Transaction::with_patch`, `SemanticLoss<D>`, `RejectStateLoss`, `AllowReportedStateLoss`, `Machine::forecast`, `InternalDefect<D>`
  Normative: MUST NOT 4, MUST 10, SHOULD 2, MAY 1

- `Part XXIII — Reconfiguration > 84. Patch values and canonical operations` [3136-3271]
  Preview: The public patch model is an owned declarative graph rewrite: The canonical operation language is: Supporting structural references are: A patch is interpreted all at once.
  Symbols: `Low`, `ModuleOrigin::Standard`, `standard_module.noncanonical_internal_edit`
  Normative: MUST NOT 2, MUST 2

- `Part XXIII — Reconfiguration > 85. Patch construction, normalization, and access` [3272-3400]
  Preview: A builder is bound to one network identity, base fingerprint, and explicit machine-local revision: The local patch-construction failure family is: Each variant corresponds one-to-one with the applicable `reconfiguration.*` construction code.
  Symbols: `reconfiguration.*`, `finish`
  Normative: MUST 2, SHOULD 1, MAY 1

- `Part XXIII — Reconfiguration > 86. Structural preparation and static migration plan` [3401-3457]
  Preview: Successful structural preparation produces: The supporting public record categories are owned closed values: `StaticMigrationPlan<D>` MUST classify every relevant structural subject.
  Symbols: `StaticMigrationPlan<D>`, `PreparedPatch<D>`
  Normative: MUST 2, SHOULD 1

- `Part XXIII — Reconfiguration > 87. Preparation placement, freshness, and target input` [3458-3498]
  Preview: The canonical preparation method belongs to `CompiledNetwork<D>` because preparation depends on topology but not machine runtime state: The compiled-network form rejects a foreign base network or fingerprint.
  Symbols: `CompiledNetwork<D>`, `establish`
  Normative: MUST NOT 1, MUST 1

- `Part XXIII — Reconfiguration > 88. Migration directives, commitment, and reporting` [3499-3698]
  Preview: Node migration is selected only from the closed built-in family: No arbitrary migration callback, implicit state reset, inferred deadline rule, or name-based structural matching is permitted.
  Symbols: `ModuleMigrationDirective::Explicit`, `NodeMigrationDirective::Reset`, `initial`, `LevelResettableSampleHold.reset_to`, `Transaction::with_patch`, `SemanticLoss<D>`, `RejectStateLoss`, `AllowReportedStateLoss`, `Machine::forecast`, `InternalDefect<D>`
  Normative: MUST NOT 1, MUST 3

- `Part XXIV — Snapshots and restoration` [3699-3755]
  Preview: Snapshot creation: The canonical snapshot is owned and stable-keyed.
  Normative: MUST NOT 2, MUST 2, SHOULD 1, MAY 1

- `Part XXIV — Snapshots and restoration > 89. Machine snapshots` [3701-3720]
  Preview: Snapshot creation: The canonical snapshot is owned and stable-keyed.
  Normative: MUST NOT 1

- `Part XXIV — Snapshots and restoration > 90. Snapshot access` [3721-3740]
  Preview: A snapshot SHOULD expose summary information without exposing mutable internals: Serialized encoding is defined separately.
  Normative: MUST 1, SHOULD 1

- `Part XXIV — Snapshots and restoration > 91. Restoration` [3741-3755]
  Preview: Restoration remains: Restoration MUST return a complete machine or a structured failure.
  Normative: MUST NOT 1, MUST 1, MAY 1

- `Part XXV — Replay` [3756-3821]
  Preview: A frame is an owned value.
  Symbols: `Machine::apply`, `apply_recorded`, `apply`
  Normative: MUST 4, SHOULD 1, MAY 1

- `Part XXV — Replay > 92. Replay frames` [3758-3771]
  Preview: A frame is an owned value.

- `Part XXV — Replay > 93. Frame creation` [3772-3802]
  Preview: A successful transaction result SHOULD support explicit frame construction: Because `Machine::apply` consumes the transaction, callers wishing to retain it for replay MUST either clone it before application or use a helper that records execution: `apply_recorded` MUST use the same transition semantics as `apply`.
  Symbols: `Machine::apply`, `apply_recorded`, `apply`
  Normative: MUST 2, SHOULD 1

- `Part XXV — Replay > 94. Replay execution` [3803-3821]
  Preview: Replay MUST stop at the first incompatible frame and report the exact frame position and structured reason.
  Normative: MUST 2, MAY 1

- `Part XXVI — Persistent diagnostic episodes and observers` [3822-3878]
  Preview: Inspection exposes active episodes through owned records: The condition key contains the code, primary subject, and catalogue-defined condition discriminator.
  Symbols: `current`, `PersistentEpisode`, `Machine<D>`, `SemanticChangeSet<D>`
  Normative: MUST NOT 2, MUST 2, MAY 1

- `Part XXVI — Persistent diagnostic episodes and observers > 95. Active diagnostic episodes` [3824-3861]
  Preview: Inspection exposes active episodes through owned records: The condition key contains the code, primary subject, and catalogue-defined condition discriminator.
  Symbols: `current`, `PersistentEpisode`
  Normative: MUST NOT 2, MUST 2

- `Part XXVI — Persistent diagnostic episodes and observers > 96. Observer separation` [3862-3878]
  Preview: Observer subscriptions are not part of `Machine<D>`.
  Symbols: `Machine<D>`, `SemanticChangeSet<D>`
  Normative: MAY 1

- `Part XXVII — Ownership, cloning, and thread boundaries` [3879-3936]
  Preview: The following SHOULD be cheaply cloneable through immutable shared ownership: Cloning these values SHOULD NOT duplicate full topology graphs or migration programs.
  Symbols: `Arc`, `Machine<D>`, `&mut Machine<D>`, `&Machine<D>`
  Normative: SHOULD NOT 2, SHOULD 4, MAY 2

- `Part XXVII — Ownership, cloning, and thread boundaries > 97. Cheaply cloneable immutable artifacts` [3881-3895]
  Preview: The following SHOULD be cheaply cloneable through immutable shared ownership: Cloning these values SHOULD NOT duplicate full topology graphs or migration programs.
  Symbols: `Arc`
  Normative: SHOULD NOT 1, SHOULD 1

- `Part XXVII — Ownership, cloning, and thread boundaries > 98. Expensive semantic clones` [3896-3922]
  Preview: The following MAY perform work proportional to semantic content when cloned: The API documentation SHOULD avoid promising cheap clone for these types.
  Symbols: `Machine<D>`
  Normative: SHOULD NOT 1, SHOULD 2, MAY 2

- `Part XXVII — Ownership, cloning, and thread boundaries > 99. Mutation and concurrency` [3923-3936]
  Preview: All semantic mutation occurs through `&mut Machine<D>`.
  Symbols: `&mut Machine<D>`, `&Machine<D>`
  Normative: SHOULD 1

- `Part XXVIII — Allocation and string behavior` [3937-3985]
  Preview: Authoring and compilation MAY allocate: - vectors and maps for graph structure; - strings for metadata; - diagnostics and evidence; - validation and compilation workspaces; - compiled immutable arrays and lookup tables.
  Symbols: `Arc<str>`
  Normative: MUST 1, SHOULD 3, MAY 3

- `Part XXVIII — Allocation and string behavior > 100. Authoring and compilation allocations` [3939-3950]
  Preview: Authoring and compilation MAY allocate: - vectors and maps for graph structure; - strings for metadata; - diagnostics and evidence; - validation and compilation workspaces; - compiled immutable arrays and lookup tables.
  Normative: MAY 1

- `Part XXVIII — Allocation and string behavior > 101. Runtime storage` [3951-3971]
  Preview: Spawning a machine MAY allocate persistent runtime storage proportional to compiled topology and policy.
  Normative: SHOULD 1, MAY 1

- `Part XXVIII — Allocation and string behavior > 102. String movement and cloning` [3972-3985]
  Preview: Metadata strings are owned by definitions and artifacts.
  Symbols: `Arc<str>`
  Normative: MUST 1, SHOULD 2, MAY 1

- `Part XXIX — End-to-end example` [3986-4133]
  Preview: The new level input is established explicitly.
  Symbols: `set`

- `Part XXIX — End-to-end example > 103. Typed authoring and initialization` [3988-4050]

- `Part XXIX — End-to-end example > 104. Ready-machine transaction` [4051-4067]

- `Part XXIX — End-to-end example > 105. Forecast and inspection` [4068-4091]

- `Part XXIX — End-to-end example > 106. Patch adding a new external level input` [4092-4133]
  Preview: The new level input is established explicitly.
  Symbols: `set`

- `Part XXX — Public API coherence requirements` [4134-4206]
  Preview: Builder, dynamic definition, validation, compilation, runtime, diagnostics, inspection, explanation, snapshots, replay, bindings, and reconfiguration MUST use the same stable structural keys.
  Symbols: `InputSnapshot`, `InputDelta`, `Low`, `Problem<D>`
  Normative: MUST NOT 3, MUST 5, MAY 1

- `Part XXX — Public API coherence requirements > 107. Shared identity model` [4136-4141]
  Preview: Builder, dynamic definition, validation, compilation, runtime, diagnostics, inspection, explanation, snapshots, replay, bindings, and reconfiguration MUST use the same stable structural keys.
  Normative: MUST 1

- `Part XXX — Public API coherence requirements > 108. Shared lifecycle model` [4142-4151]
  Preview: All public APIs MUST agree that: - a newly spawned machine is uninitialized; - initialization requires a complete target-topology `InputSnapshot`; - ready-machine advancement uses a target-topology `InputDelta`; - schedule and current runtime inspection are unavailable before initialization; - `Low` is not an uninitialized marker.
  Symbols: `InputSnapshot`, `InputDelta`, `Low`
  Normative: MUST 1

- `Part XXX — Public API coherence requirements > 109. Shared revision model` [4152-4157]
  Preview: Resolved handles, inspection plans, prepared patches, input artifacts, transactions, forecasts, snapshots, and replay frames MUST expose or internally retain their revision and topology binding where needed.
  Normative: MUST 2

- `Part XXX — Public API coherence requirements > 110. Shared failure model` [4158-4183]
  Preview: Public stages use: when independent report findings can safely accumulate, and: when the operation has one atomic success or failure outcome.
  Symbols: `Problem<D>`
  Normative: MUST NOT 1

- `Part XXX — Public API coherence requirements > 111. No semantic callbacks` [4184-4189]
  Preview: The evaluator, patch finalizer, snapshot restorer, replay engine, and provenance builder MUST NOT call arbitrary host callbacks as part of semantic execution.
  Normative: MUST NOT 1

- `Part XXX — Public API coherence requirements > 112. No hidden defaults` [4190-4206]
  Preview: The concrete API MUST NOT hide: - initialization level values; - positive temporal spans; - runtime policy; - conflict policy; - re-enable phase policy; - state-loss policy; - migration policy where work is pending; - newly added external level establishment.
  Normative: MUST NOT 1, MUST 1, MAY 1

- `Part XXXI — Deliberately deferred details` [4207-4248]
  Preview: This specification does not freeze: - the generated Rust spelling and module placement of exhaustive catalogue code constants; - the generated spelling of exhaustive code-specific evidence and suggestion variants; - exact auxiliary schema-view types used beneath descriptor accessors; - serialized encodings and serde feature shape; - observer subscription types; - exact async integration helpers; - exact iterator concrete types; - internal sharing mechanism; - exact digest byte width or algorithm; - optional borrowed zero-copy inspection views; - optional public machine-forking API; - feature flags and platform support.
  Symbols: `Problem<D>`, `Diagnostic<D>`, `DiagnosticOccurrence<D>`, `ActiveDiagnosticEpisode<D>`, `InternalDefect<D>`
  Normative: MUST NOT 1, MUST 1, MAY 1

- `Part XXXI — Deliberately deferred details > 113. Deferred exact surfaces` [4209-4229]
  Preview: This specification does not freeze: - the generated Rust spelling and module placement of exhaustive catalogue code constants; - the generated spelling of exhaustive code-specific evidence and suggestion variants; - exact auxiliary schema-view types used beneath descriptor accessors; - serialized encodings and serde feature shape; - observer subscription types; - exact async integration helpers; - exact iterator concrete types; - internal sharing mechanism; - exact digest byte width or algorithm; - optional borrowed zero-copy inspection views; - optional public machine-forking API; - feature flags and platform support.
  Symbols: `Problem<D>`, `Diagnostic<D>`, `DiagnosticOccurrence<D>`, `ActiveDiagnosticEpisode<D>`, `InternalDefect<D>`

- `Part XXXI — Deliberately deferred details > 114. Permitted ergonomic additions` [4230-4248]
  Preview: The implementation MAY add: - convenience aliases; - method chaining; - borrowed metadata setters; - iterators and collection adapters; - `TryFrom` conversions; - display and rendering helpers; - typed wrappers over common inspection results; - macro helpers for explicit stable keys; - optional serde support; - optional bound-machine façades.
  Normative: MUST NOT 1, MUST 1, MAY 1

- `Part XXXII — Required concrete API properties` [4249-4297]
  Preview: The concrete Rust API must preserve:

- `Part XXXII — Required concrete API properties > 115. Required guarantees` [4251-4297]
  Preview: The concrete Rust API must preserve:

- `Summary` [4298-4349]
  Preview: The initial concrete Rust API is deliberately data-oriented and moderately typed.
