## docs/specs/reconfiguration_and_topology_patch_spec.md
- ``mossignal` Reconfiguration and Topology-Patch Specification` [1-69]
  Preview: **Status:** Design specification, version 1 **Defines:** Topology-patch values, the exhaustive patch-operation language, declarative graph-rewrite semantics, structural preparation, stable correspondence, state and pending-work migration, module replacement, target input schemas, transaction-time finalization, semantic-loss policy, migration reports, diagnostics, and verification obligations **Does not define:** General processor execution, built-in node behavior outside reconfiguration, serialized wire encodings, the exhaustive diagnostic-code catalogue, performance targets, editor interaction, automatic multi-user patch merging, or unrestricted user-defined migration callbacks This specification defines how a running or uninitialized `mossignal` machine changes topology without introducing hidden ordering, silent state loss, stale identity, or partial publication.
  Symbols: `mossignal`
  Normative: MUST NOT 1, MUST 1, SHOULD NOT 1, SHOULD 1, MAY 1

- ``mossignal` Reconfiguration and Topology-Patch Specification > 1. Purpose` [9-43]
  Preview: This specification defines how a running or uninitialized `mossignal` machine changes topology without introducing hidden ordering, silent state loss, stale identity, or partial publication.
  Symbols: `mossignal`

- ``mossignal` Reconfiguration and Topology-Patch Specification > 2. Normative language` [44-69]
  Preview: The terms **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.
  Normative: MUST NOT 1, MUST 1, SHOULD NOT 1, SHOULD 1, MAY 1

- `Part I — Patch semantic model` [70-183]
  Preview: A topology patch is semantically a graph rewrite: where: - `G_base` is the complete stable-keyed base definition; - `G_target` is the complete stable-keyed target definition; - `C` is a partial one-to-one correspondence between surviving structural subjects.
  Symbols: `G_base`, `G_target`, `NetworkPatch<D>`, `Machine::prepare_patch`
  Normative: MUST NOT 3, MUST 3, MAY 1

- `Part I — Patch semantic model > 3. Patch as a declarative graph rewrite` [72-100]
  Preview: A topology patch is semantically a graph rewrite: where: - `G_base` is the complete stable-keyed base definition; - `G_target` is the complete stable-keyed target definition; - `C` is a partial one-to-one correspondence between surviving structural subjects.
  Symbols: `G_base`, `G_target`

- `Part I — Patch semantic model > 4. Declarative order independence` [101-125]
  Preview: Equivalent patches MUST produce equivalent preparation artifacts regardless of operation insertion order.
  Normative: MUST NOT 1, MUST 1

- `Part I — Patch semantic model > 5. Base binding` [126-148]
  Preview: Every `NetworkPatch<D>` is bound to: A patch MUST NOT be prepared against: - another network identity; - another base fingerprint; - another signal-semantics version; - another time domain.
  Symbols: `NetworkPatch<D>`, `Machine::prepare_patch`
  Normative: MUST NOT 1, MUST 1

- `Part I — Patch semantic model > 6. Topology revision and fingerprint` [149-167]
  Preview: A committed effective patch advances the machine-local topology revision exactly once.
  Normative: MUST NOT 1, MUST 1

- `Part I — Patch semantic model > 7. No implicit rebasing or merging` [168-183]
  Preview: The initial core does not automatically: - rebase a patch onto a different topology revision; - merge independently authored patches; - infer conflict resolution between patches; - compose prepared patches across an intervening topology revision; - infer stable correspondence from names or structural similarity.
  Normative: MAY 1

- `Part II — Structural correspondence and identity` [184-311]
  Preview: When the same stable key exists in base and target with the same structural category and compatible signal kind, it is an ordinary candidate for preservation.
  Symbols: `Level`, `NodeKey`, `ConnectionKey`, `Pulse`, `Select`, `when_high`
  Normative: MUST NOT 2, MUST 2

- `Part II — Structural correspondence and identity > 8. Ordinary stable-key preservation` [186-203]
  Preview: When the same stable key exists in base and target with the same structural category and compatible signal kind, it is an ordinary candidate for preservation.
  Symbols: `NodeKey`, `ConnectionKey`, `Level`, `Pulse`
  Normative: MUST NOT 1

- `Part II — Structural correspondence and identity > 9. Explicit reassociation` [204-252]
  Preview: A patch may explicitly associate one removed base subject with one new target subject of a compatible category.
  Normative: MUST 1

- `Part II — Structural correspondence and identity > 10. Correspondence constraints` [253-272]
  Preview: The complete correspondence relation MUST be a partial injection in both directions.
  Normative: MUST 1

- `Part II — Structural correspondence and identity > 11. Port correspondence` [273-293]
  Preview: Port correspondence is based on: 1.
  Symbols: `Select`, `when_high`, `Level`

- `Part II — Structural correspondence and identity > 12. Module-qualified internal identity` [294-311]
  Preview: For module internals, the logical structural identity is: A compatible module-instance replacement preserves an internal subject when: - the module instance survives or is explicitly reassociated; - the internal stable key survives; - the internal subject category and kind remain compatible.
  Normative: MUST NOT 1

- `Part III — Canonical patch-operation language` [312-668]
  Preview: The canonical operation language is a closed owned value family broadly equivalent to: Exact private representation may differ, but every operation family and its semantics are required.
  Symbols: `Level`, `Pulse`, `ReplaceNode`, `ModuleBindingSet`, `ModuleNodeMigrationDirective`, `ModuleInternalReassociation`, `NodeMigrationDirective<D>`, `AddNode`, `NodeDef<D>`, `RemoveNode`, `NodeKey`, `AddConnection`, `ConnectionDef`, `RemoveConnection`, `ReplaceConnection`, `ConnectionKey`, `InputDelta`, `InputSnapshot`, `ReplaceExternalInput`, `LevelEstablished`, `ReplaceExternalOutput`, `finish`, `ModuleInstanceKey`, `SetParent`, `SetDiagnosticMeta`, `Low`
  Normative: MUST NOT 6, MUST 13, MAY 2

- `Part III — Canonical patch-operation language > 13. Required operation enum` [314-418]
  Preview: The canonical operation language is a closed owned value family broadly equivalent to: Exact private representation may differ, but every operation family and its semantics are required.
  Symbols: `ModuleBindingSet`, `ModuleNodeMigrationDirective`, `ModuleInternalReassociation`, `NodeMigrationDirective<D>`
  Normative: MUST NOT 1, MUST 1

- `Part III — Canonical patch-operation language > 14. Node operations` [419-462]
  Preview: `AddNode` introduces one complete `NodeDef<D>` whose node key does not exist in the base topology unless it is the target of an explicit valid reassociation.
  Symbols: `ReplaceNode`, `AddNode`, `NodeDef<D>`, `RemoveNode`, `NodeKey`
  Normative: MUST 2

- `Part III — Canonical patch-operation language > 14. Node operations > 14.1 `AddNode`` [421-434]
  Preview: `AddNode` introduces one complete `NodeDef<D>` whose node key does not exist in the base topology unless it is the target of an explicit valid reassociation.
  Symbols: `AddNode`, `NodeDef<D>`

- `Part III — Canonical patch-operation language > 14. Node operations > 14.2 `RemoveNode`` [435-444]
  Preview: `RemoveNode` removes one base node.
  Symbols: `RemoveNode`
  Normative: MUST 1

- `Part III — Canonical patch-operation language > 14. Node operations > 14.3 `ReplaceNode`` [445-462]
  Preview: `ReplaceNode` supplies the complete target definition for an existing node key.
  Symbols: `ReplaceNode`, `NodeKey`
  Normative: MUST 1

- `Part III — Canonical patch-operation language > 15. Connection operations` [463-494]
  Preview: `AddConnection` introduces one complete stable-keyed `ConnectionDef`.
  Symbols: `AddConnection`, `ConnectionDef`, `RemoveConnection`, `ReplaceConnection`, `ConnectionKey`
  Normative: MUST 1

- `Part III — Canonical patch-operation language > 15. Connection operations > 15.1 `AddConnection`` [465-478]
  Preview: `AddConnection` introduces one complete stable-keyed `ConnectionDef`.
  Symbols: `AddConnection`, `ConnectionDef`

- `Part III — Canonical patch-operation language > 15. Connection operations > 15.2 `RemoveConnection`` [479-484]
  Preview: `RemoveConnection` removes one existing connection.
  Symbols: `RemoveConnection`

- `Part III — Canonical patch-operation language > 15. Connection operations > 15.3 `ReplaceConnection`` [485-494]
  Preview: `ReplaceConnection` preserves the connection key while replacing its complete definition, including endpoints or metadata.
  Symbols: `ReplaceConnection`, `ConnectionKey`
  Normative: MUST 1

- `Part III — Canonical patch-operation language > 16. External input operations` [495-524]
  Preview: A new external `Level` input creates a target-schema establishment obligation.
  Symbols: `Level`, `Pulse`, `InputDelta`, `InputSnapshot`, `ReplaceExternalInput`
  Normative: MUST 3

- `Part III — Canonical patch-operation language > 16. External input operations > 16.1 `AddExternalInput`` [497-506]
  Preview: A new external `Level` input creates a target-schema establishment obligation.
  Symbols: `Level`, `InputDelta`, `InputSnapshot`, `Pulse`
  Normative: MUST 1

- `Part III — Canonical patch-operation language > 16. External input operations > 16.2 `RemoveExternalInput`` [507-514]
  Preview: A removed external input is absent from the target input schema.
  Symbols: `Level`

- `Part III — Canonical patch-operation language > 16. External input operations > 16.3 `ReplaceExternalInput`` [515-524]
  Preview: `ReplaceExternalInput` preserves an endpoint key while replacing its complete definition.
  Symbols: `Level`, `Pulse`, `ReplaceExternalInput`
  Normative: MUST 2

- `Part III — Canonical patch-operation language > 17. External output operations` [525-550]
  Preview: A new external output must refer to a valid target signal source.
  Symbols: `Level`, `LevelEstablished`, `Pulse`, `ReplaceExternalOutput`
  Normative: MUST 1

- `Part III — Canonical patch-operation language > 17. External output operations > 17.1 `AddExternalOutput`` [527-534]
  Preview: A new external output must refer to a valid target signal source.
  Symbols: `Level`, `LevelEstablished`, `Pulse`

- `Part III — Canonical patch-operation language > 17. External output operations > 17.2 `RemoveExternalOutput`` [535-540]
  Preview: Removing an output produces a topology consequence, not a fabricated signal event.
  Symbols: `Level`

- `Part III — Canonical patch-operation language > 17. External output operations > 17.3 `ReplaceExternalOutput`` [541-550]
  Preview: `ReplaceExternalOutput` preserves the output key while replacing its source or metadata.
  Symbols: `ReplaceExternalOutput`, `Level`
  Normative: MUST 1

- `Part III — Canonical patch-operation language > 18. Module-instance operations` [551-591]
  Preview: A module instance operation introduces: - the instance key; - referenced validated module definition or fingerprint-bound module artifact; - input bindings; - hierarchy parent; - diagnostic metadata; - expanded internal structural subjects.
  Symbols: `finish`, `ModuleInstanceKey`
  Normative: MUST 2, MAY 1

- `Part III — Canonical patch-operation language > 18. Module-instance operations > 18.1 `AddModuleInstance`` [553-565]
  Preview: A module instance operation introduces: - the instance key; - referenced validated module definition or fingerprint-bound module artifact; - input bindings; - hierarchy parent; - diagnostic metadata; - expanded internal structural subjects.

- `Part III — Canonical patch-operation language > 18. Module-instance operations > 18.2 `RemoveModuleInstance`` [566-573]
  Preview: Removing a module instance does not implicitly remove descendants, external connections, or exported endpoint use in the canonical patch model.
  Symbols: `finish`
  Normative: MAY 1

- `Part III — Canonical patch-operation language > 18. Module-instance operations > 18.3 `ReplaceModuleInstance`` [574-591]
  Preview: The replacement definition MUST use the same `ModuleInstanceKey` as the operation subject.
  Symbols: `ModuleInstanceKey`
  Normative: MUST 2

- `Part III — Canonical patch-operation language > 19. Hierarchy operations` [592-616]
  Preview: `SetParent` changes the parent module instance of a hierarchical subject.
  Symbols: `SetParent`
  Normative: MUST 1

- `Part III — Canonical patch-operation language > 20. Metadata operations` [617-631]
  Preview: `SetDiagnosticMeta` replaces the complete diagnostic metadata value for one surviving subject.
  Symbols: `SetDiagnosticMeta`
  Normative: MUST NOT 4

- `Part III — Canonical patch-operation language > 21. Convenience operations` [632-650]
  Preview: Builder APIs MAY expose conveniences such as: Every convenience MUST expand deterministically into the canonical operation language before patch completion.
  Normative: MUST 2, MAY 1

- `Part III — Canonical patch-operation language > 22. Forbidden implicit edits` [651-668]
  Preview: The core MUST NOT silently: - remove incident connections when a node or port is removed; - choose a new driver for an input; - reconnect by matching names; - preserve state across changed keys without explicit reassociation; - reset a node because migration was inconvenient; - cancel pending work because an event structure was difficult to migrate; - establish a new external level input as `Low`; - synthesize pulses to represent topology change; - infer a temporal rescheduling policy; - reparent orphaned hierarchy automatically; - mutate caller-owned binding sets.
  Symbols: `Low`
  Normative: MUST NOT 1

- `Part IV — Patch normalization and structural preparation` [669-914]
  Preview: A patch builder is created from one compiled topology and explicit base revision: A machine convenience supplies its current revision: `NetworkPatchBuilder<D>` owns its operations and MUST NOT borrow the machine after construction.
  Symbols: `Level`, `finish`, `NetworkPatchBuilder<D>`, `PreparedPatch`, `PreparedPatch<D>`, `set`, `establish`, `InputSnapshot`
  Normative: MUST NOT 6, MUST 5, SHOULD 4, MAY 1

- `Part IV — Patch normalization and structural preparation > 23. Patch construction` [671-690]
  Preview: A patch builder is created from one compiled topology and explicit base revision: A machine convenience supplies its current revision: `NetworkPatchBuilder<D>` owns its operations and MUST NOT borrow the machine after construction.
  Symbols: `NetworkPatchBuilder<D>`
  Normative: MUST NOT 1

- `Part IV — Patch normalization and structural preparation > 24. Immediate builder checks` [691-706]
  Preview: The builder SHOULD reject immediately detectable contradictions, including: - duplicate non-identical additions of one key; - two conflicting replacements of one subject; - removal and replacement of the same subject; - metadata assignment to a subject also removed; - contradictory parent assignments; - contradictory reassociations; - reassociation kind or direction mismatch visible from operation data; - a replacement definition whose key differs from the replacement subject; - an operation referencing another network’s typed artifact where detectable.
  Normative: SHOULD 1, MAY 1

- `Part IV — Patch normalization and structural preparation > 25. Normalization` [707-725]
  Preview: Patch completion MUST produce a deterministically normalized operation set.
  Symbols: `finish`
  Normative: MUST NOT 1, MUST 2

- `Part IV — Patch normalization and structural preparation > 26. Target construction` [726-737]
  Preview: Structural preparation constructs the target definition as a pure deterministic function: The rewrite is all-at-once.

- `Part IV — Patch normalization and structural preparation > 27. Dangling and incidence condition` [738-757]
  Preview: The target graph must contain no dangling structure.
  Normative: SHOULD 1

- `Part IV — Patch normalization and structural preparation > 28. Structural validation` [758-778]
  Preview: Preparation MUST perform the complete validation required for an independently authored network, including: - stable-key uniqueness; - node and endpoint existence; - kind compatibility; - direction; - fixed and variadic arity; - driver rules; - module-interface validity; - hierarchy acyclicity; - semantic parameter validity; - initial-state validity; - state-schema validity; - current-reaction dependency construction; - SCC-based reaction-cycle detection; - target fingerprint construction.
  Symbols: `PreparedPatch`
  Normative: MUST NOT 1, MUST 1

- `Part IV — Patch normalization and structural preparation > 29. `PreparedPatch<D>`` [779-810]
  Preview: Successful preparation produces an immutable artifact: It contains at least: It MUST NOT contain exact runtime migration outcomes that depend on the machine state at the future effective time.
  Symbols: `PreparedPatch<D>`
  Normative: MUST NOT 1, SHOULD 1

- `Part IV — Patch normalization and structural preparation > 30. Static migration plan` [811-832]
  Preview: The prepared static plan MUST classify every relevant structural subject.
  Normative: MUST 1

- `Part IV — Patch normalization and structural preparation > 31. Preparation is state-independent` [833-850]
  Preview: Structural preparation MUST NOT read or depend on: - current logical time; - current external level valuation; - current node state; - current temporal state; - actual pending events; - current output baselines; - active diagnostic episodes; - current provenance roots; - execution-state digest.
  Normative: MUST NOT 1

- `Part IV — Patch normalization and structural preparation > 32. Prepared-patch freshness` [851-870]
  Preview: State-only transactions do not invalidate a prepared patch.

- `Part IV — Patch normalization and structural preparation > 33. Target-bound input schema` [871-884]
  Preview: Preparation derives a target input schema.
  Symbols: `Level`, `set`, `establish`, `InputSnapshot`

- `Part IV — Patch normalization and structural preparation > 34. Invalidated artifacts` [885-904]
  Preview: Preparation MUST report revision-bound artifacts made stale by commitment, including categories such as: Invalidation is not semantic state loss.
  Normative: MUST 1, SHOULD 1

- `Part IV — Patch normalization and structural preparation > 35. Region analysis` [905-914]
  Preview: Preparation derives region merges and splits from the complete target structural graph.
  Normative: MUST NOT 1

- `Part V — State compatibility and migration` [915-1496]
  Preview: Every surviving stateful or temporal node receives exactly one finalized outcome: The four semantic classes are: They are mutually exclusive and collectively exhaustive for every surviving state-owning subject.
  Symbols: `Standard`, `Reset`, `TransferStoredLevel`, `LogicLevel`, `enable`, `High`, `Immediate`, `RequirePreserve`, `Preserve`, `Cancel`, `EdgeInitialization`, `PendingEventKey`, `new_deadline`, `MatureAtPatchTime`, `PreserveDeadlines`, `CancelPending`, `RejectIfPending`, `InertialDelayMigration`, `PeriodicMigration`, `AfterFirstPeriod`, `T + target period`, `Low`, `PreservePhase`, `RestartPhase`
  Normative: SHOULD 1

- `Part V — State compatibility and migration > 36. Compatibility outcomes` [917-949]
  Preview: Every surviving stateful or temporal node receives exactly one finalized outcome: The four semantic classes are: They are mutually exclusive and collectively exhaustive for every surviving state-owning subject.

- `Part V — State compatibility and migration > 37. Migration directives` [950-977]
  Preview: Representative public shape: `Standard` selects only the built-in standard rule defined by this specification and the built-in node specification.
  Symbols: `Standard`, `RequirePreserve`, `Preserve`, `Reset`

- `Part V — State compatibility and migration > 38. New nodes` [978-991]
  Preview: A genuinely new node has no migration predecessor.

- `Part V — State compatibility and migration > 39. Removed nodes` [992-1001]
  Preview: A removed stateless node loses no internal state.
  Symbols: `Cancel`

- `Part V — State compatibility and migration > 40. Uninitialized-machine state` [1002-1016]
  Preview: An uninitialized machine still owns declared initial node and temporal state.
  Symbols: `Reset`

- `Part V — State compatibility and migration > 41. Derived current port values` [1017-1026]
  Preview: Current level-port values are derived runtime facts, not independently migratable authored state.

- `Part V — State compatibility and migration > 42. Standard built-in compatibility matrix` [1027-1071]
  Preview: The standard rule is determined by the source and target node families: | Source and target relation | Standard state result | |---|---| | Same stateless built-in kind | No state; ordinary reevaluation | | Different stateless built-in kinds | No state; allowed if target structure validates | | Same edge-detector kind | Preserve remembered observation state | | Different edge-detector kinds | Migrate remembered observation state | | Same boolean-state kind | Preserve stored level | | Different boolean-state kinds | Reject unless `TransferStoredLevel` or `Reset` is explicit | | Same temporal kind, no migration-relevant parameter change | Preserve temporal state and apply standard pending-work rule | | Same temporal kind, migration-relevant parameter change | Use node-specific standard or explicit temporal policy | | Different temporal kinds | Reject unless a separately enumerated migration exists; initial version provides none | | Stateful or temporal to stateless | Reset target/no target state; source state and work are reported loss | | Stateless to stateful or temporal | Initialize target state; no source state loss | | Unrelated stateful families | Reject unless reset is explicit | The boolean-state family is: The edge-detector family is: The temporal family members remain distinct:
  Symbols: `TransferStoredLevel`, `Reset`

- `Part V — State compatibility and migration > 43. Combinational nodes` [1072-1086]
  Preview: Combinational nodes own no semantic state and no pending work.

- `Part V — State compatibility and migration > 44. Edge detectors` [1087-1106]
  Preview: An edge detector state is: For same-kind or cross-kind migration within the edge-detector family: - an established remembered level is preserved; - an unestablished baseline remains unestablished; - changing `EdgeInitialization` does not retroactively alter preserved state; - `Reset` re-enters the target initialization policy; - target reaction evaluation compares the migrated previous observation with the settled target input and may emit according to the target detector kind.
  Symbols: `EdgeInitialization`, `Reset`

- `Part V — State compatibility and migration > 45. Boolean-state nodes` [1107-1126]
  Preview: For the same boolean-state kind: - stored `LogicLevel` is preserved; - changing an initial-level parameter does not alter preserved state; - connection changes do not alter stored state directly; - the patch-time reaction applies current target controls to the preserved previous state; - changing latch conflict policy preserves the stored level and reevaluates diagnostic-episode semantics.
  Symbols: `LogicLevel`, `TransferStoredLevel`

- `Part V — State compatibility and migration > 46. Temporal-state common rules` [1127-1180]
  Preview: Temporal migration distinguishes: Preparation defines the rule.
  Symbols: `PendingEventKey`
  Normative: SHOULD 1

- `Part V — State compatibility and migration > 46. Temporal-state common rules > 46.1 Pending-event identity` [1171-1180]
  Preview: A one-to-one migration that preserves, recomputes, or transforms one obligation SHOULD preserve its `PendingEventKey`.
  Symbols: `PendingEventKey`
  Normative: SHOULD 1

- `Part V — State compatibility and migration > 47. Deadline recomputation and overdue policy` [1181-1210]
  Preview: A migration rule that computes a target deadline from an original event origin uses: Checked overflow rejects the patch transaction atomically.
  Symbols: `new_deadline`, `MatureAtPatchTime`

- `Part V — State compatibility and migration > 48. `PulseDelay` migration` [1211-1265]
  Preview: The migration enum is: Changing delay under `Standard` uses `PreserveDeadlines`: - existing pulse groups retain deadlines; - existing multiplicities and causal contributors are preserved; - the target delay applies only to pulse input received at or after the patch-time reaction.
  Symbols: `Standard`, `PreserveDeadlines`

- `Part V — State compatibility and migration > 48. `PulseDelay` migration > 48.1 Standard rule` [1228-1235]
  Preview: Changing delay under `Standard` uses `PreserveDeadlines`: - existing pulse groups retain deadlines; - existing multiplicities and causal contributors are preserved; - the target delay applies only to pulse input received at or after the patch-time reaction.
  Symbols: `Standard`, `PreserveDeadlines`

- `Part V — State compatibility and migration > 48. `PulseDelay` migration > 48.2 `RecomputeFromOrigin`` [1236-1245]
  Preview: Each group receives deadline: Multiplicity and causal contributors are preserved.

- `Part V — State compatibility and migration > 48. `PulseDelay` migration > 48.3 `RestartFromPatchTime`` [1246-1257]
  Preview: Each group receives deadline: Its original pulse cause remains in provenance, while the patch becomes the cause of rescheduling.

- `Part V — State compatibility and migration > 48. `PulseDelay` migration > 48.4 `CancelPending`` [1258-1261]
  Preview: Every actual pending group is canceled and reported as semantic loss.

- `Part V — State compatibility and migration > 48. `PulseDelay` migration > 48.5 `RejectIfPending`` [1262-1265]
  Preview: Finalization rejects if any actual pending group exists.

- `Part V — State compatibility and migration > 49. `TransportDelay` migration` [1266-1312]
  Preview: The migration enum is: The remembered input and current output level are preserved for same-kind survival unless reset is explicit.
  Symbols: `Standard`, `CancelPending`, `RejectIfPending`

- `Part V — State compatibility and migration > 49. `TransportDelay` migration > 49.1 Standard rule` [1285-1288]
  Preview: Changing delay under `Standard` preserves every queued transition deadline.
  Symbols: `Standard`

- `Part V — State compatibility and migration > 49. `TransportDelay` migration > 49.2 Recomputed or restarted queues` [1289-1300]
  Preview: Every queued transition retains: - target level; - originating logical time; - originating causal support.

- `Part V — State compatibility and migration > 49. `TransportDelay` migration > 49.3 Connectivity changes` [1301-1306]
  Preview: Changing input connectivity does not cancel compatible queued transitions.

- `Part V — State compatibility and migration > 49. `TransportDelay` migration > 49.4 Cancellation and rejection` [1307-1312]
  Preview: `CancelPending` reports every canceled transition as semantic loss.
  Symbols: `CancelPending`, `RejectIfPending`

- `Part V — State compatibility and migration > 50. `InertialDelay` migration` [1313-1381]
  Preview: The migration enum is: The remembered input and current output level are preserved for same-kind survival unless reset is explicit.
  Symbols: `Standard`, `InertialDelayMigration`

- `Part V — State compatibility and migration > 50. `InertialDelay` migration > 50.1 Standard rule` [1332-1337]
  Preview: When delay changes while no candidate exists, `Standard` preserves the empty candidate state and applies the target delay to future candidates.
  Symbols: `Standard`, `InertialDelayMigration`

- `Part V — State compatibility and migration > 50. `InertialDelay` migration > 50.2 `PreserveDeadline`` [1338-1348]
  Preview: The candidate retains: - target level; - qualification origin; - deadline; - causal ancestry.

- `Part V — State compatibility and migration > 50. `InertialDelay` migration > 50.3 `RecomputeFromOrigin`` [1349-1358]
  Preview: The candidate deadline becomes: The original qualification interval remains semantically credited.

- `Part V — State compatibility and migration > 50. `InertialDelay` migration > 50.4 `RestartFromPatchTime`` [1359-1371]
  Preview: The candidate target remains unchanged, but: The original input cause remains causal ancestry, and the patch records that qualification restarted.

- `Part V — State compatibility and migration > 50. `InertialDelay` migration > 50.5 `CancelCandidate`` [1372-1377]
  Preview: The candidate is canceled and reported as semantic loss.

- `Part V — State compatibility and migration > 50. `InertialDelay` migration > 50.6 `RejectIfCandidate`` [1378-1381]
  Preview: Finalization rejects if a candidate exists.

- `Part V — State compatibility and migration > 51. `Periodic` migration` [1382-1475]
  Preview: The migration enum is: The state components are: If period, first-emission policy, and re-enable phase policy are unchanged, `Standard` preserves all temporal state and pending boundaries.
  Symbols: `enable`, `High`, `Immediate`, `Standard`, `PeriodicMigration`, `AfterFirstPeriod`, `T + target period`, `Low`, `PreservePhase`, `RestartPhase`

- `Part V — State compatibility and migration > 51. `Periodic` migration > 51.1 Standard rule` [1405-1412]
  Preview: If period, first-emission policy, and re-enable phase policy are unchanged, `Standard` preserves all temporal state and pending boundaries.
  Symbols: `Standard`, `PeriodicMigration`

- `Part V — State compatibility and migration > 51. `Periodic` migration > 51.2 `PreserveNextDeadline`` [1413-1429]
  Preview: If a next eligible deadline exists: - that deadline is preserved; - a boundary due exactly at `T` joins the patch-time due batch; - after that preserved boundary, cadence proceeds using the target period; - the preserved deadline becomes the phase reference for subsequent target cadence.

- `Part V — State compatibility and migration > 51. `Periodic` migration > 51.3 `RecomputeFromExistingAnchor`` [1430-1447]
  Preview: The existing anchor is preserved.
  Symbols: `enable`

- `Part V — State compatibility and migration > 51. `Periodic` migration > 51.4 `ReanchorAtPatchTime`` [1448-1461]
  Preview: The target anchor becomes `T`.
  Symbols: `High`, `Immediate`, `AfterFirstPeriod`, `T + target period`, `Low`, `PreservePhase`, `RestartPhase`

- `Part V — State compatibility and migration > 51. `Periodic` migration > 51.5 `CancelSchedule`` [1462-1471]
  Preview: Old anchor, next boundary, and previous-enabled scheduling state are cleared.
  Symbols: `enable`, `Immediate`

- `Part V — State compatibility and migration > 51. `Periodic` migration > 51.6 `RejectIfAnchored`` [1472-1475]
  Preview: Finalization rejects if an anchor or pending boundary exists.

- `Part V — State compatibility and migration > 52. Cross-kind temporal migration` [1476-1496]
  Preview: The initial core defines no lossless migration between different temporal node kinds.
  Symbols: `Reset`

- `Part VI — Other semantic state` [1497-1586]
  Preview: For a ready machine: - a preserved external `Level` input retains its authoritative value; - a reassociated external `Level` input receives the source authoritative value; - a new external `Level` input requires explicit target input establishment; - a removed external `Level` input loses its authoritative value; - external `Pulse` inputs have no persistent valuation to migrate.
  Symbols: `Level`, `Pulse`, `set`, `establish`, `LevelEstablished`, `LevelSetResetLatch`, `RetainAndDiagnose`
  Normative: SHOULD 1

- `Part VI — Other semantic state > 53. External input valuation migration` [1499-1512]
  Preview: For a ready machine: - a preserved external `Level` input retains its authoritative value; - a reassociated external `Level` input receives the source authoritative value; - a new external `Level` input requires explicit target input establishment; - a removed external `Level` input loses its authoritative value; - external `Pulse` inputs have no persistent valuation to migrate.
  Symbols: `Level`, `Pulse`, `set`, `establish`

- `Part VI — Other semantic state > 54. External output baseline migration` [1513-1526]
  Preview: For a ready machine: - a preserved `Level` output retains its prior published baseline; - a reassociated same-kind `Level` output carries the source baseline as migration evidence and causal continuity, but the new structural output key begins without an externally published baseline; - a new `Level` output has no baseline; - a removed `Level` output loses its baseline; - `Pulse` outputs have no persistent value baseline.
  Symbols: `Level`, `Pulse`, `LevelEstablished`

- `Part VI — Other semantic state > 55. Diagnostic episode migration` [1527-1554]
  Preview: Every active diagnostic episode receives one outcome: An episode may be preserved only when: - its owning subject survives or is reassociated; - its diagnostic code remains applicable; - its condition discriminator retains the same semantic identity; - its evidence can be translated without ambiguity.
  Symbols: `LevelSetResetLatch`, `RetainAndDiagnose`

- `Part VI — Other semantic state > 56. Provenance migration` [1555-1576]
  Preview: Migration is represented causally.
  Normative: SHOULD 1

- `Part VI — Other semantic state > 57. Current diagnostic metadata and paths` [1577-1586]
  Preview: Metadata and hierarchy changes may alter rendered diagnostic paths without changing the semantic identity of preserved runtime facts.

- `Part VII — Module replacement and hierarchy` [1587-1653]
  Preview: Module-instance operations are prepared by expanding module structure into the same stable-keyed semantic graph model used for direct nodes and connections.
  Normative: MUST 1

- `Part VII — Module replacement and hierarchy > 58. Module expansion model` [1589-1603]
  Preview: Module-instance operations are prepared by expanding module structure into the same stable-keyed semantic graph model used for direct nodes and connections.
  Normative: MUST 1

- `Part VII — Module replacement and hierarchy > 59. Compatible module revision` [1604-1619]
  Preview: A module-instance revision is compatible when every surviving internal subject can be classified under the ordinary rules.

- `Part VII — Module replacement and hierarchy > 60. Module interface changes` [1620-1631]
  Preview: When a module interface changes: - removed module inputs or outputs must have all external incidence explicitly removed or redirected; - new required module inputs must be bound explicitly; - signal-kind changes require remove/add rather than same-key replacement; - internal interface mapping must use stable module interface keys; - external connections are not inferred from names or positional order.

- `Part VII — Module replacement and hierarchy > 61. Nested modules` [1632-1645]
  Preview: Nested module replacement applies correspondence recursively through: Containment must remain acyclic.

- `Part VII — Module replacement and hierarchy > 62. Module metadata versus semantics` [1646-1653]
  Preview: Changing module diagnostic metadata or presentation path alone does not alter internal semantic fingerprint or migration compatibility.

- `Part VIII — Transaction-time finalization and commitment` [1654-1825]
  Preview: For a ready machine and patch effective at logical time `T`, the outer transaction MUST perform: 1.
  Symbols: `Level`, `T0`, `Pulse`, `InputSnapshot`, `set`, `enable`, `LevelChanged`, `LevelEstablished`, `TopologyChange`
  Normative: MUST NOT 3, MUST 1, MAY 1

- `Part VIII — Transaction-time finalization and commitment > 63. Effective-time ordering` [1656-1675]
  Preview: For a ready machine and patch effective at logical time `T`, the outer transaction MUST perform: 1.
  Normative: MUST 1

- `Part VIII — Transaction-time finalization and commitment > 64. Initialization patch ordering` [1676-1690]
  Preview: For an uninitialized machine and patch effective at initial time `T0`: 1.
  Symbols: `T0`, `InputSnapshot`

- `Part VIII — Transaction-time finalization and commitment > 65. State-dependent finalization` [1691-1706]
  Preview: Static conditional rules are evaluated against the state reached immediately before `T`.
  Normative: MUST NOT 1

- `Part VIII — Transaction-time finalization and commitment > 66. Same-time external input boundary` [1707-1720]
  Preview: Migration reads pre-`T` source state.
  Symbols: `set`, `enable`

- `Part VIII — Transaction-time finalization and commitment > 67. Events due exactly at patch time` [1721-1736]
  Preview: Every source event due exactly at `T` is first subject to migration under the prepared rule.
  Normative: MUST NOT 1

- `Part VIII — Transaction-time finalization and commitment > 68. Target topology installation` [1737-1752]
  Preview: Target topology installation in candidate state includes: - new compiled topology root; - target revision; - migrated state-family storage; - migrated event ownership and index; - target external input valuation layout; - target output-baseline layout; - migrated diagnostic episode ownership; - migrated provenance roots; - target graph and inspection indices.

- `Part VIII — Transaction-time finalization and commitment > 69. Topology-induced reevaluation` [1753-1772]
  Preview: After installation, every potentially affected target reaction operation must be reevaluated.
  Normative: MAY 1

- `Part VIII — Transaction-time finalization and commitment > 70. Topology as cause` [1773-1789]
  Preview: The patch is a first-class causal fact.
  Symbols: `Level`, `Pulse`
  Normative: MUST NOT 1

- `Part VIII — Transaction-time finalization and commitment > 71. Output publication` [1790-1802]
  Preview: After target settlement: - same-key preserved `Level` outputs compare target value against their prior published baseline; - changed preserved values emit `LevelChanged`; - unchanged preserved values emit no level event; - new and reassociated `Level` outputs emit `LevelEstablished`; - removed outputs produce `TopologyChange` entries; - `Pulse` outputs emit only for genuine nonzero target reaction pulse counts.
  Symbols: `Level`, `LevelChanged`, `LevelEstablished`, `TopologyChange`, `Pulse`

- `Part VIII — Transaction-time finalization and commitment > 72. Atomic failure` [1803-1825]
  Preview: Any failure during: - freshness validation; - earlier-deadline execution; - migration finalization; - checked time arithmetic; - state-loss enforcement; - target installation; - target reaction evaluation; - provenance construction; - diagnostic update; - result construction; - digest calculation; - runtime-budget checking; rejects the entire outer transaction.

- `Part IX — Semantic loss and migration reporting` [1826-1965]
  Preview: Semantic loss is the destruction, reset, truncation, or unrepresented disappearance of semantic information that existed immediately before the patch and could affect future execution or required observation.
  Symbols: `RejectStateLoss`, `AllowReportedStateLoss`, `MigrationReport<D>`
  Normative: MUST NOT 2, MUST 1

- `Part IX — Semantic loss and migration reporting > 73. Semantic-loss definition` [1828-1851]
  Preview: Semantic loss is the destruction, reset, truncation, or unrepresented disappearance of semantic information that existed immediately before the patch and could affect future execution or required observation.

- `Part IX — Semantic loss and migration reporting > 74. Changes that are not semantic loss` [1852-1869]
  Preview: The following are not by themselves semantic loss: - topology revision advancing; - dense-index reassignment; - stale resolved handles; - stale compiled inspection plans; - region merge or split; - metadata change; - deterministic memory-layout change; - event deadline change under an information-preserving explicit migration rule; - migration provenance introducing a checkpoint that preserves all required semantics; - target output changes produced by ordinary reevaluation; - introduction of new target initial state where no source state existed.

- `Part IX — Semantic loss and migration reporting > 75. Loss identity` [1870-1884]
  Preview: Every potential and actual loss has stable structured identity based on at least: Loss entries MUST NOT be deduplicated merely because rendered prose is equal.
  Normative: MUST NOT 1

- `Part IX — Semantic loss and migration reporting > 76. Reconfiguration policy` [1885-1906]
  Preview: Under `RejectStateLoss`: - any nonempty finalized semantic-loss set rejects the complete transaction; - potential loss alone does not reject if the runtime predicate is false; - no topology revision is committed.
  Symbols: `RejectStateLoss`, `AllowReportedStateLoss`

- `Part IX — Semantic loss and migration reporting > 77. Migration report` [1907-1933]
  Preview: A successful patch transaction returns one complete `MigrationReport<D>`.
  Symbols: `MigrationReport<D>`
  Normative: MUST 1

- `Part IX — Semantic loss and migration reporting > 78. Subject outcomes` [1934-1951]
  Preview: Every base and target structural subject receives one canonical structural outcome: A subject may have one primary existence outcome plus orthogonal metadata or hierarchy flags in the detailed record.

- `Part IX — Semantic loss and migration reporting > 79. Deterministic report order` [1952-1965]
  Preview: Migration records are ordered deterministically by: 1.
  Normative: MUST NOT 1

- `Part X — Public API responsibilities` [1966-2099]
  Preview: `NetworkPatch<D>` is an owned opaque value.
  Symbols: `NetworkPatch<D>`, `PreparedPatch<D>`, `with_patch`, `Machine::forecast`
  Normative: MUST NOT 1, MUST 2, SHOULD 2

- `Part X — Public API responsibilities > 80. Patch access` [1968-1984]
  Preview: `NetworkPatch<D>` is an owned opaque value.
  Symbols: `NetworkPatch<D>`
  Normative: SHOULD 1

- `Part X — Public API responsibilities > 81. Prepared-patch access` [1985-2001]
  Preview: `PreparedPatch<D>` exposes:
  Symbols: `PreparedPatch<D>`

- `Part X — Public API responsibilities > 82. Patch builder responsibilities` [2002-2038]
  Preview: The builder MUST provide methods for every canonical operation family.
  Normative: MUST 1

- `Part X — Public API responsibilities > 83. Preparation report` [2039-2050]
  Preview: Patch preparation returns: Independent structural diagnostics SHOULD be collected where safe.
  Normative: SHOULD 1

- `Part X — Public API responsibilities > 84. Transaction attachment` [2051-2076]
  Preview: A prepared patch commits only through a transaction: `with_patch` MUST validate where determinable: - base revision binding; - target input-schema binding; - lifecycle-compatible snapshot versus delta; - network identity; - time domain.
  Symbols: `with_patch`
  Normative: MUST 1

- `Part X — Public API responsibilities > 85. Exact forecast` [2077-2091]
  Preview: An exact future patch preview uses ordinary `Machine::forecast` with a patch-bearing transaction.
  Symbols: `Machine::forecast`
  Normative: MUST NOT 1

- `Part X — Public API responsibilities > 86. No implicit commit of forecast` [2092-2099]
  Preview: A forecast is not a reservation or prepared commit token.

- `Part XI — Diagnostics and failures` [2100-2181]
  Preview: Patch construction must distinguish at least: These are local construction failures, not runtime failures.
  Symbols: `ReconfigurationFailure`
  Normative: SHOULD 2

- `Part XI — Diagnostics and failures > 87. Patch-build failures` [2102-2116]
  Preview: Patch construction must distinguish at least: These are local construction failures, not runtime failures.

- `Part XI — Diagnostics and failures > 88. Preparation diagnostics` [2117-2144]
  Preview: Preparation diagnostics must cover at least: Diagnostics should identify stable subjects and graph paths where available.

- `Part XI — Diagnostics and failures > 89. Finalization failures` [2145-2165]
  Preview: Runtime reconfiguration failures must distinguish at least: Exact nested evidence types are defined by the diagnostic and failure catalogue.
  Symbols: `ReconfigurationFailure`

- `Part XI — Diagnostics and failures > 90. Diagnostic accumulation` [2166-2171]
  Preview: Preparation SHOULD continue after one independent error where safe so that callers receive a useful complete report.
  Normative: SHOULD 2

- `Part XI — Diagnostics and failures > 91. Diagnostic episode behavior` [2172-2181]
  Preview: Preparation warnings are occurrences, not machine diagnostic episodes.

- `Part XII — Verification obligations` [2182-2346]
  Preview: The verification suite must retain an independent correctness-oriented reference path that: 1.
  Symbols: `RejectStateLoss`, `AllowReportedStateLoss`, `set`
  Normative: SHOULD 1

- `Part XII — Verification obligations > 92. Reference reconfiguration path` [2184-2195]
  Preview: The verification suite must retain an independent correctness-oriented reference path that: 1.

- `Part XII — Verification obligations > 93. Operation-order invariance` [2196-2205]
  Preview: Tests must permute equivalent operation insertion order and compare: - normalized patch; - target fingerprint; - preparation diagnostics; - static migration plan; - final committed machine and result.

- `Part XII — Verification obligations > 94. Preparation state independence` [2206-2211]
  Preview: For one base topology and patch, preparation performed against machines with different runtime states but the same topology revision must produce equivalent prepared artifacts.

- `Part XII — Verification obligations > 95. Effective-time state correctness` [2212-2220]
  Preview: A required scenario is: 1.

- `Part XII — Verification obligations > 96. Classification totality` [2221-2232]
  Preview: For every successful finalization: - every surviving state-owning subject has exactly one state outcome; - every source pending event has exactly one event outcome; - every external level valuation has one preservation, reassociation, removal, or addition outcome; - every external level output baseline has one outcome; - every active diagnostic episode has one outcome; - every required provenance root has one outcome; - every actual semantic loss appears exactly once.

- `Part XII — Verification obligations > 97. State-loss policy tests` [2233-2243]
  Preview: Tests must establish: - any actual loss rejects under `RejectStateLoss`; - potential but unrealized loss does not reject; - every actual loss is reported under `AllowReportedStateLoss`; - dense-handle invalidation is not mislabeled as state loss; - reset counts as loss only where source semantic state or progress is discarded; - initializing state where no source state existed is not loss.
  Symbols: `RejectStateLoss`, `AllowReportedStateLoss`

- `Part XII — Verification obligations > 98. Temporal boundary tests` [2244-2264]
  Preview: Every temporal migration policy must be tested for source deadlines: Where recomputation is supported, tests must additionally cover target deadlines: Equal-time event order must be permuted without changing semantics.

- `Part XII — Verification obligations > 99. Target input-schema tests` [2265-2277]
  Preview: Tests must cover: - preserved external level input inherited; - preserved input changed with `set`; - reassociated input valuation transferred; - new level input established; - missing establishment rejected; - removed input reference rejected; - pulse input absence interpreted as zero; - initialization patch requiring complete target snapshot.
  Symbols: `set`

- `Part XII — Verification obligations > 100. Topology-induced reaction equivalence` [2278-2292]
  Preview: The production invalidation set after patch installation must be differentially compared with complete target-graph evaluation.

- `Part XII — Verification obligations > 101. Module migration tests` [2293-2304]
  Preview: Generated and focused tests must cover: - module internal node preservation; - internal key removal and addition; - module interface addition and removal; - nested instance replacement; - hierarchy move without state reset; - pending event preservation inside replaced module; - state loss rejection inside one affected instance rejecting the entire patch.

- `Part XII — Verification obligations > 102. Failure atomicity` [2305-2320]
  Preview: Every preparation-time or runtime rejection category must preserve the complete published machine.

- `Part XII — Verification obligations > 103. Stable identity and dense reuse` [2321-2331]
  Preview: Tests must verify: - equal stable keys preserve identity only when category and kind permit; - explicit reassociation is one-to-one; - old reassociated keys become unavailable; - dense-slot reuse cannot transfer state or episodes to another subject; - stale handles and plans fail structurally; - stable keys can be resolved again against the target revision.

- `Part XII — Verification obligations > 104. Bounded exhaustive exploration` [2332-2346]
  Preview: For small generated networks and patches, the harness SHOULD enumerate: - all small valid operation subsets; - all operation orders; - small state valuations; - empty and nonempty pending-event sets; - both state-loss policies; - relevant temporal migration choices.
  Normative: SHOULD 1

- `Part XIII — Deliberately unspecified and excluded behavior` [2347-2420]
  Preview: This specification does not mandate: - patch storage container; - persistent versus copied target graph construction; - dense state-layout migration algorithm; - event-arena implementation; - incremental compilation strategy; - cache invalidation representation; - allocation strategy.

- `Part XIII — Deliberately unspecified and excluded behavior > 105. Internal representation freedom` [2349-2362]
  Preview: This specification does not mandate: - patch storage container; - persistent versus copied target graph construction; - dense state-layout migration algorithm; - event-arena implementation; - incremental compilation strategy; - cache invalidation representation; - allocation strategy.

- `Part XIII — Deliberately unspecified and excluded behavior > 106. No arbitrary migration code` [2363-2377]
  Preview: The initial core does not accept caller-defined migration functions.

- `Part XIII — Deliberately unspecified and excluded behavior > 107. No automatic structural matching` [2378-2393]
  Preview: The core does not infer continuity from: - names; - descriptions; - diagnostic paths; - node position; - graph similarity; - connection neighborhood; - insertion order; - dense index; - module display order.

- `Part XIII — Deliberately unspecified and excluded behavior > 108. No general patch merge` [2394-2399]
  Preview: Collaborative editor merge, conflict-free replicated patches, three-way graph merge, and optimistic patch rebasing are outside this specification.

- `Part XIII — Deliberately unspecified and excluded behavior > 109. No automatic inverse patch` [2400-2412]
  Preview: A structural inverse may be derivable for some patches, but semantic rollback is not generally automatic because the original transaction may have: - discarded state; - canceled events; - advanced logical time; - emitted outputs; - transformed provenance; - changed external authoritative input.

- `Part XIII — Deliberately unspecified and excluded behavior > 110. Persistence boundary` [2413-2420]
  Preview: This specification defines the semantic content of patch and migration artifacts but not their canonical wire encoding.

- `Part XIV — Required guarantees` [2421-2463]
  Preview: The implementation MUST provide: This document refines the existing shared model: - the API and semantics specification remains authoritative for observable machine behavior; - the built-in node specification remains authoritative for ordinary node laws and standard compatibility; - the processor architecture remains authoritative for internal transaction ordering and atomic publication; - the concrete Rust API surface remains authoritative for general ownership and type organization, except where it explicitly deferred the exhaustive patch language to this document; - the testing and verification policy remains authoritative for CI and release gates.
  Normative: MUST 1

- `Part XIV — Required guarantees > 111. Patch-language guarantees` [2423-2449]
  Preview: The implementation MUST provide:
  Normative: MUST 1

- `Part XIV — Required guarantees > 112. Coherence with the other specifications` [2450-2463]
  Preview: This document refines the existing shared model: - the API and semantics specification remains authoritative for observable machine behavior; - the built-in node specification remains authoritative for ordinary node laws and standard compatibility; - the processor architecture remains authoritative for internal transaction ordering and atomic publication; - the concrete Rust API surface remains authoritative for general ownership and type organization, except where it explicitly deferred the exhaustive patch language to this document; - the testing and verification policy remains authoritative for CI and release gates.

- `Summary` [2464-2472]
  Preview: A `mossignal` topology patch is a declarative, stable-keyed graph rewrite rather than an ordered mutation script.
  Symbols: `mossignal`
