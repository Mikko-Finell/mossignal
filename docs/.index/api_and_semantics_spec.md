## docs/specs/api_and_semantics_spec.md
- ``mossignal` API and Semantics Specification` [1-78]
  Preview: **Status:** Design specification, version 2 **Defines:** Public semantics, API responsibilities, diagnostics, inspection, persistence, and reconfiguration **Does not define:** Implementation architecture, performance strategy, or testing policy `mossignal` is a deterministic, host-agnostic Rust library for defining, validating, initializing, executing, inspecting, explaining, persisting, and reconfiguring discrete signal networks.
  Symbols: `mossignal`, `Low`
  Normative: MUST NOT 2, MUST 1, SHOULD NOT 1, SHOULD 1, MAY 1

- ``mossignal` API and Semantics Specification > 1. Purpose` [9-30]
  Preview: `mossignal` is a deterministic, host-agnostic Rust library for defining, validating, initializing, executing, inspecting, explaining, persisting, and reconfiguring discrete signal networks.
  Symbols: `mossignal`

- ``mossignal` API and Semantics Specification > 2. Normative language` [31-36]
  Preview: The terms **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.
  Normative: MUST NOT 1, MUST 1, SHOULD NOT 1, SHOULD 1, MAY 1

- ``mossignal` API and Semantics Specification > 3. Scope and non-goals` [37-78]
  Preview: `mossignal` defines: - signal kinds and their exact semantics; - network definitions; - nodes, ports, connections, and reusable modules; - typed and dynamic authoring APIs; - validation and compilation; - explicit machine initialization; - deterministic synchronous reactions; - caller-driven exact logical time; - stateful and temporal behavior; - atomic transaction semantics; - explicit runtime policy and structured budget failure; - external endpoint bindings; - runtime inspection; - causal explanations and provenance checkpoints; - structured diagnostics and persistent diagnostic episodes; - state-preserving reconfiguration; - graph queries; - scheduling, quiescence, and dormancy; - semantic change sets for observer layers; - snapshots, restoration, replay, and hypothetical execution.
  Symbols: `mossignal`, `Low`
  Normative: MUST NOT 1

- `Part I — Semantic model` [79-304]
  Preview: A network is a directed graph containing: - nodes; - typed input and output ports; - directed connections; - typed external inputs; - typed external outputs; - optional modules and hierarchy; - stable structural keys; - diagnostic metadata.
  Symbols: `mossignal`, `LogicLevel`, `High`, `PulseCount`, `ZERO`, `ONE`, `Level`, `Pulse`, `Schedule<D>`
  Normative: MUST NOT 9, MUST 13, SHOULD 1

- `Part I — Semantic model > 4. Networks and regions` [81-101]
  Preview: A network is a directed graph containing: - nodes; - typed input and output ports; - directed connections; - typed external inputs; - typed external outputs; - optional modules and hierarchy; - stable structural keys; - diagnostic metadata.
  Symbols: `mossignal`
  Normative: MUST NOT 1, MUST 1

- `Part I — Semantic model > 5. Signal kinds` [102-171]
  Preview: The core library defines exactly two signal kinds: These are marker types, not runtime variants of one untyped signal.
  Symbols: `LogicLevel`, `High`, `PulseCount`, `ZERO`, `ONE`
  Normative: MUST NOT 2, MUST 5

- `Part I — Semantic model > 5. Signal kinds > 5.1 Level` [113-127]
  Preview: A level is persistent state: Once established in a ready machine, a level remains at its current value until a defined event changes it.
  Symbols: `LogicLevel`
  Normative: MUST 1

- `Part I — Semantic model > 5. Signal kinds > 5.2 Pulse` [128-156]
  Preview: A pulse is a discrete occurrence at a logical time.
  Symbols: `High`, `PulseCount`, `ZERO`, `ONE`
  Normative: MUST NOT 1, MUST 3

- `Part I — Semantic model > 5. Signal kinds > 5.3 Explicit conversion` [157-171]
  Preview: Conversion between kinds MUST be explicit.
  Normative: MUST NOT 1, MUST 1

- `Part I — Semantic model > 6. Nodes` [172-222]
  Preview: Nodes are classified as combinational, stateful, or temporal, but current-reaction causality is defined per dependency path rather than per whole node.
  Normative: MUST 3

- `Part I — Semantic model > 6. Nodes > 6.1 Combinational nodes` [176-183]
  Preview: A combinational node’s output is determined entirely by its settled current level inputs and current simultaneous pulse batch.
  Normative: MUST 1

- `Part I — Semantic model > 6. Nodes > 6.2 Stateful nodes` [184-203]
  Preview: A stateful node retains semantic state between transactions.
  Normative: MUST 1

- `Part I — Semantic model > 6. Nodes > 6.3 Temporal nodes` [204-222]
  Preview: A temporal node schedules behavior for a caller-supplied future logical time.
  Normative: MUST 1

- `Part I — Semantic model > 7. Ports and connections` [223-250]
  Preview: Canonical port and endpoint keys are: `S` is `Level` or `Pulse`.
  Symbols: `Level`, `Pulse`
  Normative: MUST NOT 2, MUST 2

- `Part I — Semantic model > 8. Current-reaction causality and cycles` [251-287]
  Preview: Each node kind defines a conservative **current-reaction dependency signature** describing which current inputs may affect which current outputs during one reaction.
  Normative: MUST NOT 3, MUST 2, SHOULD 1

- `Part I — Semantic model > 9. Quiescence and dormancy` [288-304]
  Preview: A ready machine is **quiescent** when no immediate propagation work or unpublished current-time change remains.
  Symbols: `Schedule<D>`
  Normative: MUST NOT 1

- `Part II — Canonical object model` [305-456]
  Preview: The canonical representation lifecycle is: Canonical types are: A compiled network may spawn any number of independent machines.
  Symbols: `ValidatedNetwork<D>`, `Machine<D>`, `AwaitingInitialization`, `Ready`, `NetworkBuilder<D>`, `Signal<S>`, `Level`, `Pulse`, `UncheckedNetwork<D>`, `CompiledNetwork<D>`, `Low`, `LogicLevel`
  Normative: MUST NOT 6, MUST 2, SHOULD 1

- `Part II — Canonical object model > 10. Lifecycle` [307-343]
  Preview: The canonical representation lifecycle is: Canonical types are: A compiled network may spawn any number of independent machines.
  Symbols: `AwaitingInitialization`, `Ready`

- `Part II — Canonical object model > 11. `NetworkBuilder<D>`` [344-382]
  Preview: `NetworkBuilder<D>` is the strongly typed Rust authoring API.
  Symbols: `NetworkBuilder<D>`, `Signal<S>`, `Level`, `Pulse`
  Normative: MUST NOT 1, SHOULD 1

- `Part II — Canonical object model > 12. `UncheckedNetwork<D>`` [383-396]
  Preview: `UncheckedNetwork<D>` is the canonical representation for runtime-authored or deserialized definitions.
  Symbols: `UncheckedNetwork<D>`
  Normative: MUST NOT 1, MUST 1

- `Part II — Canonical object model > 13. `ValidatedNetwork<D>`` [397-408]
  Preview: `ValidatedNetwork<D>` satisfies all pre-execution structural and semantic requirements.
  Symbols: `ValidatedNetwork<D>`
  Normative: MUST NOT 1

- `Part II — Canonical object model > 14. `CompiledNetwork<D>`` [409-425]
  Preview: `CompiledNetwork<D>` is an immutable executable specification.
  Symbols: `CompiledNetwork<D>`
  Normative: MUST 1

- `Part II — Canonical object model > 15. `Machine<D>`` [426-456]
  Preview: `Machine<D>` is one mutable execution instance.
  Symbols: `Machine<D>`, `Low`, `LogicLevel`
  Normative: MUST NOT 3

- `Part III — Identity, bindings, and metadata` [457-612]
  Preview: Canonical stable keys are: Structural keys: - are unique within one network identity; - survive compatible revisions when the logical element survives; - are independent of compiled execution positions; - require no global allocator; - carry no semantic meaning through numeric ordering.
  Symbols: `RuntimePolicyId`, `mossignal`, `BoundMachine`, `NetworkRevision`, `ExecutionStateDigest`, `ObservableStateDigest`, `SnapshotDigest`
  Normative: MUST NOT 6, MUST 5, SHOULD 2, MAY 1

- `Part III — Identity, bindings, and metadata > 16. Stable structural keys` [459-484]
  Preview: Canonical stable keys are: Structural keys: - are unique within one network identity; - survive compatible revisions when the logical element survives; - are independent of compiled execution positions; - require no global allocator; - carry no semantic meaning through numeric ordering.
  Normative: MUST 1

- `Part III — Identity, bindings, and metadata > 17. External bindings` [485-517]
  Preview: Bindings associate endpoints with opaque caller-owned keys.
  Symbols: `mossignal`, `BoundMachine`
  Normative: MUST NOT 2, SHOULD 2

- `Part III — Identity, bindings, and metadata > 18. Diagnostic metadata` [518-537]
  Preview: Human-readable identity is separate from structural identity and bindings.
  Normative: MUST NOT 1, MAY 1

- `Part III — Identity, bindings, and metadata > 19. Revision, fingerprint, digests, and runtime policy` [538-594]
  Preview: These concepts are distinct: `NetworkRevision` identifies the topology installed in one machine.
  Symbols: `RuntimePolicyId`, `NetworkRevision`, `ExecutionStateDigest`, `ObservableStateDigest`, `SnapshotDigest`
  Normative: MUST NOT 2, MUST 3

- `Part III — Identity, bindings, and metadata > 19. Revision, fingerprint, digests, and runtime policy > 19.1 Topology revision` [553-562]
  Preview: `NetworkRevision` identifies the topology installed in one machine.
  Symbols: `NetworkRevision`

- `Part III — Identity, bindings, and metadata > 19. Revision, fingerprint, digests, and runtime policy > 19.2 Network fingerprint` [563-568]
  Preview: A fingerprint MUST reflect structural keys, node kinds, typed ports, connections, semantic parameters, state-relevant module structure, and signal-semantics version.
  Normative: MUST 2

- `Part III — Identity, bindings, and metadata > 19. Revision, fingerprint, digests, and runtime policy > 19.3 Digest scopes` [569-578]
  Preview: `ExecutionStateDigest` covers state capable of affecting future execution, including topology fingerprint, lifecycle status, logical time, topology revision, external levels, stateful and temporal state, pending events, and active diagnostic episodes.
  Symbols: `ExecutionStateDigest`, `ObservableStateDigest`, `SnapshotDigest`
  Normative: MUST NOT 1

- `Part III — Identity, bindings, and metadata > 19. Revision, fingerprint, digests, and runtime policy > 19.4 Runtime policy` [579-594]
  Preview: Semantically relevant execution limits are represented by an explicit immutable runtime policy: Examples include limits on internal reactions, evaluated operations, pending events, events created per transaction, and required provenance growth.
  Symbols: `RuntimePolicyId`
  Normative: MUST NOT 1, MUST 1

- `Part III — Identity, bindings, and metadata > 20. Resolved handles` [595-612]
  Preview: Stable keys support persistence and reconfiguration.
  Normative: MUST NOT 1, MUST 1

- `Part IV — Logical time` [613-644]
  Preview: Canonical time types are: `D` is a caller-defined time-domain marker.
  Symbols: `mossignal`, `NonZeroSpan<D>`
  Normative: MUST NOT 2, MUST 5

- `Part IV — Logical time > 21. Caller-owned time` [615-632]
  Preview: Canonical time types are: `D` is a caller-defined time-domain marker.
  Symbols: `mossignal`, `NonZeroSpan<D>`
  Normative: MUST NOT 2, MUST 2

- `Part IV — Logical time > 22. Time progression` [633-644]
  Preview: The first successful transaction establishes an arbitrary initial logical time and initializes the machine.
  Normative: MUST 3

- `Part V — Transactions and execution` [645-777]
  Preview: The atomic execution unit is: A transaction MAY contain: - expected topology revision; - optional expected `ExecutionStateDigest` for state-sensitive freshness; - one prepared topology patch; - one reconfiguration state-loss policy when a patch is present; - exactly one `InputSnapshot` or `InputDelta` according to lifecycle rules; - caller metadata used only for diagnostics and replay.
  Symbols: `T0`, `InputSnapshot`, `InputDelta`, `ExecutionStateDigest`, `CompiledNetwork::spawn`, `RuntimePolicy`
  Normative: MUST NOT 2, MUST 12, MAY 3

- `Part V — Transactions and execution > 23. Explicit transaction values` [647-684]
  Preview: The atomic execution unit is: A transaction MAY contain: - expected topology revision; - optional expected `ExecutionStateDigest` for state-sensitive freshness; - one prepared topology patch; - one reconfiguration state-loss policy when a patch is present; - exactly one `InputSnapshot` or `InputDelta` according to lifecycle rules; - caller metadata used only for diagnostics and replay.
  Symbols: `InputSnapshot`, `InputDelta`, `ExecutionStateDigest`
  Normative: MUST 3, MAY 2

- `Part V — Transactions and execution > 24. Transaction ordering` [685-725]
  Preview: For initialization at time `T0`, the machine MUST: 1.
  Symbols: `T0`
  Normative: MUST NOT 1, MUST 2

- `Part V — Transactions and execution > 24. Transaction ordering > 24.1 First transaction` [687-702]
  Preview: For initialization at time `T0`, the machine MUST: 1.
  Symbols: `T0`
  Normative: MUST 1

- `Part V — Transactions and execution > 24. Transaction ordering > 24.2 Ready-machine transaction` [703-725]
  Preview: For a transaction at time `T`, the ready machine MUST: 1.
  Normative: MUST NOT 1, MUST 1

- `Part V — Transactions and execution > 25. Atomic input semantics` [726-733]
  Preview: All external input at one time is one unordered semantic batch.
  Normative: MUST 2

- `Part V — Transactions and execution > 26. Settlement` [734-743]
  Preview: Every successful reaction MUST settle all immediate consequences before it can contribute to a committed outer transaction.
  Normative: MUST NOT 1, MUST 2

- `Part V — Transactions and execution > 27. Canonical machine API` [744-777]
  Preview: `CompiledNetwork::spawn` MUST associate the machine with an explicit validated `RuntimePolicy`.
  Symbols: `CompiledNetwork::spawn`, `RuntimePolicy`
  Normative: MUST 3, MAY 1

- `Part VI — Inputs and outputs` [778-930]
  Preview: `InputSnapshot` is the complete authoritative state of all external level inputs at one time, plus pulse occurrences at that time.
  Symbols: `InputSnapshot`, `Low`, `InputDelta`, `LevelEstablished`, `AwaitingInitialization`, `processed_times`, `LevelChanged`, `StateChange<D>`
  Normative: MUST NOT 3, MUST 6, SHOULD 4, MAY 1

- `Part VI — Inputs and outputs > 28. `InputSnapshot`` [780-805]
  Preview: `InputSnapshot` is the complete authoritative state of all external level inputs at one time, plus pulse occurrences at that time.
  Symbols: `InputSnapshot`, `Low`
  Normative: MUST 2

- `Part VI — Inputs and outputs > 29. `InputDelta`` [806-819]
  Preview: `InputDelta` expresses changes relative to the external level valuation already held by a ready machine.
  Symbols: `InputDelta`, `AwaitingInitialization`
  Normative: MUST 2

- `Part VI — Inputs and outputs > 30. Input projection` [820-832]
  Preview: The library SHOULD support prevalidated projection from caller-owned binding keys: Projection MUST diagnose missing, duplicate, unknown, ambiguous, wrong-kind, wrong-network, and stale-revision observations.
  Normative: MUST NOT 1, MUST 1, SHOULD 1

- `Part VI — Inputs and outputs > 31. `TransactionResult<D>` and semantic change sets` [833-871]
  Preview: Every successful transaction produces one immutable semantic change set regardless of whether any observer or subscription exists.
  Symbols: `processed_times`
  Normative: SHOULD 1, MAY 1

- `Part VI — Inputs and outputs > 32. `OutputEvent<D>`` [872-912]
  Preview: Before initialization, a level output has no observable baseline.
  Symbols: `LevelEstablished`, `Low`, `LevelChanged`
  Normative: MUST NOT 1, MUST 1

- `Part VI — Inputs and outputs > 33. State-change reporting` [913-930]
  Preview: `StateChange<D>` SHOULD cover: - node internal state; - level-port establishment and change; - pulse activity for each completed reaction; - pending-event creation, migration, transformation, cancellation, and firing; - next-deadline changes; - active diagnostic-episode changes; - provenance-root changes; - module aggregate state; - region changes where relevant.
  Symbols: `StateChange<D>`
  Normative: MUST NOT 1, SHOULD 2

- `Part VII — Stateful and temporal semantics` [931-998]
  Preview: The core MUST avoid vaguely specified generic memory nodes.
  Normative: MUST 10, SHOULD 2

- `Part VII — Stateful and temporal semantics > 34. Explicit memory primitives` [933-944]
  Preview: The core MUST avoid vaguely specified generic memory nodes.
  Normative: MUST 3, SHOULD 1

- `Part VII — Stateful and temporal semantics > 35. Edge detection` [945-956]
  Preview: Repeating the same level emits nothing.
  Normative: MUST 1

- `Part VII — Stateful and temporal semantics > 36. Pulse multiplicity` [957-962]
  Preview: Every pulse-consuming node MUST define whether it preserves, coalesces, consumes, bounds, or transforms multiplicity.
  Normative: MUST 1

- `Part VII — Stateful and temporal semantics > 37. Pulse delay` [963-968]
  Preview: A pulse delay schedules every occurrence for a future time.
  Normative: MUST 1

- `Part VII — Stateful and temporal semantics > 38. Level delay` [969-977]
  Preview: The library MUST distinguish: - **transport delay**, which reproduces every transition after a duration; - **inertial delay**, which reproduces a transition only if the new level remains stable for the duration.
  Normative: MUST 2

- `Part VII — Stateful and temporal semantics > 39. Periodic behavior` [978-990]
  Preview: Periodic nodes MUST define period, enabling condition, first-emission phase, disable behavior, re-enable behavior, phase preservation, large-jump behavior, and multiplicity when several periods elapse.
  Normative: MUST 1, SHOULD 1

- `Part VII — Stateful and temporal semantics > 40. Large time jumps` [991-998]
  Preview: Advancing directly from `A` to `B` MUST be observationally equivalent to processing every intervening deadline chronologically.
  Normative: MUST 1

- `Part VIII — Diagnostics` [999-1120]
  Preview: Diagnostics MUST be first-class structured data: Rendered text is not authoritative.
  Normative: MUST NOT 3, MUST 9, SHOULD 4, MAY 2

- `Part VIII — Diagnostics > 41. Structured diagnostics` [1001-1026]
  Preview: Diagnostics MUST be first-class structured data: Rendered text is not authoritative.
  Normative: MUST 1

- `Part VIII — Diagnostics > 42. Stable codes` [1027-1032]
  Preview: Every category MUST have a stable code suitable for documentation, filtering, automated handling, regression tracking, editor integration, and support reports.
  Normative: MUST 2

- `Part VIII — Diagnostics > 43. Subjects and evidence` [1033-1038]
  Preview: A subject MUST be able to identify a network, region, module, node, port, connection, endpoint, binding, pending event, snapshot, transaction, revision, or resolved handle.
  Normative: MUST 1, MAY 1

- `Part VIII — Diagnostics > 44. Suggestions` [1039-1044]
  Preview: Machine-readable suggestions MAY be included only when a correction is unambiguous.
  Normative: MUST NOT 1, MAY 1

- `Part VIII — Diagnostics > 45. Reports` [1045-1061]
  Preview: Validation, compilation, binding, and structural patch preparation SHOULD use: Independent findings SHOULD be collected where safe.
  Normative: MUST 2, SHOULD 2

- `Part VIII — Diagnostics > 46. Required validation coverage` [1062-1086]
  Preview: Validation MUST cover: - unknown or duplicate keys; - missing nodes and ports; - invalid direction; - signal-kind mismatch; - unsupported multiple drivers; - missing required inputs; - invalid arity; - invalid module interface conformance; - cycles in the current-reaction dependency graph; - invalid or incomplete dependency signatures; - invalid timing parameters; - invalid initial state; - duplicate or ambiguous bindings; - malformed hierarchy; - incompatible network references; - incompatible state schema.
  Normative: MUST 1, SHOULD 2

- `Part VIII — Diagnostics > 47. Lifecycle-wide quality` [1087-1120]
  Preview: The same diagnostic model MUST apply to authoring, validation, compilation, binding, input projection, initialization, execution, inspection, explanation, snapshots, restoration, replay, patch preparation, patch finalization, stale handles, forecasts, runtime-policy failures, and observer resynchronization.
  Normative: MUST NOT 2, MUST 2

- `Part VIII — Diagnostics > 47. Lifecycle-wide quality > 47.1 Persistent diagnostic episodes` [1093-1120]
  Preview: A persistent diagnostic condition has stable semantic identity derived from facts such as: Conceptually an episode is: A diagnostic event is emitted when an episode begins, materially changes, and optionally when it resolves.
  Normative: MUST NOT 1, MUST 1

- `Part IX — Inspection and explanation` [1121-1345]
  Preview: Every semantically relevant element MUST be inspectable.
  Symbols: `Machine`, `MissingPulse { since }`
  Normative: MUST NOT 5, MUST 10, SHOULD 6, MAY 5

- `Part IX — Inspection and explanation > 48. Inspection is core` [1123-1132]
  Preview: Every semantically relevant element MUST be inspectable.
  Normative: MUST 3

- `Part IX — Inspection and explanation > 49. Direct inspection` [1133-1155]
  Preview: Canonical methods SHOULD include:
  Normative: SHOULD 1

- `Part IX — Inspection and explanation > 50. Node inspection` [1156-1177]
  Preview: A ready-machine node inspection MUST expose, where applicable: - key and kind; - current level input and output values; - pulse activity in the most recent retained reaction, explicitly labeled as history; - internal and declared initial state; - last committed state transition; - pending events; - next deadline; - current causal justification; - active persistent diagnostic conditions; - provenance checkpoint boundary; - metadata; - containing module and region; - topology revision and logical time.
  Normative: MUST 2, SHOULD 1, MAY 1

- `Part IX — Inspection and explanation > 51. Stable inspection queries and compiled plans` [1178-1202]
  Preview: The library SHOULD distinguish stable inspection intent from revision-bound dense access: A stable query MAY survive topology revisions and be recompiled.
  Normative: MUST NOT 1, MUST 1, SHOULD 1, MAY 1

- `Part IX — Inspection and explanation > 52. Inspection subscriptions and observer state` [1203-1231]
  Preview: Subscriptions and their delivery state live outside semantic `Machine` state.
  Symbols: `Machine`
  Normative: MUST NOT 2, MUST 1, MAY 1

- `Part IX — Inspection and explanation > 53. Causal provenance` [1232-1277]
  Preview: Every externally observable establishment or change MUST have a cause.
  Normative: MUST NOT 1, MUST 2, SHOULD 2, MAY 2

- `Part IX — Inspection and explanation > 53. Causal provenance > 53.1 Authoritative roots` [1256-1269]
  Preview: An explanation may terminate at an authoritative root such as: - declared initial state; - external input observation; - committed caller transaction; - retained temporal origin; - topology migration or reset; - snapshot checkpoint; - explicit provenance checkpoint.

- `Part IX — Inspection and explanation > 53. Causal provenance > 53.2 Provenance checkpoints` [1270-1277]
  Preview: Older ancestry MAY be compacted into an explicit checkpoint establishing semantic facts at a logical time and topology revision.
  Normative: MUST NOT 1, SHOULD 1, MAY 1

- `Part IX — Inspection and explanation > 54. Explanation API` [1278-1302]

- `Part IX — Inspection and explanation > 55. Explanation result` [1303-1345]
  Preview: Explanation data is structured and independent from prose rendering.
  Symbols: `MissingPulse { since }`
  Normative: MUST NOT 1, MUST 1, SHOULD 1

- `Part X — Reconfiguration` [1346-1503]
  Preview: Topology changes are explicit values: A patch may add or remove nodes, connections, modules, and endpoints; change semantic parameters; and modify hierarchy or metadata.
  Symbols: `Preserve`, `Migrate`, `Reset`, `Reject`, `forecast`, `ExecutionStateDigest`, `RuntimePolicyId`, `RejectStateLoss`, `AllowReportedStateLoss`, `LevelEstablished`
  Normative: MUST NOT 3, MUST 6, SHOULD 1

- `Part X — Reconfiguration > 56. `NetworkPatch<D>`` [1348-1359]
  Preview: Topology changes are explicit values: A patch may add or remove nodes, connections, modules, and endpoints; change semantic parameters; and modify hierarchy or metadata.
  Normative: MUST 1

- `Part X — Reconfiguration > 57. Structural patch preparation` [1360-1398]
  Preview: Patch handling is divided into reusable structural preparation and state-dependent transaction-time finalization.
  Symbols: `Preserve`, `Migrate`, `Reset`, `Reject`
  Normative: MUST NOT 1, MUST 1, SHOULD 1

- `Part X — Reconfiguration > 58. Prepared-patch freshness and exact preview` [1399-1417]
  Preview: A prepared patch is bound to its exact base topology revision and semantic definition.
  Symbols: `forecast`, `ExecutionStateDigest`, `RuntimePolicyId`

- `Part X — Reconfiguration > 59. State compatibility` [1418-1434]
  Preview: Every stateful and temporal kind MUST define compatibility across revisions.
  Normative: MUST 2

- `Part X — Reconfiguration > 60. Preservation rules` [1435-1446]
  Preview: When a stable node key survives and definitions are compatible, state MUST be preserved.
  Normative: MUST NOT 1, MUST 1

- `Part X — Reconfiguration > 61. Pending-event migration` [1447-1464]
  Preview: Every temporal kind MUST define pending-event behavior when the node is removed, duration changes, connections change, semantic kind changes, compatible identity survives, or containing module changes.
  Normative: MUST 1

- `Part X — Reconfiguration > 62. State-loss policy` [1465-1479]
  Preview: `RejectStateLoss` prevents commitment if the finalized semantic loss set is non-empty.
  Symbols: `RejectStateLoss`, `AllowReportedStateLoss`

- `Part X — Reconfiguration > 63. Atomic commitment` [1480-1503]
  Preview: A prepared patch commits only through a transaction: The transaction finalizes actual state and event migration against the state reached at the patch time, installs the candidate topology, settles topology-induced reaction changes, and publishes one atomic result.
  Symbols: `LevelEstablished`
  Normative: MUST NOT 1

- `Part XI — Modules and graph queries` [1504-1554]
  Preview: A module has typed public inputs and outputs, private internal structure, parameters, metadata, a module fingerprint, and compatibility rules.
  Normative: MUST NOT 1, MUST 2, SHOULD 3

- `Part XI — Modules and graph queries > 64. Modules` [1506-1524]
  Preview: A module has typed public inputs and outputs, private internal structure, parameters, metadata, a module fingerprint, and compatibility rules.
  Normative: MUST NOT 1, MUST 2, SHOULD 1

- `Part XI — Modules and graph queries > 65. Read-only graph view` [1525-1534]
  Preview: It preserves keys and hierarchy and supports visualization, diagnostics, navigation, and dependency analysis.

- `Part XI — Modules and graph queries > 66. Region and dependency queries` [1535-1554]
  Preview: Canonical queries SHOULD include: The graph API SHOULD answer which inputs can affect an output, which outputs an input can affect, whether an output is reachable, and which stateful, temporal, and module elements lie on relevant paths.
  Normative: SHOULD 2

- `Part XII — Scheduling, forecasting, persistence, and replay` [1555-1686]
  Preview: Every successful transaction on a ready machine exposes `Schedule::Dormant` or `Schedule::WakeAt(time)`.
  Symbols: `apply`, `ExecutionStateDigest`, `RuntimePolicyId`, `Schedule::Dormant`, `Schedule::WakeAt(time)`, `WakeAt`, `next_deadline()`, `schedule()`, `Dormant`, `ObservableStateDigest`, `SnapshotDigest`, `Low`, `forecast`
  Normative: MUST NOT 3, MUST 10

- `Part XII — Scheduling, forecasting, persistence, and replay > 67. Scheduling` [1557-1568]
  Preview: Every successful transaction on a ready machine exposes `Schedule::Dormant` or `Schedule::WakeAt(time)`.
  Symbols: `Schedule::Dormant`, `Schedule::WakeAt(time)`, `WakeAt`, `next_deadline()`, `schedule()`, `Dormant`
  Normative: MUST NOT 1, MUST 1

- `Part XII — Scheduling, forecasting, persistence, and replay > 68. Forecasting` [1569-1594]
  Preview: A forecast executes the same deterministic transition function as `apply` on unpublished candidate state.
  Symbols: `apply`, `ExecutionStateDigest`, `RuntimePolicyId`
  Normative: MUST 1

- `Part XII — Scheduling, forecasting, persistence, and replay > 69. Snapshots` [1595-1624]
  Preview: A snapshot MUST contain the complete semantic state sufficient to determine future behavior, including: - lifecycle status; - logical time when ready; - topology revision; - network fingerprint; - semantic and snapshot-schema versions; - authoritative external levels when ready; - current settled level values when ready; - stateful-node state; - temporal-node state; - pending event calendar; - external level-output baselines and establishment state; - required causal roots or provenance checkpoints; - active persistent diagnostic episodes; - `ExecutionStateDigest`; - `ObservableStateDigest`; - `RuntimePolicyId` where exact operational restoration requires it; - an attached `SnapshotDigest` computed over the canonical snapshot payload, excluding the digest field itself.
  Symbols: `ExecutionStateDigest`, `ObservableStateDigest`, `RuntimePolicyId`, `SnapshotDigest`
  Normative: MUST NOT 1, MUST 2

- `Part XII — Scheduling, forecasting, persistence, and replay > 70. Restoration` [1625-1650]
  Preview: Restoration is validation, not deserialization alone.
  Symbols: `Low`
  Normative: MUST NOT 1, MUST 2

- `Part XII — Scheduling, forecasting, persistence, and replay > 71. Replay` [1651-1686]
  Preview: A replay frame contains enough information to reproduce execution without hidden caller state.
  Symbols: `apply`, `forecast`
  Normative: MUST 4

- `Part XIII — Extensibility boundaries` [1687-1739]
  Preview: The core MUST NOT expose unrestricted callback-based custom nodes.
  Symbols: `Level`, `Pulse`
  Normative: MUST NOT 2, MUST 1, SHOULD 1

- `Part XIII — Extensibility boundaries > 72. Closed evaluator boundary` [1689-1702]
  Preview: The core MUST NOT expose unrestricted callback-based custom nodes.
  Normative: MUST NOT 1

- `Part XIII — Extensibility boundaries > 73. No arbitrary payloads` [1703-1708]
  Preview: Signals carry only `Level` or `Pulse`.
  Symbols: `Level`, `Pulse`

- `Part XIII — Extensibility boundaries > 74. Composition before primitives` [1709-1714]
  Preview: New behavior SHOULD first be expressed through existing primitives and modules.
  Normative: SHOULD 1

- `Part XIII — Extensibility boundaries > 75. Requirements for new node kinds` [1715-1739]
  Preview: Every new node kind MUST define: - typed ports; - semantic parameters; - declared initial state; - current-reaction dependency signature; - current output law over previous state, settled current inputs, and due obligations; - proposed successor-state law; - simultaneous-input semantics; - pulse multiplicity semantics; - exact-deadline semantics where temporal; - strictly-future scheduling behavior; - inspection schema; - current-support and why-not explanation semantics; - transition and migration causality; - snapshot schema; - state compatibility; - pending-event migration; - persistent diagnostic-condition behavior; - diagnostics.
  Normative: MUST NOT 1, MUST 1

- `Part XIV — End-to-end API example` [1740-1848]
  Preview: Bindings remain separate: The evaluator does not inspect the external key types.

- `Part XV — Required quality properties` [1849-1926]
  Preview: Equivalent network, lifecycle state, logical times, topology transactions, external input transactions, runtime policy, and semantic versions MUST produce equivalent observable results.
  Symbols: `LevelEstablished`
  Normative: MUST NOT 1, MUST 3, SHOULD 1

- `Part XV — Required quality properties > 76. Determinism` [1851-1856]
  Preview: Equivalent network, lifecycle state, logical times, topology transactions, external input transactions, runtime policy, and semantic versions MUST produce equivalent observable results.
  Normative: MUST 2

- `Part XV — Required quality properties > 77. Strong typing` [1857-1864]
  Preview: The typed API MUST encode signal kind, endpoint direction, and time domain wherever statically knowable.
  Normative: MUST 1

- `Part XV — Required quality properties > 78. Precise time and initialization semantics` [1865-1872]
  Preview: All temporal behavior is expressed in exact discrete caller-owned logical time.
  Symbols: `LevelEstablished`

- `Part XV — Required quality properties > 79. State preservation` [1873-1880]
  Preview: Compatible state survives topology revisions.

- `Part XV — Required quality properties > 80. Complete inspectability` [1881-1886]
  Preview: Every stateful and temporal behavior is inspectable.

- `Part XV — Required quality properties > 81. Causal explainability` [1887-1892]
  Preview: The library explains current output and stored state through structured causal information.
  Normative: SHOULD 1

- `Part XV — Required quality properties > 82. Diagnostic quality` [1893-1898]
  Preview: Diagnostics are structured, stable, deterministic, attached to precise subjects, usable by non-textual tooling, and available throughout the lifecycle.

- `Part XV — Required quality properties > 83. Transactional consistency` [1899-1904]
  Preview: Initialization, chronological deadline advancement, topology finalization, input update, state migration, reaction evaluation, provenance, diagnostic episodes, digest computation, and publication follow one atomic transaction model.

- `Part XV — Required quality properties > 84. Domain neutrality` [1905-1910]
  Preview: The evaluator knows nothing about caller objects, actions, or vocabulary.

- `Part XV — Required quality properties > 85. Replayability` [1911-1916]
  Preview: A compatible snapshot, runtime policy, and transaction history reproduce execution exactly.

- `Part XV — Required quality properties > 86. No hidden global state` [1917-1920]
  Preview: Identity allocation, time, scheduling, and execution do not depend on global mutable state.

- `Part XV — Required quality properties > 87. API coherence` [1921-1926]
  Preview: Initialization, execution, inspection, explanation, reconfiguration, persistence, graph queries, diagnostics, and observer change sets describe one shared semantic model.
  Normative: MUST NOT 1

- `Part XVI — Deferred specifications` [1927-1949]
  Preview: The following are intentionally deferred: - testing and verification policy; - exhaustive and property-based test requirements; - reference evaluator requirements; - fuzzing policy; - model checking; - declarative temporal contracts; - constrained user-defined automata; - editor-specific interaction design; - visualization formats; - implementation architecture; - performance targets; - memory layout; - parallel execution; - platform and feature-flag policy.

- `Summary` [1950-2000]
  Preview: `mossignal` is an inspectable, causally explainable, state-preserving signal machine with an explicit lifecycle.
  Symbols: `mossignal`
