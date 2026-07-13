## docs/specs/testing_and_verification_policy.md
- ``mossignal` Testing and Verification Policy` [1-56]
  Preview: **Status:** Design specification, version 1 **Defines:** Verification obligations, reference semantics, conformance testing, property-based testing, differential testing, bounded exhaustive exploration, fuzzing, fault injection, debug invariant checking, regression artifacts, and CI/release gates **Does not define:** Public signal semantics, built-in node semantics, processor architecture, serialized wire formats, performance targets, editor testing, application integration, or unrestricted formal verification This specification defines how `mossignal` demonstrates that an implementation preserves the semantics and architectural invariants established by the API, built-in node, and processor specifications.
  Symbols: `mossignal`
  Normative: MUST NOT 1, MUST 1, SHOULD NOT 1, SHOULD 1, MAY 1

- ``mossignal` Testing and Verification Policy > 1. Purpose` [9-38]
  Preview: This specification defines how `mossignal` demonstrates that an implementation preserves the semantics and architectural invariants established by the API, built-in node, and processor specifications.
  Symbols: `mossignal`

- ``mossignal` Testing and Verification Policy > 2. Normative language` [39-56]
  Preview: The terms **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.
  Normative: MUST NOT 1, MUST 1, SHOULD NOT 1, SHOULD 1, MAY 1

- `Part I — Verification model` [57-228]
  Preview: For verification purposes, a ready machine is modeled as a deterministic partial transition system.
  Symbols: `τ`, `M'`, `Reference`, `Candidate`
  Normative: MUST NOT 1, MUST 1

- `Part I — Verification model > 3. System under verification` [59-80]
  Preview: For verification purposes, a ready machine is modeled as a deterministic partial transition system.
  Symbols: `τ`, `M'`

- `Part I — Verification model > 4. Semantic observation` [81-107]
  Preview: A comparison harness must be able to derive a canonical semantic observation from a machine and result.

- `Part I — Verification model > 5. Semantic equivalence` [108-139]
  Preview: Two outcomes are **semantically equivalent** when they agree on every observable fact defined by the applicable specifications after removing representation-only differences.
  Normative: MUST NOT 1, MUST 1

- `Part I — Verification model > 6. Refinement obligations` [140-185]
  Preview: Let `Reference` denote a simple implementation of specified semantics and `Candidate` an optimized or incremental implementation.
  Symbols: `Reference`, `Candidate`

- `Part I — Verification model > 7. Categories of obligation` [186-228]
  Preview: Verification requirements fall into five categories.

- `Part I — Verification model > 7. Categories of obligation > 7.1 Invariant obligations` [190-201]
  Preview: A property must hold for every reachable state.

- `Part I — Verification model > 7. Categories of obligation > 7.2 Equivalence obligations` [202-207]
  Preview: Two execution methods must produce semantically equivalent results.

- `Part I — Verification model > 7. Categories of obligation > 7.3 Completeness obligations` [208-218]
  Preview: A classification or explanation must account for every relevant item.

- `Part I — Verification model > 7. Categories of obligation > 7.4 Failure-atomicity obligations` [219-222]
  Preview: Every structured rejection must leave the published semantic machine unchanged.

- `Part I — Verification model > 7. Categories of obligation > 7.5 Compatibility obligations` [223-228]
  Preview: Persisted or migrated artifacts must be accepted exactly when the applicable compatibility rules hold and rejected precisely otherwise.

- `Part II — Verification layers` [229-266]
  Preview: The implementation must use several complementary layers.

- `Part II — Verification layers > 8. Required verification layers` [231-266]
  Preview: The implementation must use several complementary layers.

- `Part II — Verification layers > 8. Required verification layers > 8.1 Specification examples` [235-238]
  Preview: Focused examples establish named boundary cases and serve as readable executable documentation.

- `Part II — Verification layers > 8. Required verification layers > 8.2 Conformance suites` [239-242]
  Preview: Reusable suites verify that every implementation of a semantic family satisfies the common obligations of that family.

- `Part II — Verification layers > 8. Required verification layers > 8.3 Property-based testing` [243-246]
  Preview: Generated valid structures and histories exercise broad state spaces and semantic interactions.

- `Part II — Verification layers > 8. Required verification layers > 8.4 Differential testing` [247-250]
  Preview: Candidate implementations are compared against reference implementations.

- `Part II — Verification layers > 8. Required verification layers > 8.5 Bounded exhaustive exploration` [251-254]
  Preview: Finite small domains are enumerated completely where feasible.

- `Part II — Verification layers > 8. Required verification layers > 8.6 Fuzzing` [255-258]
  Preview: Untrusted encoded or dynamically authored data is subjected to adversarial malformed input, while valid structured fuzzers exercise long semantic histories.

- `Part II — Verification layers > 8. Required verification layers > 8.7 Regression testing` [259-266]
  Preview: Every confirmed defect receives a permanent minimized reproducer.

- `Part III — Reference implementations` [267-356]
  Preview: The project MUST retain simple correctness-oriented reference paths for the following subsystems.
  Normative: MUST 2, SHOULD 1

- `Part III — Reference implementations > 9. Required reference paths` [269-328]
  Preview: The project MUST retain simple correctness-oriented reference paths for the following subsystems.
  Normative: MUST 1

- `Part III — Reference implementations > 9. Required reference paths > 9.1 Full topological reaction evaluator` [273-280]
  Preview: The reference reaction evaluator processes every reaction operation exactly once in one valid deterministic topological order.

- `Part III — Reference implementations > 9. Required reference paths > 9.2 Clone-and-swap transaction executor` [281-286]
  Preview: The reference transaction path clones the complete semantic machine, executes the transaction on the clone, and replaces the original only on success.

- `Part III — Reference implementations > 9. Required reference paths > 9.3 Ordered semantic event calendar` [287-294]
  Preview: The reference calendar represents deadlines through an ordered mapping from exact logical time to complete equal-deadline batches.

- `Part III — Reference implementations > 9. Required reference paths > 9.4 Full validation and compilation` [295-310]
  Preview: The reference topology path rebuilds all derived structures from the complete authored definition, including: - stable-key lookup; - dependency graph; - SCC decomposition; - deterministic topological order; - state layouts; - temporal descriptors; - regions; - graph-query metadata; - fingerprint input.

- `Part III — Reference implementations > 9. Required reference paths > 9.5 Full region recomputation` [311-314]
  Preview: The reference region implementation recomputes weakly connected components from the complete structural graph.

- `Part III — Reference implementations > 9. Required reference paths > 9.6 Stable-keyed canonical state representation` [315-318]
  Preview: The reference persistence and digest representation is expressed through stable semantic identity rather than dense runtime positions.

- `Part III — Reference implementations > 9. Required reference paths > 9.7 Fresh inspection projection` [319-324]
  Preview: The reference inspection path computes each requested projection directly from the complete committed machine state.

- `Part III — Reference implementations > 9. Required reference paths > 9.8 Straightforward replay fold` [325-328]
  Preview: The reference replay path applies replay frames sequentially through the ordinary transition function and validates expected prior and resulting digests.

- `Part III — Reference implementations > 10. Availability of reference paths` [329-346]
  Preview: Reference paths MUST: - exist in the ordinary test configuration; - be callable by property and differential tests; - be available in a debug verification configuration where practical; - share semantic definitions with production code without sharing the optimized algorithm being verified; - remain maintained when semantics evolve.
  Normative: MUST 1

- `Part III — Reference implementations > 11. Reference path independence` [347-356]
  Preview: Reference and candidate paths SHOULD avoid sharing the exact logic whose correctness is under comparison.
  Normative: SHOULD 1

- `Part IV — Test data generation and reproducibility` [357-471]
  Preview: Property-based and structured fuzz generators SHOULD produce semantic objects rather than arbitrary bytes whenever the property requires valid input.
  Normative: MUST 2, SHOULD 3

- `Part IV — Test data generation and reproducibility > 12. Structured generators` [359-385]
  Preview: Property-based and structured fuzz generators SHOULD produce semantic objects rather than arbitrary bytes whenever the property requires valid input.
  Normative: SHOULD 1

- `Part IV — Test data generation and reproducibility > 13. Valid-network generation` [386-395]
  Preview: A valid-network generator SHOULD construct the reaction graph in a way that guarantees acyclicity, for example by: 1.
  Normative: SHOULD 1

- `Part IV — Test data generation and reproducibility > 14. Invalid-network generation` [396-416]
  Preview: Invalid generators should target one or a small number of defects deliberately rather than relying only on random corruption.

- `Part IV — Test data generation and reproducibility > 15. Transaction-history generation` [417-431]
  Preview: A transaction-history generator must preserve lifecycle and time rules unless the property intentionally tests rejection.

- `Part IV — Test data generation and reproducibility > 16. Shrinking` [432-453]
  Preview: Every generated failure SHOULD shrink toward a minimal semantic counterexample.
  Normative: SHOULD 1

- `Part IV — Test data generation and reproducibility > 17. Reproducibility` [454-471]
  Preview: Every randomized failure MUST report enough information to reproduce it, including: - random seed; - generator version where relevant; - minimized semantic artifact or serialized reproducer; - feature configuration; - runtime policy; - semantic version; - candidate and reference observations; - normalization result; - platform information when relevant.
  Normative: MUST 2

- `Part V — Validation and compilation verification` [472-543]
  Preview: Every accepted `ValidatedNetwork` MUST satisfy all static invariants required by compilation and execution.
  Symbols: `ValidatedNetwork`
  Normative: MUST 1, SHOULD 2

- `Part V — Validation and compilation verification > 18. Validation soundness` [474-487]
  Preview: Every accepted `ValidatedNetwork` MUST satisfy all static invariants required by compilation and execution.
  Symbols: `ValidatedNetwork`
  Normative: MUST 1

- `Part V — Validation and compilation verification > 19. Validation completeness over known defect classes` [488-495]
  Preview: For every enumerated validation rule, at least one focused positive case and one focused negative case are required.
  Normative: SHOULD 1

- `Part V — Validation and compilation verification > 20. Reaction-cycle verification` [496-511]
  Preview: Compilation derives a current-reaction dependency graph that must be a directed acyclic graph.
  Normative: SHOULD 1

- `Part V — Validation and compilation verification > 21. Deterministic compilation` [512-525]
  Preview: Semantically equivalent stable-keyed definitions constructed in different insertion orders must produce equivalent: - fingerprint; - structural graph view; - reaction dependency relation; - state schemas; - region partition; - graph queries; - externally observable compiled metadata.

- `Part V — Validation and compilation verification > 22. Compiled invariant checks` [526-543]
  Preview: Debug and test builds must be able to verify:

- `Part VI — Initialization verification` [544-624]
  Preview: Tests must cover both lifecycle states: An uninitialized machine must not be treated as a ready machine holding `Low` values.
  Symbols: `Periodic`, `Low`, `LevelEstablished`, `InputDelta`, `Baseline`, `Assume`, `PulseDelay`, `TransportDelay`, `InertialDelay`

- `Part VI — Initialization verification > 23. Lifecycle coverage` [546-556]
  Preview: Tests must cover both lifecycle states: An uninitialized machine must not be treated as a ready machine holding `Low` values.
  Symbols: `Low`

- `Part VI — Initialization verification > 24. First-transaction obligations` [557-572]
  Preview: The first successful transaction must be tested for: - arbitrary initial logical time; - complete authoritative level snapshot; - initial pulse batch; - declared initial state as previous state; - ordinary full reaction evaluation; - successor-state commitment; - future temporal scheduling; - provenance roots; - diagnostic episode establishment; - `LevelEstablished` output events; - ready-machine schedule after commitment.
  Symbols: `LevelEstablished`

- `Part VI — Initialization verification > 25. Initialization rejection` [573-588]
  Preview: Tests must verify structured rejection of: - `InputDelta` before initialization; - incomplete level snapshot; - duplicate or conflicting observations; - unknown endpoint; - wrong network or binding projection; - stale revision; - invalid patch; - budget failure; - checked time failure.
  Symbols: `InputDelta`

- `Part VI — Initialization verification > 26. Pre-initialization inspection` [589-601]
  Preview: Structural inspection must remain available before initialization.

- `Part VI — Initialization verification > 27. Edge-detector initialization` [602-612]
  Preview: Every edge detector must be tested under both policies: The first observation under `Baseline` must establish memory without emission.
  Symbols: `Baseline`, `Assume`

- `Part VI — Initialization verification > 28. Temporal initialization` [613-624]
  Preview: Required cases include: - fresh `PulseDelay` with no pending work; - `TransportDelay` first input equal to and different from its explicit initial level; - `InertialDelay` first input equal to and different from its explicit initial level; - fresh disabled `Periodic` remaining anchorless; - fresh enabled `Periodic` under each first-emission policy.
  Symbols: `Periodic`, `PulseDelay`, `TransportDelay`, `InertialDelay`

- `Part VII — Reaction and signal verification` [625-729]
  Preview: The full topological evaluator must be tested directly against the node laws and against hand-authored multi-node examples.
  Symbols: `High`, `Low`, `Select`, `Merge`, `Coalesce`, `Zip`
  Normative: SHOULD 1

- `Part VII — Reaction and signal verification > 29. Full reaction semantics` [627-632]
  Preview: The full topological evaluator must be tested directly against the node laws and against hand-authored multi-node examples.

- `Part VII — Reaction and signal verification > 30. Incremental reaction equivalence` [633-652]
  Preview: For generated valid networks, previous machine states, and same-time stimulus batches: Comparison must include: - settled level outputs; - pulse multiplicities; - proposed successor state; - future event additions and cancellations; - diagnostic-condition updates; - provenance roots or normalized derivations; - semantic change set.

- `Part VII — Reaction and signal verification > 31. Work-order invariance` [653-660]
  Preview: Where several reaction operations are simultaneously eligible, tests must vary the valid work order.
  Normative: SHOULD 1

- `Part VII — Reaction and signal verification > 32. Input-order invariance` [661-670]
  Preview: Equivalent same-time input batches must produce equivalent results under permutations of: - level observation insertion order; - pulse occurrence insertion order; - binding projection order; - external endpoint order; - equal-time event representation order.

- `Part VII — Reaction and signal verification > 33. Glitch freedom` [671-684]
  Preview: Required focused cases include: - two inputs changing oppositely while `Any` remains `High`; - two inputs changing oppositely while `All` remains `Low`; - parity-preserving simultaneous changes; - `Select` branch and selector changes at one time; - multi-layer reconvergent combinational paths; - edge detectors downstream of settlement-equivalent changes; - stateful nodes whose current outputs feed downstream edge detectors or stateful nodes.
  Symbols: `High`, `Low`, `Select`

- `Part VII — Reaction and signal verification > 34. Pulse algebra` [685-701]
  Preview: Tests must verify: - pulse multiplicity is a non-negative count; - fan-out copies the full count to every destination; - duplicate sources are counted once per connected port; - `Merge` sums counts; - `Coalesce` maps positive counts to one; - `Zip` takes the minimum and does not retain unmatched pulses; - pulse routing preserves selected multiplicity; - no pulse persists as current state after its reaction.
  Symbols: `Merge`, `Coalesce`, `Zip`

- `Part VII — Reaction and signal verification > 35. Level algebra` [702-717]
  Preview: The level combinational catalogue must be tested exhaustively over small arities and all input valuations where feasible.

- `Part VII — Reaction and signal verification > 36. Current-state isolation` [718-729]
  Preview: Every stateful node test must confirm that: - current output may reflect current inputs according to the node law; - every state transition reads the same previous committed state; - proposed successor state is not exposed as stored state during the reaction; - one state cell proposes at most one successor value; - all successor state commits together after successful settlement.

- `Part VIII — Built-in node conformance` [730-814]
  Preview: Every built-in node kind MUST implement a reusable conformance suite before it may be considered complete.
  Symbols: `RetainAndDiagnose`
  Normative: MUST 1, SHOULD 1

- `Part VIII — Built-in node conformance > 37. Node conformance requirement` [732-737]
  Preview: Every built-in node kind MUST implement a reusable conformance suite before it may be considered complete.
  Normative: MUST 1

- `Part VIII — Built-in node conformance > 38. Common conformance matrix` [738-773]
  Preview: For each primitive, the test inventory must explicitly classify and cover: A field that does not apply must be marked explicitly as not applicable.

- `Part VIII — Built-in node conformance > 39. Exhaustive primitive testing` [774-787]
  Preview: Where the complete input and state domain is finite and small, primitive laws SHOULD be exhaustively enumerated.
  Normative: SHOULD 1

- `Part VIII — Built-in node conformance > 40. Stateful chain testing` [788-799]
  Preview: Focused multi-node cases must cover same-reaction chains such as: The tests must confirm current downstream visibility together with one atomic successor-state commit.

- `Part VIII — Built-in node conformance > 41. Conflict policies` [800-814]
  Preview: Both set/reset latch kinds must test every conflict policy: For level-controlled conflict, `RetainAndDiagnose` must be tested as a persistent diagnostic episode.
  Symbols: `RetainAndDiagnose`

- `Part IX — Temporal verification` [815-952]
  Preview: Tests must use arbitrary initial times and non-unit spans.
  Symbols: `origin + delay`, `next_deadline`, `Schedule::WakeAt`

- `Part IX — Temporal verification > 42. Exact discrete time` [817-829]
  Preview: Tests must use arbitrary initial times and non-unit spans.

- `Part IX — Temporal verification > 43. Strictly future scheduling` [830-841]
  Preview: Every newly created temporal obligation must satisfy: Generated semantic histories should assert this invariant after every successful reaction.

- `Part IX — Temporal verification > 44. Direct-jump equivalence` [842-859]
  Preview: For any ready machine `M` and future target `T`, let the pending deadlines before `T` be: Tests must compare: with an equivalent sequence that advances through each meaningful deadline and then `T`, accounting for the rule that caller transactions themselves must use strictly increasing times.

- `Part IX — Temporal verification > 45. Equal-deadline batching` [860-873]
  Preview: Events sharing one deadline must be processed as one unordered batch.

- `Part IX — Temporal verification > 46. Exact-deadline boundary triad` [874-885]
  Preview: Every temporal cancellation or control law must include cases where the relevant input occurs: This triad is mandatory for inertial cancellation, periodic enable control, and any future temporal primitive with deadline-sensitive input.

- `Part IX — Temporal verification > 47. `PulseDelay`` [886-897]
  Preview: Tests must verify: - every input occurrence is reproduced once at `origin + delay`; - multiplicity is preserved; - groups with equal deadline may aggregate semantically; - new input at the due time does not suppress the due group; - new input schedules only strictly future work; - duration change preserves existing deadlines by default; - every alternative migration policy is explicit and complete.
  Symbols: `origin + delay`

- `Part IX — Temporal verification > 48. `TransportDelay`` [898-910]
  Preview: Tests must verify: - every settled input transition is queued; - short-lived transitions are preserved; - current input has no instantaneous path to current output; - a due transition matures despite a new same-time input transition; - remembered input updates correctly; - same-deadline migrated transitions resolve by greatest originating logical time; - no same-time intermediate output transition is observable; - indistinguishable conflicting origins reject or resolve explicitly.

- `Part IX — Temporal verification > 49. `InertialDelay`` [911-921]
  Preview: Tests must verify: - candidate creation only when input differs from the output remaining after due obligations; - contradictory change strictly before deadline cancels; - contradictory change exactly at deadline does not cancel maturation; - the exact-deadline contradiction may create a new opposite candidate; - current input has no instantaneous path to current output; - duration-change migration policies preserve their specified origin and deadline semantics.

- `Part IX — Temporal verification > 50. `Periodic`` [922-939]
  Preview: Tests must cover every combination of: Tests must also verify: - disabled boundaries emit nothing and do not accumulate; - large jumps preserve separate emissions at actual boundary times; - settled same-time enable controls a due boundary; - reconfiguration policies define anchor and next-deadline outcomes explicitly.

- `Part IX — Temporal verification > 51. Event cancellation and next deadline` [940-952]
  Preview: After cancellation, an event must: - not fire; - not appear as pending; - not determine `next_deadline` or `Schedule::WakeAt`; - remain only in explicitly retained optional history, if such history exists.
  Symbols: `next_deadline`, `Schedule::WakeAt`

- `Part X — Transaction and failure-atomicity verification` [953-1041]
  Preview: Tests must establish that public inspection observes either the complete old machine or the complete new machine, never a partially published intermediate.
  Normative: SHOULD 1

- `Part X — Transaction and failure-atomicity verification > 52. Single-publication semantics` [955-960]
  Preview: Tests must establish that public inspection observes either the complete old machine or the complete new machine, never a partially published intermediate.

- `Part X — Transaction and failure-atomicity verification > 53. Candidate-versus-clone equivalence` [961-972]
  Preview: For generated machines and transactions: This comparison applies to both success and structured failure.

- `Part X — Transaction and failure-atomicity verification > 54. Failure atomicity` [973-995]
  Preview: For every structured failure category, the test harness must capture the complete semantic observation before execution and compare it after rejection.

- `Part X — Transaction and failure-atomicity verification > 55. Fault injection` [996-1017]
  Preview: Test-only fault injection SHOULD cover every fallible preparation stage that mutates candidate state or allocates semantic artifacts.
  Normative: SHOULD 1

- `Part X — Transaction and failure-atomicity verification > 56. Rejection categories` [1018-1033]
  Preview: Focused tests must cover at least: - stale revision; - stale expected execution digest; - invalid lifecycle operation; - non-increasing time; - checked time overflow; - wrong network identity; - conflict-rejecting latch; - prohibited state loss; - incompatible patch; - invalid snapshot restoration; - runtime budget exhaustion.

- `Part X — Transaction and failure-atomicity verification > 57. No host effects during preparation` [1034-1041]
  Preview: The evaluator must not invoke host callbacks during propagation or preparation.

- `Part XI — Reconfiguration verification` [1042-1156]
  Preview: Tests must distinguish: Structural preparation must depend only on the base topology and static patch content.
  Symbols: `RejectStateLoss`, `AllowReportedStateLoss`

- `Part XI — Reconfiguration verification > 58. Two-stage patch semantics` [1044-1054]
  Preview: Tests must distinguish: Structural preparation must depend only on the base topology and static patch content.

- `Part XI — Reconfiguration verification > 59. Prepared-patch freshness` [1055-1063]
  Preview: Tests must verify: - state-only transactions do not invalidate structural preparation; - topology revision changes do invalidate it; - applying a prepared patch to the wrong base revision fails structurally; - exact forecasts are additionally bound to execution-state digest, effective time, and runtime policy.

- `Part XI — Reconfiguration verification > 60. State reached at effective time` [1064-1072]
  Preview: A required differential scenario is: 1.

- `Part XI — Reconfiguration verification > 61. Complete migration classification` [1073-1095]
  Preview: For every surviving stateful or temporal subject, finalization must produce exactly one outcome: For every pending event, finalization must produce exactly one outcome: Tests must assert that the classification is total and contains no duplicate ownership.

- `Part XI — Reconfiguration verification > 62. State-loss policy` [1096-1111]
  Preview: Under `RejectStateLoss`, any nonempty finalized semantic loss set must reject the complete transaction.
  Symbols: `RejectStateLoss`, `AllowReportedStateLoss`

- `Part XI — Reconfiguration verification > 63. Region merge and split` [1112-1117]
  Preview: Adding or removing connections may merge or split weak regions.

- `Part XI — Reconfiguration verification > 64. Topology-induced reaction` [1118-1131]
  Preview: After migration, every potentially affected current-reaction operation must be reevaluated.

- `Part XI — Reconfiguration verification > 65. Stable and resolved identity` [1132-1142]
  Preview: Tests must verify: - preserved stable keys retain identity; - removed keys become unavailable; - new keys do not alias removed subjects accidentally; - stale dense handles fail structurally; - stale compiled inspection plans fail structurally; - dense-slot reuse never transfers state, provenance, or diagnostic episodes to another subject.

- `Part XI — Reconfiguration verification > 66. Patch differential testing` [1143-1156]
  Preview: Generated small patches should be applied through: and an independent reference path that rebuilds the new topology, computes stable-key correspondence, applies explicit migration laws, and fully reevaluates the new graph.

- `Part XII — Provenance and explanation verification` [1157-1240]
  Preview: Every committed provenance derivation must be acyclic.
  Symbols: `Parity`, `AtLeast`, `Zip`

- `Part XII — Provenance and explanation verification > 67. Provenance acyclicity` [1159-1166]
  Preview: Every committed provenance derivation must be acyclic.

- `Part XII — Provenance and explanation verification > 68. Authoritative-root completeness` [1167-1182]
  Preview: Every required current historical fact must have a backward path to an authoritative root.

- `Part XII — Provenance and explanation verification > 69. Joint and unordered support` [1183-1195]
  Preview: For nodes such as `All`, `Any`, `Parity`, `AtLeast`, `Zip`, and grouped pulse merges, supporter order is not semantic.
  Symbols: `Parity`, `AtLeast`, `Zip`

- `Part XII — Provenance and explanation verification > 70. Current support versus latest transition` [1196-1203]
  Preview: Focused tests must change the current supporting facts of an unchanged level output.

- `Part XII — Provenance and explanation verification > 71. Checkpoint compaction` [1204-1214]
  Preview: Provenance compaction tests must compare future behavior and current explanation before and after replacing older ancestry with an authoritative checkpoint.

- `Part XII — Provenance and explanation verification > 72. Migration provenance` [1215-1224]
  Preview: Tests must verify: - preserved state retains applicable ancestry; - migrated state derives from old state plus migration rule; - reset state receives an explicit reset or initialization cause; - preserved pending events retain scheduling ancestry plus migration facts; - canceled required work is reported as semantic loss where applicable.

- `Part XII — Provenance and explanation verification > 73. Why-not explanations` [1225-1240]
  Preview: For supported requests, tests must construct known blocked outcomes and verify that explanations identify actual: - unsatisfied prerequisites; - blockers; - conflicts; - disabled controls; - disconnections; - pending conditions; - nearest relevant paths.

- `Part XIII — Diagnostic verification` [1241-1298]
  Preview: Tests must compare diagnostics structurally, not primarily through rendered prose.

- `Part XIII — Diagnostic verification > 74. Structured diagnostic conformance` [1243-1260]
  Preview: Tests must compare diagnostics structurally, not primarily through rendered prose.

- `Part XIII — Diagnostic verification > 75. Persistent episode lifecycle` [1261-1279]
  Preview: For every persistent condition, tests must cover: An unchanged active condition must not emit the same warning on every unrelated transaction.

- `Part XIII — Diagnostic verification > 76. Episode identity` [1280-1285]
  Preview: Episode identity must derive from stable semantic facts rather than prose, dense slots, or provenance arena identifiers.

- `Part XIII — Diagnostic verification > 77. Validation report behavior` [1286-1298]
  Preview: Validation, compilation, binding, and patch preparation reports should collect independent findings where safe.

- `Part XIV — Inspection and observer verification` [1299-1359]
  Preview: Inspection must be a pure projection.
  Symbols: `M_prev`, `M_next`

- `Part XIV — Inspection and observer verification > 78. Inspection purity` [1301-1314]
  Preview: Inspection must be a pure projection.

- `Part XIV — Inspection and observer verification > 79. Stable query and compiled plan equivalence` [1315-1328]
  Preview: For a valid revision-bound plan: After topology revision change, the stale plan must fail structurally and must not silently retarget.

- `Part XIV — Inspection and observer verification > 80. Incremental subscription equivalence` [1329-1338]
  Preview: For every committed transition from `M_prev` to `M_next`, an observer projection and delta must satisfy: Differential tests must compare incremental observer updates against fresh projection.
  Symbols: `M_prev`, `M_next`

- `Part XIV — Inspection and observer verification > 81. Explanation-sensitive invalidation` [1339-1344]
  Preview: Subscriptions requesting explanation data must update when causal support changes even if the observed signal value does not.

- `Part XIV — Inspection and observer verification > 82. Cursor and resynchronization` [1345-1359]
  Preview: Observer tests must cover: - uninterrupted cursor progression; - topology revision change; - missed retained change set; - expired history; - explicit resynchronization; - delivery failure after semantic commit.

- `Part XV — Persistence, forecast, replay, and digest verification` [1360-1475]
  Preview: For every generated valid machine, including uninitialized machines: must produce a semantically equivalent machine under a compatible compiled topology and runtime policy.
  Symbols: `M_r`, `τ`

- `Part XV — Persistence, forecast, replay, and digest verification > 83. Snapshot round trip` [1362-1373]
  Preview: For every generated valid machine, including uninitialized machines: must produce a semantically equivalent machine under a compatible compiled topology and runtime policy.

- `Part XV — Persistence, forecast, replay, and digest verification > 84. Snapshot sufficiency` [1374-1383]
  Preview: For restored machine `M_r` equivalent to original `M`, and every compatible future transaction sequence `T`: Property tests should exercise future histories containing state changes, temporal deadlines, reconfiguration, diagnostics, explanations, and further snapshots.
  Symbols: `M_r`

- `Part XV — Persistence, forecast, replay, and digest verification > 85. Restoration rejection` [1384-1405]
  Preview: Malformed or incompatible snapshots must fail precisely for classes including: - schema version mismatch; - semantic version mismatch; - fingerprint mismatch; - lifecycle inconsistency; - missing or duplicate state facts; - invalid state schema; - unknown stable subject; - invalid external valuation; - invalid pending-event owner; - non-future pending deadline; - inconsistent output baseline; - missing or cyclic required provenance; - invalid diagnostic episode; - runtime-policy incompatibility where required; - digest mismatch.

- `Part XV — Persistence, forecast, replay, and digest verification > 86. Forecast equivalence` [1406-1419]
  Preview: For machine `M` and transaction `τ`: The original machine must remain unchanged on both forecast success and forecast failure.
  Symbols: `τ`

- `Part XV — Persistence, forecast, replay, and digest verification > 87. Replay equivalence` [1420-1431]
  Preview: A recorded successful execution and replay from the same compatible starting snapshot must produce equivalent: - output events; - state changes; - topology revisions; - diagnostics and active episodes; - provenance roots and checkpoints; - schedules; - final execution and observable digests.

- `Part XV — Persistence, forecast, replay, and digest verification > 88. Replay concatenation` [1432-1443]
  Preview: For valid transaction sequences `A` and `B`: Tests must include replay boundaries immediately before and after temporal deadlines and topology patches.

- `Part XV — Persistence, forecast, replay, and digest verification > 89. Digest-scope verification` [1444-1461]
  Preview: Tests must establish the intended distinctions among: Required cases include: - optional retained history differs while execution-state digest remains equal; - current explanation checkpoint state differs and observable digest changes; - complete snapshot metadata differs and snapshot digest changes; - dense-index or allocation differences do not change semantic digests; - presentation metadata does not change topology fingerprint or semantic digests where excluded.

- `Part XV — Persistence, forecast, replay, and digest verification > 90. Canonical encoding vectors` [1462-1475]
  Preview: Once canonical encoding is specified, the project must retain golden vectors for representative: - network fingerprints; - execution-state digests; - observable-state digests; - snapshot digests; - runtime-policy identifiers.

- `Part XVI — Runtime-policy and budget verification` [1476-1522]
  Preview: Semantically relevant runtime policy fields must contribute to `RuntimePolicyId` canonically.
  Symbols: `RuntimePolicyId`

- `Part XVI — Runtime-policy and budget verification > 91. Policy identity` [1478-1488]
  Preview: Semantically relevant runtime policy fields must contribute to `RuntimePolicyId` canonically.
  Symbols: `RuntimePolicyId`

- `Part XVI — Runtime-policy and budget verification > 92. Budget boundaries` [1489-1508]
  Preview: Every budget must be tested at: where representable.

- `Part XVI — Runtime-policy and budget verification > 93. Budget failure atomicity` [1509-1522]
  Preview: Exceeding a budget must: - reject the entire outer transaction; - preserve the published machine exactly; - identify the exceeded budget; - report limit and consumed amount where practical; - never skip semantic work or publish partial settlement.

- `Part XVII — Property-based and differential campaigns` [1523-1581]
  Preview: The ordinary test suite must include property campaigns covering at least: Extended CI should execute generated histories containing hundreds or thousands of successful and rejected operations, including: - state changes; - temporal scheduling and cancellation; - forecasts; - snapshots and restorations; - patch preparation and commitment; - stale artifact attempts; - diagnostic episode changes; - provenance checkpoints.

- `Part XVII — Property-based and differential campaigns > 94. Mandatory property campaigns` [1525-1547]
  Preview: The ordinary test suite must include property campaigns covering at least:

- `Part XVII — Property-based and differential campaigns > 95. Long-history campaigns` [1548-1562]
  Preview: Extended CI should execute generated histories containing hundreds or thousands of successful and rejected operations, including: - state changes; - temporal scheduling and cancellation; - forecasts; - snapshots and restorations; - patch preparation and commitment; - stale artifact attempts; - diagnostic episode changes; - provenance checkpoints.

- `Part XVII — Property-based and differential campaigns > 96. Metamorphic properties` [1563-1581]
  Preview: Where a direct oracle is expensive, tests may use transformations that must preserve or predictably change semantics.

- `Part XVIII — Bounded exhaustive exploration and model checking` [1582-1654]
  Preview: Bounded exhaustive exploration provides complete verification over deliberately small finite domains.
  Normative: SHOULD 1

- `Part XVIII — Bounded exhaustive exploration and model checking > 97. Purpose` [1584-1589]
  Preview: Bounded exhaustive exploration provides complete verification over deliberately small finite domains.

- `Part XVIII — Bounded exhaustive exploration and model checking > 98. Required bounded exploration` [1590-1605]
  Preview: The project SHOULD exhaustively enumerate: - all valuations of each finite primitive input domain; - all previous-state values for Boolean state cells; - bounded pulse counts sufficient to distinguish zero, odd, even, one, and many; - every simultaneous set/reset presence combination; - every edge-detector initialization and first-input combination; - exact-deadline before/at/after cases; - small event calendars; - small reaction DAGs and their topological orders; - short transaction sequences over small generated networks; - small reconfiguration preservation and rejection cases; - diagnostic episode transition systems.
  Normative: SHOULD 1

- `Part XVIII — Bounded exhaustive exploration and model checking > 99. State-space bounds` [1606-1622]
  Preview: Every exhaustive harness must state its bounds explicitly, for example: A passing bounded result must not be described as proof beyond those bounds.

- `Part XVIII — Bounded exhaustive exploration and model checking > 100. Transition-system exploration` [1623-1639]
  Preview: For small machine models, the harness may enumerate reachable states under all valid bounded transactions.

- `Part XVIII — Bounded exhaustive exploration and model checking > 101. External formal tools` [1640-1654]
  Preview: Use of a model checker, SMT solver, proof assistant, or symbolic executor is optional unless a later specification makes it mandatory for a particular subsystem.

- `Part XIX — Fuzzing` [1655-1716]
  Preview: Coverage-guided fuzzing MUST target every boundary that accepts dynamically authored or serialized caller-controlled data, including applicable forms of: - unchecked network definitions; - serialized network definitions; - snapshots; - replay frames; - topology patches; - transactions; - binding projections; - inspection queries; - explanation requests; - canonical decoders.
  Normative: MUST 1

- `Part XIX — Fuzzing > 102. Untrusted boundaries` [1657-1671]
  Preview: Coverage-guided fuzzing MUST target every boundary that accepts dynamically authored or serialized caller-controlled data, including applicable forms of: - unchecked network definitions; - serialized network definitions; - snapshots; - replay frames; - topology patches; - transactions; - binding projections; - inspection queries; - explanation requests; - canonical decoders.
  Normative: MUST 1

- `Part XIX — Fuzzing > 103. Malformed-input fuzzing obligations` [1672-1685]
  Preview: Malformed input must not cause: - panic attributable to caller-controlled invalidity; - undefined behavior; - out-of-bounds access; - unbounded uncontrolled allocation outside runtime policy; - silent acceptance of invalid structure; - partial publication; - nondeterministic diagnostic category.

- `Part XIX — Fuzzing > 104. Structured semantic fuzzing` [1686-1698]
  Preview: A second fuzzing class should generate valid semantic objects and long operation sequences.

- `Part XIX — Fuzzing > 105. Fuzz corpus management` [1699-1716]
  Preview: Confirmed unique failures must be minimized and added to the permanent regression corpus.

- `Part XX — Debug verification and implementation safety` [1717-1789]
  Preview: Debug and test configurations should support expensive recomputation of: The checks should run at semantically meaningful boundaries, especially after compilation, reaction settlement, migration finalization, and transaction commitment.
  Symbols: `unsafe`

- `Part XX — Debug verification and implementation safety > 106. Debug invariant checks` [1719-1740]
  Preview: Debug and test configurations should support expensive recomputation of: The checks should run at semantically meaningful boundaries, especially after compilation, reaction settlement, migration finalization, and transaction commitment.

- `Part XX — Debug verification and implementation safety > 107. Assertions versus structured failures` [1741-1754]
  Preview: Tests must preserve the distinction between: - invalid caller-controlled data; - expected semantic rejection; - internal invariant violation.

- `Part XX — Debug verification and implementation safety > 108. Unsafe code verification` [1755-1771]
  Preview: The initial implementation should remain safe Rust.
  Symbols: `unsafe`

- `Part XX — Debug verification and implementation safety > 109. Concurrency verification` [1772-1789]
  Preview: If internal parallel evaluation is later introduced, it must refine the same deterministic reaction semantics.

- `Part XXI — CI and release policy` [1790-1876]
  Preview: Every ordinary change must run, at minimum: - compilation under supported core configurations; - static analysis and formatting policy; - focused unit tests; - built-in node conformance suites; - deterministic specification examples; - moderate property-based campaigns; - core differential tests; - regression corpus; - snapshot and replay smoke tests; - debug invariant checks for affected subsystems.

- `Part XXI — CI and release policy > 110. Per-change verification` [1792-1806]
  Preview: Every ordinary change must run, at minimum: - compilation under supported core configurations; - static analysis and formatting policy; - focused unit tests; - built-in node conformance suites; - deterministic specification examples; - moderate property-based campaigns; - core differential tests; - regression corpus; - snapshot and replay smoke tests; - debug invariant checks for affected subsystems.

- `Part XXI — CI and release policy > 111. Extended CI` [1807-1821]
  Preview: Extended CI should run: - larger property case counts; - longer generated histories; - all supported feature combinations; - relevant platform targets; - bounded exhaustive exploration; - fuzz corpus replay; - fault-injection campaigns; - canonical encoding vectors; - restoration incompatibility matrix; - patch migration campaigns.

- `Part XXI — CI and release policy > 112. Scheduled verification` [1822-1833]
  Preview: Scheduled or pre-release jobs should run: - long-running coverage-guided fuzzing; - high-count differential campaigns; - large temporal histories; - adversarial reconfiguration with pending events; - repeated snapshot/restore/replay cycles; - sanitizer or interpreter-based checks where applicable; - performance regression suites once performance policy exists.

- `Part XXI — CI and release policy > 113. Required release gates` [1834-1846]
  Preview: A release that claims support for a feature must not proceed while any of the following apply: - a required conformance field lacks tests; - candidate and reference paths diverge on a confirmed valid case; - a known structured failure can partially mutate published state; - a persistence golden vector changes unintentionally; - a stable diagnostic code changes unintentionally; - a supported snapshot or replay compatibility promise is broken without explicit versioning; - a confirmed panic remains reachable through malformed caller-controlled data; - a new built-in node lacks migration, inspection, explanation, and persistence coverage.

- `Part XXI — CI and release policy > 114. Flaky tests` [1847-1854]
  Preview: A test whose result depends on uncontrolled iteration order, scheduling, timing, or random seed handling is itself a defect.

- `Part XXI — CI and release policy > 115. Test runtime classification` [1855-1876]
  Preview: Tests should be labeled by expected cost so the project can run fast checks frequently without losing heavier verification.

- `Part XXII — Regression artifacts and compatibility history` [1877-1920]
  Preview: Every confirmed semantic defect must receive a minimized permanent regression test.

- `Part XXII — Regression artifacts and compatibility history > 116. Permanent regressions` [1879-1890]
  Preview: Every confirmed semantic defect must receive a minimized permanent regression test.

- `Part XXII — Regression artifacts and compatibility history > 117. Historical artifact corpus` [1891-1904]
  Preview: Once persistence formats exist, the project should retain representative historical artifacts for every supported compatibility version: - network definitions; - uninitialized snapshots; - ready snapshots; - replay logs; - active diagnostic episodes; - pending temporal work; - provenance checkpoints.

- `Part XXII — Regression artifacts and compatibility history > 118. Semantic version changes` [1905-1920]
  Preview: A change to public semantics must update: - semantic version identifiers; - affected reference behavior; - conformance tests; - compatibility expectations; - canonical vectors where applicable; - regression artifacts; - migration or rejection diagnostics.

- `Part XXIII — Requirements for future features` [1921-1979]
  Preview: A new primitive may enter the core only when: - its full semantic definition exists; - its dependency signature is tested; - its conformance matrix is complete; - its finite cases are exhaustively tested where feasible; - its state and pending-work migration are tested; - its inspection and explanation are tested; - its snapshot and replay behavior are tested; - its diagnostic occurrence or episode behavior is tested; - it participates in generated-network differential campaigns.

- `Part XXIII — Requirements for future features > 119. New built-in nodes` [1923-1936]
  Preview: A new primitive may enter the core only when: - its full semantic definition exists; - its dependency signature is tested; - its conformance matrix is complete; - its finite cases are exhaustively tested where feasible; - its state and pending-work migration are tested; - its inspection and explanation are tested; - its snapshot and replay behavior are tested; - its diagnostic occurrence or episode behavior is tested; - it participates in generated-network differential campaigns.

- `Part XXIII — Requirements for future features > 120. Standard modules` [1937-1948]
  Preview: A named standard module must verify: - public interface conformance; - canonical expansion or equivalent primitive semantics; - stable internal identity where promised; - inspection and hierarchy visibility; - explanation paths; - revision compatibility; - state preservation across compatible module updates.

- `Part XXIII — Requirements for future features > 121. New optimized paths` [1949-1960]
  Preview: Every new optimization must identify: 1.

- `Part XXIII — Requirements for future features > 122. Declarative contracts and assertions` [1961-1964]
  Preview: A future contract subsystem must define its own conformance and model-checking obligations while preserving non-interference with core machine semantics unless contracts are explicitly defined as transaction-rejecting policy.

- `Part XXIII — Requirements for future features > 123. Constrained user-defined automata` [1965-1979]
  Preview: Any future user-defined automaton facility must provide a finite, inspectable, persistable transition schema suitable for: - exhaustive bounded transition testing; - dependency-signature validation; - deterministic replay; - migration classification; - explanation generation; - malformed-definition fuzzing.

- `Part XXIV — Deliberately unspecified choices` [1980-2022]
  Preview: This policy does not mandate: - a particular Rust testing framework; - a particular property-testing library; - a particular fuzzing engine; - a particular model checker; - exact CI provider configuration; - exact randomized case counts; - exact fuzzing duration; - code-coverage percentage targets; - mutation-testing thresholds; - one canonical serialization format; - one specific sanitizer suite.
  Normative: MUST NOT 1, MAY 1

- `Part XXIV — Deliberately unspecified choices > 124. Implementation freedom` [1982-1999]
  Preview: This policy does not mandate: - a particular Rust testing framework; - a particular property-testing library; - a particular fuzzing engine; - a particular model checker; - exact CI provider configuration; - exact randomized case counts; - exact fuzzing duration; - code-coverage percentage targets; - mutation-testing thresholds; - one canonical serialization format; - one specific sanitizer suite.

- `Part XXIV — Deliberately unspecified choices > 125. Coverage metrics` [2000-2014]
  Preview: Line and branch coverage MAY be used as gap-detection tools.
  Normative: MUST NOT 1, MAY 1

- `Part XXIV — Deliberately unspecified choices > 126. Formal-verification boundary` [2015-2022]
  Preview: The project does not claim complete formal verification of arbitrary networks, unbounded event histories, persistence implementations, or the Rust compiler and runtime.

- `Part XXV — Required verification properties` [2023-2071]
  Preview: The verification system must continuously exercise and defend:

- `Part XXV — Required verification properties > 127. Required guarantees` [2025-2071]
  Preview: The verification system must continuously exercise and defend:

- `Summary` [2072-2108]
  Preview: `mossignal` verification is organized around executable semantic refinement.
  Symbols: `mossignal`
