# Ordinary cause-reference remapping preparation

Planning evidence for one native implementation bead; this document is not a
normative specification. Stop before implementation and submit the bead to the
coordinator for independent planning acceptance.

## Baseline and readiness

- Repository: `/Users/mikkofinell/code/mossignal`, branch `main`.
- Accepted HEAD: `bdf70ae6c793e2872e11fe652280cf2a01d1990d`
  (`ms-49a.2: Separate edge comparisons from successor observation provenance`).
- Initial working tree was clean; `main` was two commits ahead of `origin/main`.
  `bv --robot-triage` and `br ready --json` found no open or in-progress work.
- Predecessor `ms-49a.2` is closed and accepted. This is a separately assigned
  continuation slice, not a retired roadmap item.
- Generated implementation bead: `ms-efv`, open and unassigned, depending only
  on the already closed `ms-49a.2`. Its description is the bounded task brief.
- The scope has no conflicting requirements, missing fundamental semantics, or
  frozen compatibility obstruction. Independent coordinator planning review
  accepted this scope on 2026-10-10; implementation still requires assignment.
- Planning authors use `br --no-auto-flush`; the independent reviewer owns
  export, acceptance and commit. No code, test, specification or vector edits
  are part of preparation.

Independent acceptance checked the complete planning document and bead against
the actual fork/membership, ordinary settlement, current-root publication,
standard-role and checkpoint/restoration paths. No scope correction or contract
change was needed. All eleven reviewed contracts' 369 cited source sections were
independently confirmed unchanged. The verification design separates obsolete
cause rewriting from necessary copying and real translation, documents the
clone-reference limitation, and preserves exact vectors and logical budgets.
Planning validation includes contract tooling, bead reference checks, whitespace
and diff checks. Implementation tests and gates remain future obligations.

## Reused contract basis

All paths below are under `docs/specs/contracts/`. All eleven records remain
`reviewed` and unchanged. `scripts/contracts.py status` resolved 369 source
citations as unchanged (252 in the seven requested records, 117 in the four
additional records). Source fingerprints were not refreshed. No draft contract
is needed: the lineage and membership proof describes private implementation,
and eliminating its redundant work introduces no reusable product promise.

| Contract ID | File | Required represented facets |
| --- | --- | --- |
| `mossignal.provenance.retention` | `provenance-retention.yaml` | Current root manifest, complete closure, independent artifacts, logical append accounting |
| `mossignal.provenance.causal_explanations` | `causal-explanations.yaml` | Current support versus establishment, authoritative ancestry, public module law |
| `mossignal.inspection.semantic_projection` | `semantic-inspection.yaml` | Owned cause access, complete committed observations, pure reads |
| `mossignal.verification.optimized_paths` | `optimized-path-verification.yaml` | Executable refinement, reference independence limits, complete rejection observation |
| `mossignal.reconfiguration.atomic_replacement` | `atomic-topology-replacement.yaml` | Effective-time ordering, SOURCE migration, current source roots, atomic failure, later restore/replay |
| `mossignal.identity.machine_state_digests` | `machine-state-digests.yaml` | Content-addressed identity, storage exclusions, exact P4/E4/O4 profile and canonical inputs |
| `mossignal.persistence.machine_snapshot_restoration` | `machine-snapshot-restoration.yaml` | Strict validation, private cause reconstruction, round-trip and future sufficiency |
| `mossignal.node.edge_detector_family` | `edge-detector-family.yaml` | Exact comparison/successor observation support, unchanged detector laws |
| `mossignal.runtime.temporal_event_execution` | `temporal-event-execution.yaml` | Complete pending facts, deadline batches, chronological stream and atomic outer transaction |
| `mossignal.runtime.transaction_forecast` | `transaction-forecast.yaml` | Shared transition, owned result, source unchanged |
| `mossignal.diagnostics.persistent_episodes` | `persistent-diagnostic-episodes.yaml` | Material evidence, owned causal changes, deadline publication and rollback |

Task-scoped discovery searched cause references, private storage identity,
sharing, retention, pending work and translation in `docs/specs/`. The concrete
cause lifetime authority is `concrete_rust_api_surface.md`, Part XX / 76. Cause
references; content identity is governed by the persistence specification's
provenance sections. Runtime Part V / 30. Provenance records permits private
segments and forbids retroactive derivation mutation, consistent with the reused
owned-closure rules. Runtime Part XIX / 105. Open implementation freedom leaves
sharing and storage private. Relevant deadline, artifact, migration, diagnostic,
profile and verification facets are already represented above. No contradictory
or new applicable product requirement was found. Public checkpoint/history
controls, new temporal laws, persisted patches, catalogue evolution, and broader
contract completeness remain outside this slice.

## Source proof: identity and membership are separate facts

In `crates/mossignal/src/causal_store.rs`, `fresh_scope(Some(parent))` copies the
parent's 32 bytes and changes only bytes 16..24 for a fresh allocation batch.
`same_lineage` compares bytes 0..16. Therefore every ordinary descendant scope
has the parent's lineage. `transaction.rs::remap_cause` returns the original
CauseRef unchanged in that case; it changes scope only across lineages.

`Records::fork` changes the catalogue's allocation scope but clones its Arc node
handles and exact `BTreeMap<CauseRef, usize>` membership. Old nodes, predecessor
references and canonical caches stay identical. `Records::owned_closure` may
change catalogue positions but rebuilds membership using each node's original
CauseRef, without changing its stored ordinal or scope. `include` likewise
preserves node causes. `ProvenanceView::inspect` and `resolve_ordinal` resolve
through membership, rather than treating a CauseRef ordinal as a current vector
position. New records get the new allocation batch, avoiding collisions with
old handles even after collection changes catalogue length.

Same lineage alone does not prove membership for an arbitrary old cause. The
actual execution data flow supplies that proof:

1. `publish_candidate` selects the current root manifest, including input/output,
   operation, state, standard-module and pending causes, and includes active
   episodes' independently owned views before selecting the machine closure.
2. `apply_advance` takes those candidate-owned collections and the corresponding
   committed view. `build_ready_provenance` forks that view before using prior
   inputs, outputs, state or due-event causes. Due batches came from the retained
   calendar; newly constructed causes are appended to this same prepared graph.
3. Each internal deadline retains the complete accumulated prepared catalogue
   for the next reaction. There is no current-machine compaction between those
   reactions. Earlier output and diagnostic event roots therefore remain members
   until `publish_success` selects the outer result's closure.
4. Initialization starts a fresh graph with empty previous runtime collections;
   scheduling and observations consume only newly appended causes.
5. A ready patch switches to a fresh lineage through `checkpoint_migration`.
   It translates the selected source closure, finalized roots and pending facts,
   earlier result events, standard causal roles and active episodes before the
   target's ordinary fork. Target input/state/due/result causes consequently
   belong to the migrated lineage. The earlier source-side ordinary reactions
   have the original lineage. Initialization patches have no prior runtime roots.
6. `forecast` calls `stage`, the same unpublished transition used by `apply`.
   No second remapping path or evaluator exists.

`Records::push` still resolves every real predecessor from prepared membership
and owns its Arc. This necessary predecessor work, graph closure traversal,
canonical freezing and isolated catalogue copying must remain.

## Audited candidates and retained work

All ordinary candidates below are in `crates/mossignal/src/transaction.rs`
unless another file is named. Remove only the demonstrated identity work.

| Candidate | Evidence and bounded change |
| --- | --- |
| `remap_pending_causes` calls in initialization, internal deadline loop and final reaction | Pending causes are already graph members. Delete the identity-only sweeps, including both passes around scheduling in advance; keep calendar ownership, extraction, aggregation and scheduling. |
| `remap_output_event_causes` calls before/after internal and final scheduling | Earlier output causes remain members of the uncollected outer graph, or have explicitly migrated. Delete the accumulated-stream sweeps; keep event construction, chronology and result roots. |
| `remap_cause_map` on inertial cancellation, periodic anchor and periodic cancellation maps | Existing roots and newly scheduled/canceled facts have the same lineage and membership. Delete these sweeps; retain real insertion/removal and isolated map ownership. |
| `remap_episode_changes` | Changes consume current reaction causes; accumulated changes stay in the prepared graph or were explicitly migrated. Delete ordinary cause-only sweeps. Preserve episode reconciliation and patch translation through `DiagnosticEpisodeChange::remap_cause`. |
| `build_ready_provenance` input map reconstruction | The remap is identity, but the map is mutated by explicit observations and returned as successor state. Retain the necessary owned copy with causes carried directly. |
| `build_ready_provenance` temporary previous output map | It is used only as read-only `PreviousLevelOutputs.causes`. Pass the existing map directly; the separate remapped map has no ownership purpose. |
| `append_evaluation_provenance` previous edge/toggle/establishment/transport maps | Each returned map is updated by real node evaluations. Preserve owned map construction and updates; carry existing causes directly. Keep ms-49a's separate C/M records and both logical appends. |
| `append_evaluation_provenance` previous periodic anchor map | The local map is read-only, used to extend supporters and not returned. Borrow the prior map or use direct optional lookups; remove its identity-only reconstruction. |
| Due PulseDelay/TransportDelay/Inertial/Periodic supporters and periodic scheduling supporters | Aggregated due causes came from the retained calendar or finalized migration. Keep supporter extension/sorting/deduplication; copy handles without remap calls. |
| `append_inertial_cancellation` (also used for periodic cancellation) | A canceled obligation is already retained or explicitly migrated; replacement is a new member. Carry both causes directly; preserve the actual cancellation derivation. |
| `push_record` / `ProvenanceRecord::translate_causes` | Every production caller constructs supporters/contributions from prepared members. Initialization uses new causes; ordinary evaluation/scheduling uses fork members; checkpoint migration explicitly translates before calling. Remove the second identity traversal of new records, while retaining `Records::push` predecessor validation/ownership. |
| `standard/stateful.rs::observe_reaction` rebase callback | Its only three callers are initialization, internal and final settlement. Prior latest-reset/toggle/capture roles are retained current roots or already migrated. Carry them directly; retain the complete new StandardHistory and causal-role maps, module laws and new accepted-event updates. |

Mechanical removal of private parameters or helpers made unused by these changes
is permitted. Exact helper spelling and whether a necessary owned map uses
`clone` or equivalent direct construction are deterministic implementation
freedom; no broad map-sharing redesign is selected.

The following translations have a different responsibility and must survive:

- `ProvenanceView::restored` uses `fresh_scope(None)` and `remap_record` to turn
  validated temporary `[0; 32]` ordinal references into a new owning lineage.
  `snapshot_restore.rs::build_view`, `unfinalized_cause` and `scoped_cause` also
  materialize digest-to-private-handle correspondence. This is private handle
  reconstruction, not semantic repair or topology migration.
- `checkpoint_migration` starts a fresh lineage, retains source-qualified
  canonical SOURCE facts, translates checkpoint predecessors using
  `translated_cause` / `translate_record`, and constructs migration and target
  pending records. Explicit translations of accumulated outputs, diagnostic
  changes, standard roles and active episodes remain. Removing the redundant
  `push_record` traversal does not remove these earlier translations.
- `migration.rs::remap_node_causes`, `remap_input_causes` and
  `remap_output_causes` preserve/reassociate structural keys under the prepared
  plan. Even when their CauseRef values stay unchanged, their key mapping and
  filtering are real migration work. Preserve these helpers.
- Existing test-only record reconstruction in `Records::reverse_supporters`
  and `duplicate_allocation_for_test` is outside the production optimization.
  Do not weaken permutation, canonical-coalescing or foreign-cause tests.

## Verification plan

First add test-only work accounting at the existing actual production remap
boundaries and write the regression without removing production work. Count
same-lineage remap attempts and identity-only visits separately by pending,
accumulated outputs, diagnostics, cause maps, standard roles and new-record
supporters/contributions. Count reconstruction of read-only remap maps as well.
An existing `old_record_rewrites == 0` assertion is insufficient: these obsolete
CauseRef traversals currently leave immutable records untouched. Instrumentation
must count real execution, not expected fixture sizes or observer traversals.
Run a zero-obsolete-work assertion against the accepted baseline and observe
its positive counts failing before the implementation correction. Preserve that
regression and accounting after removing the actual walks, including walks that
could otherwise be hidden by replacing a remap callback with an identity one.

Use small bounded fixtures and record their exact nonempty exercised facts:

- Initialization creates delayed work and real cancellation/phase roots where
  applicable. Subsequent quiet advances retain future obligations. Vary retained
  far-future groups (for example 1/8/32) independently of processed deadlines.
- A direct jump crosses at least three distinct internal deadlines and then a
  quiet target reaction, with at least one future obligation still retained.
  Earlier pulse/level outputs accumulate. For example, TransportDelay controls
  can establish set at time 2, establish reset at 3, and clear reset at 4;
  a retained-conflict latch then begins an episode at 3 and resolves it at 4.
  Keep a separate obligation beyond the target. This exercises the diagnostic
  sweep with nonempty accumulated causes; do not require a material-change
  event from mere reevaluation of an unchanged retained conflict.
- Supplement with inertial replacement and periodic disable/reenable cases to
  exercise cancellation and phase maps and all four due-event cause families.
  Existing standard-stateful fixtures cover retained reset, toggle (including
  accepted even batches) and capture roles after quiet target settlement.
- Repeat meaningful initialization and ready cases through forecast. Reset/read
  counters immediately around the ordinary transition, before snapshots,
  inspections, canonical references, restoration or replay verification.

Require zero eliminated attempts/visits in ordinary apply and forecast, not
zero total work. Report necessary successor-map copies, catalogue membership
copies, predecessor ownership, closure visits/bytes, encodes/hashes and logical
growth separately. The counter domain must distinguish genuine reconstruction
or translation from obsolete identity work. Snapshot restoration must exercise
nonidentity private-handle reconstruction; a preserving qualified patch after
earlier deadlines must exercise actual SOURCE/old-to-new translation. After
either boundary, later patch-free continuation again has zero obsolete work.

Compare immutable canonical record bytes, exact event and diagnostic data,
stable subject/role-to-content bindings, calendars/public serials, schedules and
state. For surviving old facts, assert unchanged raw CauseRefs and actual shared
nodes/membership across ordinary forks; do not require arbitrary collected
causes to resolve through the latest machine. Hold old results, observations,
episodes and forecasts and resolve their saved references after later execution
and machine drop. Compare held-versus-dropped artifact cases for unchanged
machine roots, digests, budgets and work; release owners and verify reclamation.

Reuse `transaction/tests/execution_preparation.rs`, `state_digest_reference.rs`
and the existing direct-jump, replay, forecast, standard-stateful, root-retention
and ms-49a edge oracles. The clone-preparation path shares the evaluator and
publication, so it cannot independently detect the deleted remap calls. Supplement
it with actual-work instrumentation, direct membership/identity checks, detached
canonical comparisons and exact pre-existing vectors; document this limitation.
Direct-versus-stepwise temporal comparisons normalize only permitted reaction
partition/stamp differences, never invent full digest equality for added steps.

Keep all golden files byte-for-byte equal to accepted HEAD. Neither P4/E4/O4
identities nor any API, schema, profile, domain or version changes. Validate
encoded restore and subsequent qualified patch/replay/forecast continuations,
with original SOURCE ownership and strict edge C/M validation. Reuse exact edge
append ledgers and budget thresholds; identity work removal earns no growth
credit. Exercise complete predecessor rollback after earlier deadlines at the
existing extraction/provenance/digest/result/projection fault points and real
budget/overflow rejection, including held causes and warm digest caches.

Implementation acceptance requires applicable focused tests (nonzero matching
test counts), `make check-dev`, `make check-final`, `git diff --check`, an exact
vector diff against the baseline, and a source audit that every preserved genuine
translation still exists. Preparation runs contract tooling and diff checks only;
it makes no claim that implementation acceptance gates have run.

## Delivery boundary

Expected production edits are local to `transaction.rs` and
`standard/stateful.rs`, with private unused-helper cleanup only as necessary.
Test accounting belongs in `causal_work.rs`; focused tests may extend the
transaction test modules and existing integration fixtures. Restoration,
migration, root selection, encoders, digest references and vectors are controls,
not planned semantic rewrite surfaces.

Exclude incremental freezing, result-specific ownership changes, observable
digest redesign, garbage-collector redesign, arbitrary history caps, broad module
refactoring, new node/module behavior, Mossmoot or dependency adoption, push, and
other continuation slices. Submit only this preparation and one unclaimed bead;
the coordinator owns independent acceptance and further assignment.
