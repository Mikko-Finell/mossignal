## docs/specs/processor_and_runtime_architecture.md
- ``mossignal` Processor and Runtime Architecture` [1-65]
  Preview: **Status:** Consolidated design specification, version 2 **Defines:** Processor architecture, compilation, runtime state, reaction evaluation, temporal execution, transactions, reconfiguration, provenance, inspection, persistence, replay, and implementation boundaries **Does not define:** The complete public API surface, individual built-in node semantics, serialized wire formats, testing policy, application integration, model checking, or performance targets This specification defines the intended internal architecture of the `mossignal` processor.
  Symbols: `mossignal`

- ``mossignal` Processor and Runtime Architecture > 1. Purpose` [9-36]
  Preview: This specification defines the intended internal architecture of the `mossignal` processor.
  Symbols: `mossignal`

- ``mossignal` Processor and Runtime Architecture > 2. Theory-grounded engineering` [37-65]
  Preview: Where the processor implements a recognized mathematical structure, the design, implementation, and verification should name that structure and use its established results directly.

- `Part I — Representation and ownership` [66-244]
  Preview: The canonical representation lifecycle is: `NetworkBuilder` and `UncheckedNetwork` represent authored structure: * stable structural keys; * nodes and typed ports; * directed connections; * external endpoints; * modules and hierarchy; * semantic parameters; * initial-state declarations; * diagnostic metadata.
  Symbols: `NetworkBuilder`, `UncheckedNetwork`, `ValidatedNetwork`, `CompiledNetwork`, `Machine`

- `Part I — Representation and ownership > 3. Lifecycle` [68-157]
  Preview: The canonical representation lifecycle is: `NetworkBuilder` and `UncheckedNetwork` represent authored structure: * stable structural keys; * nodes and typed ports; * directed connections; * external endpoints; * modules and hierarchy; * semantic parameters; * initial-state declarations; * diagnostic metadata.
  Symbols: `NetworkBuilder`, `UncheckedNetwork`, `ValidatedNetwork`, `CompiledNetwork`, `Machine`

- `Part I — Representation and ownership > 3. Lifecycle > 3.1 Authored definitions` [82-96]
  Preview: `NetworkBuilder` and `UncheckedNetwork` represent authored structure: * stable structural keys; * nodes and typed ports; * directed connections; * external endpoints; * modules and hierarchy; * semantic parameters; * initial-state declarations; * diagnostic metadata.
  Symbols: `NetworkBuilder`, `UncheckedNetwork`

- `Part I — Representation and ownership > 3. Lifecycle > 3.2 Validated definitions` [97-116]
  Preview: `ValidatedNetwork` establishes that every static structural and semantic requirement holds.
  Symbols: `ValidatedNetwork`

- `Part I — Representation and ownership > 3. Lifecycle > 3.3 Compiled topology` [117-133]
  Preview: `CompiledNetwork` is an immutable executable program.
  Symbols: `CompiledNetwork`

- `Part I — Representation and ownership > 3. Lifecycle > 3.4 Running machine` [134-157]
  Preview: `Machine` pairs one compiled topology with mutable semantic execution state.
  Symbols: `Machine`

- `Part I — Representation and ownership > 4. Immutable program and mutable store` [158-207]
  Preview: The processor follows the conceptual model: [ Machine = (CompiledProgram,\ MutableStore) ] The compiled program owns immutable facts such as: The mutable store owns: This separation ensures that: * two machines spawned from one compiled network cannot share semantic state accidentally; * runtime mutation cannot corrupt compiled graph invariants; * fingerprints do not depend on execution history; * forecasts and transaction staging have clear ownership boundaries; * topology can be replaced atomically during reconfiguration.

- `Part I — Representation and ownership > 5. Stable keys and dense indices` [208-244]
  Preview: Stable keys and dense runtime indices serve different purposes.

- `Part II — Initialization` [245-408]
  Preview: A newly spawned machine is explicitly uninitialized.
  Symbols: `Low`, `High`, `Level`, `InputSnapshot`, `InputDelta`, `Immediate`, `AfterFirstPeriod`, `PreservePhase`, `Dormant`

- `Part II — Initialization > 6. Machine lifecycle states` [247-272]
  Preview: A newly spawned machine is explicitly uninitialized.
  Symbols: `Low`, `Level`

- `Part II — Initialization > 7. First transaction` [273-305]
  Preview: The first successful transaction initializes the machine.
  Symbols: `InputSnapshot`, `InputDelta`

- `Part II — Initialization > 8. Initial state behavior` [306-313]
  Preview: Declared initial state acts as the previous stored state supplied to the first reaction.
  Symbols: `High`, `Low`

- `Part II — Initialization > 9. Edge-detector initialization` [314-332]
  Preview: Edge detectors have explicit initialization policy.

- `Part II — Initialization > 9. Edge-detector initialization > 9.1 Baseline` [318-326]
  Preview: The previous observation begins unestablished.

- `Part II — Initialization > 9. Edge-detector initialization > 9.2 Assume` [327-332]
  Preview: The configured initial level acts as the previous observation.

- `Part II — Initialization > 10. Output establishment` [333-367]
  Preview: A level output has no prior observable value before initialization.
  Symbols: `Low`

- `Part II — Initialization > 11. Periodic initialization` [368-383]
  Preview: A newly spawned periodic node has: The first settled `High` enable establishes the initial phase anchor at (T_0).
  Symbols: `High`, `Immediate`, `AfterFirstPeriod`, `PreservePhase`

- `Part II — Initialization > 12. Pre-initialization inspection` [384-408]
  Preview: Before initialization, structural inspection is available: * topology; * node definitions; * declared initial state; * metadata; * graph structure.
  Symbols: `Dormant`

- `Part III — Graph model and causality` [409-586]
  Preview: The authored network forms a directed typed structural graph containing: * nodes; * ports; * connections; * external endpoints; * modules; * stable keys.
  Symbols: `enable`, `Select`

- `Part III — Graph model and causality > 13. Structural graph` [411-434]
  Preview: The authored network forms a directed typed structural graph containing: * nodes; * ports; * connections; * external endpoints; * modules; * stable keys.

- `Part III — Graph model and causality > 14. Reaction equations` [435-465]
  Preview: Each built-in node is interpreted as a deterministic synchronous transducer.

- `Part III — Graph model and causality > 15. Reaction dependency graph` [466-489]
  Preview: Compilation derives a current-reaction dependency graph (G_R).

- `Part III — Graph model and causality > 16. Dependency-specific causality barriers` [490-516]
  Preview: A whole node is not inherently a causality barrier.
  Symbols: `enable`

- `Part III — Graph model and causality > 17. Static dependency signatures` [517-528]
  Preview: Each built-in node kind must define a conservative current-reaction dependency signature.
  Symbols: `Select`

- `Part III — Graph model and causality > 18. Strongly connected components` [529-548]
  Preview: Current-reaction cycle detection should use strongly connected component decomposition.

- `Part III — Graph model and causality > 19. Topological order` [549-564]
  Preview: Because (G_R) is acyclic, it admits a topological ordering.

- `Part III — Graph model and causality > 20. Regions` [565-586]
  Preview: Weakly connected regions are the connected components of the structural graph after edge direction is ignored.

- `Part IV — Compilation` [587-650]
  Preview: Compilation transforms a validated network into an immutable executable representation.

- `Part IV — Compilation > 21. Compilation responsibilities` [589-608]
  Preview: Compilation transforms a validated network into an immutable executable representation.

- `Part IV — Compilation > 22. Compiled invariants` [609-625]
  Preview: A compiled network must establish: * every dense reference is in bounds; * every node descriptor matches its node kind; * every port has the correct signal kind; * every connection satisfies driver rules; * every reaction dependency advances in topological order; * the reaction graph is acyclic; * every state slot belongs to the correct state family; * region membership partitions structural subjects; * stable-key lookup is unambiguous; * endpoint tables are complete and type-correct.

- `Part IV — Compilation > 23. Semantic fingerprint` [626-650]
  Preview: The compiled fingerprint is derived from canonical semantic structure, including: * stable structural keys; * node kinds; * typed ports; * connections; * semantic parameters; * state-relevant module structure; * signal-semantics version.

- `Part V — Runtime storage` [651-786]
  Preview: Runtime state should be partitioned by semantic family rather than stored as one heap-allocated trait object per node.
  Symbols: `CauseRef`

- `Part V — Runtime storage > 24. Storage by semantic family` [653-685]
  Preview: Runtime state should be partitioned by semantic family rather than stored as one heap-allocated trait object per node.

- `Part V — Runtime storage > 25. Persistent levels` [686-691]
  Preview: Level ports have persistent current values.

- `Part V — Runtime storage > 26. Reaction-scoped pulses` [692-715]
  Preview: Pulse values exist only within one logical reaction.

- `Part V — Runtime storage > 27. Previous and proposed state` [716-739]
  Preview: Stateful evaluation must distinguish: Every state transition reads the same previous-state vector.

- `Part V — Runtime storage > 28. Adjacency` [740-753]
  Preview: Compiled reaction adjacency should use compact immutable storage.

- `Part V — Runtime storage > 29. Event records and deadline index` [754-774]
  Preview: Temporal storage has two separate responsibilities: 1.

- `Part V — Runtime storage > 30. Provenance records` [775-786]
  Preview: Committed provenance records are immutable.
  Symbols: `CauseRef`

- `Part VI — Reaction evaluation` [787-894]
  Preview: A logical reaction receives: * previous committed machine state; * one complete same-time external stimulus batch; * all surviving temporal obligations due now; * topology and migration facts effective now.

- `Part VI — Reaction evaluation > 31. Reaction inputs and outputs` [789-809]
  Preview: A logical reaction receives: * previous committed machine state; * one complete same-time external stimulus batch; * all surviving temporal obligations due now; * topology and migration facts effective now.

- `Part VI — Reaction evaluation > 32. Glitch freedom` [810-824]
  Preview: Only settled pre-reaction and post-reaction values are semantically observable.

- `Part VI — Reaction evaluation > 33. Full reference evaluation` [825-843]
  Preview: The reference reaction evaluator processes every reaction operation once in a valid topological order.

- `Part VI — Reaction evaluation > 34. Incremental dirty propagation` [844-858]
  Preview: A correct incremental evaluator should: 1.

- `Part VI — Reaction evaluation > 35. Recommended worklist` [859-870]
  Preview: The initial implementation should use: * dense topological indices; * generation-stamped dirty state; * an ordered worklist.

- `Part VI — Reaction evaluation > 36. Same-time chains through stateful nodes` [871-894]
  Preview: Current outputs from stateful nodes may affect downstream stateful nodes during the same reaction.

- `Part VII — Logical time and temporal execution` [895-1047]
  Preview: Logical time is exact and discrete.
  Symbols: `mossignal`, `enable = Low`, `InertialDelay`

- `Part VII — Logical time and temporal execution > 37. Discrete exact time` [897-914]
  Preview: Logical time is exact and discrete.
  Symbols: `mossignal`

- `Part VII — Logical time and temporal execution > 38. Time arithmetic` [915-930]
  Preview: Time supports checked operations such as: Overflow and invalid subtraction produce structured failure.

- `Part VII — Logical time and temporal execution > 39. Event calendar` [931-944]
  Preview: The semantic event calendar is: [ C : Time \rightharpoonup FiniteMultiset(PendingEvent) ] Each deadline maps to one complete unordered finite batch.

- `Part VII — Logical time and temporal execution > 40. Strictly future scheduling` [945-956]
  Preview: Every newly pending event must satisfy: [ deadline > current\ reaction\ time ] Immediate behavior is produced directly in the current reaction rather than inserted into the calendar at the current time.

- `Part VII — Logical time and temporal execution > 41. Advancing time` [957-970]
  Preview: For a transaction at time (T), the processor must: 1.

- `Part VII — Logical time and temporal execution > 42. Large-time-jump equivalence` [971-988]
  Preview: If intervening deadlines are: [ d_1 < d_2 < \dots < d_k < T ] then direct advancement must be observationally equivalent to: [ Step(d_k)\circ\dots\circ Step(d_1) ] followed by the reaction at (T).

- `Part VII — Logical time and temporal execution > 43. Exact-deadline obligations` [989-1001]
  Preview: A pending temporal obligation due at (T) is evaluated from temporal state entering the reaction.
  Symbols: `enable = Low`

- `Part VII — Logical time and temporal execution > 44. Inertial deadline boundary` [1002-1019]
  Preview: An inertial candidate created at (S) with deadline (D) succeeds if its target remains the input throughout: [ [S,D) ] A contradictory input change strictly before (D) cancels it.
  Symbols: `InertialDelay`

- `Part VII — Logical time and temporal execution > 45. Cancellation` [1020-1037]
  Preview: Cancellation removes an event from the semantic calendar.

- `Part VII — Logical time and temporal execution > 46. Event aggregation` [1038-1047]
  Preview: Pending events may be aggregated only where an established associative and commutative law preserves observable behavior.

- `Part VIII — Transactions and atomicity` [1048-1174]
  Preview: Machine execution is a deterministic partial transition: [ \delta(M,\tau)= \begin{cases} (M',R) & \text{on success}\ Failure & \text{on rejection} \end{cases} ] Failure atomicity requires: > If execution fails, the published semantic machine remains exactly (M).

- `Part VIII — Transactions and atomicity > 47. State-transition model` [1050-1079]
  Preview: Machine execution is a deterministic partial transition: [ \delta(M,\tau)= \begin{cases} (M',R) & \text{on success}\ Failure & \text{on rejection} \end{cases} ] Failure atomicity requires: > If execution fails, the published semantic machine remains exactly (M).

- `Part VIII — Transactions and atomicity > 48. Single commit point` [1080-1087]
  Preview: A transaction has one conceptual publication point.

- `Part VIII — Transactions and atomicity > 49. Preparation and commitment` [1088-1112]
  Preview: Preparation includes all operations that may fail: * revision validation; * input validation; * earlier-deadline execution; * topology compilation; * migration finalization; * reaction evaluation; * state staging; * event scheduling; * provenance construction; * diagnostic-episode updates; * result construction; * digest calculation; * resource-budget checks.

- `Part VIII — Transactions and atomicity > 50. Reference execution strategy` [1113-1131]
  Preview: Clone-and-swap is the reference atomic implementation: The production runtime may use: * sparse overlays; * copy-on-write; * private arena segments; * prepared replacement structures.

- `Part VIII — Transactions and atomicity > 51. Pending-event staging` [1132-1144]
  Preview: The candidate calendar is semantically: [ C' = (C \setminus Fired \setminus Cancelled \setminus MigratedOut) \cup Added \cup MigratedIn ] The implementation may realize this through overlays or replacement roots, provided all transaction-local reads observe the candidate semantics.

- `Part VIII — Transactions and atomicity > 52. Output and diagnostic publication` [1145-1152]
  Preview: Output events, state changes, diagnostics, migration reports, and semantic change sets are staged values.

- `Part VIII — Transactions and atomicity > 53. Forecast` [1153-1174]
  Preview: Forecasting executes the same transition function on unpublished candidate state: [ Forecast(M,\tau)=Apply(Clone(M),\tau) ] It must use the same: * reaction evaluation; * deadline processing; * migration; * provenance; * diagnostics; * budgets; * failure semantics.

- `Part IX — Reconfiguration` [1175-1338]
  Preview: A topology patch is a graph rewrite with an explicit preserved interface: [ G_{old} \xleftarrow{} P \xrightarrow{} G_{new} ] where (P) identifies preserved structural subjects.
  Symbols: `RejectStateLoss`, `AllowReportedStateLoss`, `LevelChanged`, `LevelEstablished`

- `Part IX — Reconfiguration > 54. Graph rewrite model` [1177-1192]
  Preview: A topology patch is a graph rewrite with an explicit preserved interface: [ G_{old} \xleftarrow{} P \xrightarrow{} G_{new} ] where (P) identifies preserved structural subjects.

- `Part IX — Reconfiguration > 55. Two-stage patch handling` [1193-1233]
  Preview: Patch handling is divided into: 1.

- `Part IX — Reconfiguration > 55. Two-stage patch handling > 55.1 Structural preparation` [1200-1217]
  Preview: Structural preparation performs: * graph-rewrite validation; * compilation of the proposed topology; * stable-key correspondence; * reaction-cycle validation; * static compatibility analysis; * construction of migration functions; * construction of pending-event migration rules; * classification of conditional and unavoidable loss; * resolved-handle and inspection-plan invalidation analysis.

- `Part IX — Reconfiguration > 55. Two-stage patch handling > 55.2 Transaction-time finalization` [1218-1233]
  Preview: At effective time (T), the outer transaction: 1.

- `Part IX — Reconfiguration > 56. Compatibility outcomes` [1234-1258]
  Preview: Each stateful or temporal subject receives one compatibility outcome: Compatibility may depend on: * node kind; * state schema; * port shape; * semantic parameters; * timing policy; * module identity; * current value; * pending-event payload; * signal-semantics version.

- `Part IX — Reconfiguration > 57. Structural validity` [1259-1266]
  Preview: The planner must prevent dangling structure.

- `Part IX — Reconfiguration > 58. Pending-event outcomes` [1267-1280]
  Preview: Every pending event must receive one outcome: No pending event may disappear silently.

- `Part IX — Reconfiguration > 59. State-loss policy` [1281-1296]
  Preview: Under `RejectStateLoss`, commitment is permitted only if the semantic loss set is empty.
  Symbols: `RejectStateLoss`, `AllowReportedStateLoss`

- `Part IX — Reconfiguration > 60. Topology changes as reaction causes` [1297-1308]
  Preview: A topology patch may change current outputs without any external signal change.

- `Part IX — Reconfiguration > 61. Output consequences` [1309-1323]
  Preview: For a preserved level output: * compare its prior published baseline under the old topology; * with its settled value under the new topology.
  Symbols: `LevelChanged`, `LevelEstablished`

- `Part IX — Reconfiguration > 62. Exact patch preview` [1324-1338]
  Preview: An exact preview of a future patch is a forecast.

- `Part X — Causal provenance` [1339-1471]
  Preview: Causal provenance is an immutable labeled acyclic derivation graph, not an execution log.
  Symbols: `High`, `Zip`, `Merge`

- `Part X — Causal provenance > 63. Derivation structure` [1341-1353]
  Preview: Causal provenance is an immutable labeled acyclic derivation graph, not an execution log.
  Symbols: `High`, `Zip`

- `Part X — Causal provenance > 64. Causal order` [1354-1365]
  Preview: Every provenance edge must advance in at least one of: * logical time; * reaction dependency order; * migration or checkpoint establishment order.

- `Part X — Causal provenance > 65. Current support and transition cause` [1366-1385]
  Preview: Current combinational justification is distinct from the cause of the most recent transition.

- `Part X — Causal provenance > 66. Authoritative roots` [1386-1399]
  Preview: An explanation may terminate at an authoritative root such as: * declared initial state; * external input observation; * committed caller transaction; * retained temporal origin; * topology migration or reset; * snapshot checkpoint; * explicit provenance checkpoint.

- `Part X — Causal provenance > 67. Provenance checkpoints` [1400-1426]
  Preview: Provenance may be compacted by replacing older ancestry with explicit authoritative checkpoint roots.

- `Part X — Causal provenance > 68. Retention` [1427-1436]
  Preview: Let (R) be the set of required current roots and policy-selected historical roots.

- `Part X — Causal provenance > 69. Pulse provenance` [1437-1453]
  Preview: Pulse provenance records grouped contribution counts rather than inventing ordered pulse identities.
  Symbols: `Merge`

- `Part X — Causal provenance > 70. Migration provenance` [1454-1471]
  Preview: Preserved state may retain causes established under an earlier revision.

- `Part XI — Persistent diagnostics` [1472-1536]
  Preview: Persistent diagnostic conditions are represented as semantic episodes rather than repeated messages.

- `Part XI — Persistent diagnostics > 71. Diagnostic episodes` [1474-1497]
  Preview: Persistent diagnostic conditions are represented as semantic episodes rather than repeated messages.

- `Part XI — Persistent diagnostics > 72. Episode identity` [1498-1509]
  Preview: Episode identity derives from stable semantic facts such as: It does not derive from rendered wording or provenance arena IDs.

- `Part XI — Persistent diagnostics > 73. Semantic state` [1510-1523]
  Preview: Active episode state affects whether future transactions emit a “condition began” diagnostic.

- `Part XI — Persistent diagnostics > 74. Reconfiguration` [1524-1536]
  Preview: When the owning subject is: * preserved compatibly: preserve the episode if its condition retains the same meaning; * migrated: apply an explicit episode migration rule; * removed: resolve or terminate the episode with a topology consequence; * reset: reevaluate the condition from the new initial state.

- `Part XII — Inspection and observer state` [1537-1651]
  Preview: Inspection is a pure projection: [ I_P : MachineState \rightarrow InspectionSnapshot ] It must not alter: * execution; * scheduling; * migration; * fingerprints; * provenance; * semantic digests.
  Symbols: `Machine`

- `Part XII — Inspection and observer state > 75. Inspection as projection` [1539-1557]
  Preview: Inspection is a pure projection: [ I_P : MachineState \rightarrow InspectionSnapshot ] It must not alter: * execution; * scheduling; * migration; * fingerprints; * provenance; * semantic digests.

- `Part XII — Inspection and observer state > 76. Stable queries and compiled plans` [1558-1574]
  Preview: Stable inspection intent uses structural keys and requested fields.

- `Part XII — Inspection and observer state > 77. Observer-layer boundary` [1575-1593]
  Preview: Subscriptions, delivery state, and compiled inspection plans live outside the semantic `Machine`.
  Symbols: `Machine`

- `Part XII — Inspection and observer state > 78. Semantic change set` [1594-1608]
  Preview: Every successful transaction produces an immutable semantic change set containing sufficient facts for observers, including: * logical times processed; * level changes and establishments; * pulse activity; * state transitions; * event additions and removals; * diagnostic-episode changes; * topology and region changes; * provenance-root changes.

- `Part XII — Inspection and observer state > 79. Subscription update` [1609-1626]
  Preview: A subscription maintains: [ V_t = I_P(M_t) ] Its incremental delta must satisfy: [ ApplyDelta(V_{t-1},\Delta V_t)=I_P(M_t) ] Observer updates occur after semantic commit.

- `Part XII — Inspection and observer state > 80. Explanation-sensitive subscriptions` [1627-1639]
  Preview: A requested explanation may change while the observed output value remains unchanged.

- `Part XII — Inspection and observer state > 81. Cursors and resynchronization` [1640-1651]
  Preview: Observer cursors identify committed machine versions and delivery continuity.

- `Part XIII — Persistence, replay, and identity` [1652-1816]
  Preview: A machine snapshot serializes the complete semantic state sufficient to determine all future behavior.

- `Part XIII — Persistence, replay, and identity > 82. Snapshot sufficiency` [1654-1661]
  Preview: A machine snapshot serializes the complete semantic state sufficient to determine all future behavior.

- `Part XIII — Persistence, replay, and identity > 83. Snapshot contents` [1662-1690]
  Preview: A semantic snapshot includes: * lifecycle state; * logical time; * topology revision; * network fingerprint; * semantic versions; * external level valuation; * stateful and temporal-node state; * pending event calendar; * output baselines; * required provenance roots or checkpoints; * active diagnostic episodes; * execution-state digest; * observable-state digest; * runtime policy identity where required.

- `Part XIII — Persistence, replay, and identity > 84. Restoration` [1691-1712]
  Preview: Restoration is validation, not deserialization alone.

- `Part XIII — Persistence, replay, and identity > 85. Replay` [1713-1729]
  Preview: Replay is repeated application of the deterministic transition function: [ Replay(M,T)=foldl(\delta,M,T) ] Replay frames should include: * expected previous execution-state digest; * expected revision; * transaction; * resulting execution-state digest.

- `Part XIII — Persistence, replay, and identity > 86. Replay concatenation` [1730-1749]
  Preview: For transaction sequences (A) and (B): [ Replay(M,A+!!+B) ================ Replay(Replay(M,A),B) ] where every transaction remains valid at its application point.

- `Part XIII — Persistence, replay, and identity > 87. Digest scopes` [1750-1797]
  Preview: The architecture distinguishes three digest scopes.

- `Part XIII — Persistence, replay, and identity > 87. Digest scopes > 87.1 Execution-state digest` [1754-1774]
  Preview: Covers state capable of affecting future execution: * topology fingerprint; * logical time; * revision; * external levels; * stateful and temporal state; * pending events; * active diagnostic episodes.

- `Part XIII — Persistence, replay, and identity > 87. Digest scopes > 87.2 Observable-state digest` [1775-1785]
  Preview: Extends execution state with current inspectable facts such as: * current output baselines; * required current provenance roots; * current explanation checkpoint state; * current diagnostic evidence.

- `Part XIII — Persistence, replay, and identity > 87. Digest scopes > 87.3 Snapshot digest` [1786-1797]
  Preview: Covers the complete canonical snapshot artifact, including: * observable state; * retained optional provenance history; * checkpoint metadata; * persisted observation metadata; * schema versions.

- `Part XIII — Persistence, replay, and identity > 88. Canonical encoding` [1798-1816]
  Preview: Digests derive from canonical semantic encoding independent of: * dense-index assignment; * memory addresses; * allocation order; * hash iteration; * heap shape; * presentation metadata.

- `Part XIV — Runtime policy and replay` [1817-1877]
  Preview: Configurable limits that may change transaction success or failure are explicit immutable runtime policy.

- `Part XIV — Runtime policy and replay > 89. Runtime policy` [1819-1834]
  Preview: Configurable limits that may change transaction success or failure are explicit immutable runtime policy.

- `Part XIV — Runtime policy and replay > 90. Runtime policy identity` [1835-1861]
  Preview: Semantically relevant policy fields contribute to a canonical: Exact operational replay requires: Machines may share execution state while using different policies.

- `Part XIV — Runtime policy and replay > 91. Budget failure` [1862-1877]
  Preview: Exceeding a budget: * fails the complete outer transaction; * leaves the published machine unchanged; * identifies the exceeded budget; * reports the configured limit and consumed amount where practical; * never skips work or publishes partial settlement.

- `Part XV — Errors and containment` [1878-1933]
  Preview: Examples include: * malformed definitions; * stale revisions; * wrong signal kinds; * invalid time progression; * corrupted snapshots; * incompatible patches.
  Symbols: `unsafe`

- `Part XV — Errors and containment > 92. Invalid caller-controlled data` [1880-1892]
  Preview: Examples include: * malformed definitions; * stale revisions; * wrong signal kinds; * invalid time progression; * corrupted snapshots; * incompatible patches.

- `Part XV — Errors and containment > 93. Expected semantic rejection` [1893-1903]
  Preview: Examples include: * conflict-rejecting state transitions; * prohibited state loss; * checked time overflow; * runtime budget exhaustion.

- `Part XV — Errors and containment > 94. Internal invariant violations` [1904-1919]
  Preview: Examples include: * a compiled reaction edge violates topological order; * a state slot belongs to the wrong family; * a migrated event has no valid owner; * committed provenance contains a cycle; * a stable-key lookup is ambiguous after successful compilation.

- `Part XV — Errors and containment > 95. Unsafe code` [1920-1933]
  Preview: The initial implementation should prefer safe Rust.
  Symbols: `unsafe`

- `Part XVI — Subsystems and crate boundaries` [1934-1982]
  Preview: The initial implementation should use one cohesive Rust library crate with strong internal module boundaries.

- `Part XVI — Subsystems and crate boundaries > 96. Initial crate structure` [1936-1950]
  Preview: The initial implementation should use one cohesive Rust library crate with strong internal module boundaries.

- `Part XVI — Subsystems and crate boundaries > 97. Internal dependency direction` [1951-1982]
  Preview: The internal subsystem dependency graph should remain acyclic.

- `Part XVII — Performance direction` [1983-2044]
  Preview: SCC decomposition, topological ordering, region discovery, and adjacency construction should be approximately: [ O(|V|+|E|) ] Canonical sorting for fingerprints may add: [ O(|V|\log |V| + |E|\log |E|) ] where required.

- `Part XVII — Performance direction > 98. Compilation complexity` [1985-2000]
  Preview: SCC decomposition, topological ordering, region discovery, and adjacency construction should be approximately: [ O(|V|+|E|) ] Canonical sorting for fingerprints may add: [ O(|V|\log |V| + |E|\log |E|) ] where required.

- `Part XVII — Performance direction > 99. Reaction complexity` [2001-2012]
  Preview: Incremental reaction evaluation should approach: [ O(|V_a|+|E_a|) ] plus worklist overhead, where (V_a) and (E_a) are the affected reaction closure.

- `Part XVII — Performance direction > 100. Temporal complexity` [2013-2026]
  Preview: Ordinary deadline insertion and extraction may initially use: [ O(\log N) ] calendar operations.

- `Part XVII — Performance direction > 101. Reconfiguration complexity` [2027-2036]
  Preview: Full recompilation and full region recomputation are acceptable initially.

- `Part XVII — Performance direction > 102. Inspection and persistence complexity` [2037-2044]
  Preview: Direct dense-field inspection should be approximately constant time.

- `Part XVIII — Reference paths and verification hooks` [2045-2078]
  Preview: The architecture should retain simple correctness references: * full topological reaction evaluator; * clone-and-swap transaction execution; * ordered-map event calendar; * full graph recompilation; * full region recomputation; * stable-keyed snapshot form; * canonical semantic digest input.

- `Part XVIII — Reference paths and verification hooks > 103. Reference implementations` [2047-2060]
  Preview: The architecture should retain simple correctness references: * full topological reaction evaluator; * clone-and-swap transaction execution; * ordered-map event calendar; * full graph recompilation; * full region recomputation; * stable-keyed snapshot form; * canonical semantic digest input.

- `Part XVIII — Reference paths and verification hooks > 104. Debug invariant checks` [2061-2078]
  Preview: Debug and test configurations should support expensive validation such as: * recomputing SCCs; * verifying every reaction edge advances topologically; * comparing incremental and full reaction results; * recomputing calendar minimum; * checking provenance acyclicity; * recomputing regions; * validating active diagnostic episodes; * recomputing canonical digests; * snapshot round-trip checking.

- `Part XIX — Deliberately unspecified choices` [2079-2104]
  Preview: This specification does not mandate: * exact vector, map, heap, or arena types; * dense-index widths; * `Arc` or another sharing mechanism; * structure-of-arrays versus array-of-structures everywhere; * enum dispatch versus generated evaluator tables; * provenance interning; * timing wheels or calendar queues; * incremental SCC maintenance; * fully dynamic connectivity; * parallel reaction evaluation; * SIMD execution; * custom allocators; * incremental fingerprint maintenance; * exact digest algorithm; * serialized wire encoding.
  Symbols: `Arc`

- `Part XIX — Deliberately unspecified choices > 105. Open implementation freedom` [2081-2104]
  Preview: This specification does not mandate: * exact vector, map, heap, or arena types; * dense-index widths; * `Arc` or another sharing mechanism; * structure-of-arrays versus array-of-structures everywhere; * enum dispatch versus generated evaluator tables; * provenance interning; * timing wheels or calendar queues; * incremental SCC maintenance; * fully dynamic connectivity; * parallel reaction evaluation; * SIMD execution; * custom allocators; * incremental fingerprint maintenance; * exact digest algorithm; * serialized wire encoding.
  Symbols: `Arc`

- `Part XX — Required architectural properties` [2105-2146]
  Preview: The processor architecture must preserve: Optimizations are valid only when they preserve these properties.

- `Part XX — Required architectural properties > 106. Required guarantees` [2107-2146]
  Preview: The processor architecture must preserve: Optimizations are valid only when they preserve these properties.

- `Summary` [2147-2179]
  Preview: `mossignal` is implemented as a deterministic synchronous transition system over an immutable compiled reaction graph and a mutable semantic store.
  Symbols: `mossignal`
