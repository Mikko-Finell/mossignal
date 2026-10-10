# Result-specific transaction provenance preparation

Independently accepted planning and API boundary on 2026-10-10, from clean `main` at
`126120550517764c4188572f7b47b28bffb75409` (accepted `ms-k1l`). No Rust or host
implementation is part of this pass. Generated implementation bead: `ms-am9`,
"Select transaction provenance from exposed result causes", open and unassigned,
depending on closed `ms-k1l`.
The coordinator accepted the complete result manifest and existing-inspection
consumer migrations without scope or contract correction. Export and planning
commit are coordinator-owned; a separate assignment is required before
implementation. This acceptance does not claim implemented behavior.

The task comes from Mossmoot `docs/scratch/refactoring_brief.md`, section 2,
after accepted ordinary remapping and incremental freezing. That brief is task
context, not Mossignal authority or an active roadmap. Initial triage found all
99 beads closed and no ready/in-progress or matching open work. An explicit
closed-item search found related foundations (`ms-0ap`, `ms-0ap.1`, `ms-0ap.2`,
`ms-wn5`, `ms-e43`) and edge planning (`ms-49a.1`); they cover earlier sharing,
collection, identity or diagnostic work, or exclude result narrowing. This
continuation is a new bounded task, not reopening those accepted scopes. No
roadmap heading is changed.

## Recommended boundary

Keep the current public types and methods. A transaction result independently
owns sufficient complete ancestry for every cause exposed by its whole result.
Use existing owned machine/forecast inspections for deliberately requested
current facts. The implementation bead selects the smallest result root manifest
for the currently implemented surface:

```text
union by CauseRef identity of
    cause of every OutputEvent in the complete chronological output_events
    cause of every DiagnosticEpisodeChange in diagnostic_episode_changes
then retain their full backward closure
```

This is a task design choice within the existing sufficient-ownership contract,
not a newly source-grounded requirement that all conforming implementations must
have exactly this physical view. An empty cause-bearing change/event stream has
an empty result view even when the machine has current facts. A small event can
still require large genuine ancestry. Retained results do not serve as implicit
current-machine inspections. Clarify `TransactionResult::provenance` rustdoc to
describe all exposed result causes, and point current-state questions to existing
owned inspections. No blanket machine artifact, new public method, deprecated
alias, compatibility shim or new error category is proposed.

The authoritative minimum is already represented by
`provenance-retention.yaml` / `independent-artifact-ownership`,
`semantic-inspection.yaml` / `retained-cause-ownership`, and the concrete API's
Part XVII / 64–66 and Part XX / 76. No source promises that a result resolves
unrelated machine facts. There is no conflicting requirement or frozen baseline.
The pre-release policy permits deliberate in-place evolution of incidental
view size/lookup behavior. No normative amendment or new draft contract is
needed for this conforming bounded choice. Permanent stronger policy can be
considered only after review of the concrete result.

## Contract selection and evidence

Read the skill's three references, contract README/template and catalog. Initial
seven-record coordinator evidence had 223 unchanged citations. Task discovery
additionally covered occurrences, episodes, bindings, initialization/ready
transactions, ordered/temporal execution, digests, snapshot/restore scope and module
inspection. All selected records remain recorded as `reviewed`; status is
per citation, not a claim of global completeness.

| Record under `docs/specs/contracts/` | Contract ID | Unchanged / changed citations | Task facets used |
| --- | --- | ---: | --- |
| `provenance-retention.yaml` | `mossignal.provenance.retention` | 5 / 0 | independent artifact and current-machine ownership, complete closure, growth and rejection |
| `semantic-inspection.yaml` | `mossignal.inspection.semantic_projection` | 10 / 0 | complete owned observations, cause lifetime, pure committed projection |
| `causal-explanations.yaml` | `mossignal.provenance.causal_explanations` | 10 / 0 | current versus transition, authoritative complete ancestry, temporal and module explanations |
| `transaction-forecast.yaml` | `mossignal.runtime.transaction_forecast` | 26 / 0 | owned hypothetical result, read-only state, unchanged source and apply agreement |
| `replay-artifacts.yaml` | `mossignal.persistence.replay_artifacts` | 82 / 0 | ordinary fold, canonical artifacts and current version validation |
| `atomic-topology-replacement.yaml` | `mossignal.reconfiguration.atomic_replacement` | 81 / 0 | effective-time source events, revision/episode provenance, migration reporting and atomic failure |
| `optimized-path-verification.yaml` | `mossignal.verification.optimized_paths` | 9 / 0 | executable refinement, actual work evidence, complete rejection observation and oracle limits |
| `runtime-diagnostic-occurrences.yaml` | `mossignal.diagnostics.runtime_occurrences` | 19 / 3 | owned occurrence record and committed result publication |
| `persistent-diagnostic-episodes.yaml` | `mossignal.diagnostics.persistent_episodes` | 17 / 0 | owned active/change records, material change, causal retention and chronological publication |
| `application-bindings.yaml` | `mossignal.binding.application_adapter` | 32 / 0 | lossless projected cause/stamp/revision and ordinary delegation |
| `live-bindings.yaml` | `mossignal.binding.live_reconfiguration` | 5 / 0 | historical producing map and projection before publication |
| `initialization-transaction.yaml` | `mossignal.runtime.initialization_transaction` | 26 / 7 | resolvable initialization causes, single publication and failure preservation |
| `ready-level-transaction.yaml` | `mossignal.runtime.ready_level_transaction` | 42 / 1 | resolvable ready causes, single publication and failure preservation |
| `ordered-reactions.yaml` | `mossignal.runtime.ordered_reactions` | 18 / 0 | producing stamps, effective-time source partition and atomic resource failure |
| `temporal-event-execution.yaml` | `mossignal.runtime.temporal_event_execution` | 30 / 0 | chronological complete result, deadline partition and explicit budgets |
| `machine-state-digests.yaml` | `mossignal.identity.machine_state_digests` | 57 / 0 | existing root/version profile, immutable content, pure result exposure and failure identity |
| `machine-snapshot-artifact.yaml` | `mossignal.persistence.machine_snapshot_artifact` | 65 / 0 | pure snapshot creation and current self-contained provenance |
| `module-aware-execution.yaml` | `mossignal.module.aware_execution` | 32 / 3 | retained hierarchy and qualified provenance/owned observation |
| `machine-snapshot-restoration.yaml` | `mossignal.persistence.machine_snapshot_restoration` | 80 / 0 | unchanged current-root validation, complete provenance and restore/continuation sufficiency controls |

Total: **646 unchanged and 14 changed citations** across nineteen records, with
none missing, ambiguous or unfingerprinted. Full status is
`/private/tmp/result-ownership-selected-contract-status.txt`.

The changed citations are pre-existing evidence drift, not edits in this pass:
occurrence simultaneous-batch/conflict/catalogue context; initialization
schema/output/constructor/budget/catalogue context; ready budget context; module
deadline/budget/direct-jump context. The chosen ownership/publication facets in
the three early contracts and qualified-observation facets in the module record
have only unchanged sources. Changed-facet rules are not a new authority for
this bead. The three module sections were inspected: current occurrence-aware
deadline partition, logical-growth accounting and partition-sensitive direct
jump verification are governed here by the up-to-date ordered, temporal,
retention and verification records. No stored hash was refreshed and no record
was reopened or promoted. General coverage or unrelated drift is not a blocker.

Every discovered applicable facet maps to the table's represented rules.
Concrete API Part XVII / 65's broader future `SemanticChangeSet` and persistence
Part XII / 118–119's standalone result audit artifacts are adjacent unsupported
delivery surfaces, not prerequisites for narrowing the implemented result.
Implementing them or claiming their formats is excluded. A future cause-bearing
result field must extend the result manifest; it cannot inherit today's two-list
inventory silently. No new or changed contracts are selected.

## Complete implemented artifact inventory

| Product/facet | Exposed causes and owner |
| --- | --- |
| `TransactionResult` scalar time, processed stamps, revisions, digests, schedule | No CauseRef. Result digests identify the machine before/after, not result view membership. |
| `output_events` | Each LevelEstablished, LevelChanged and Pulsed has one cause. Select every occurrence in stream order, including intermediate deadlines and source revisions; retain full ancestry. |
| `occurrences` | `DiagnosticOccurrence` owns Problem, stamp and revision. Current nested Problem/evidence/subjects contain no CauseRef; preserve their exact values/order, without inventing a provenance root. |
| `diagnostic_episode_changes` | Every Began, Changed, Resolved and Terminated exposes `cause()`. Before/after Problems own evidence but contain no additional CauseRef. Select all changes, including earlier/resolved/removed owners. |
| scoped diagnostic accessors | `occurrences_for` and `diagnostic_episode_changes_for` return selections borrowed from the same complete result; scope filtering must not shrink the enclosing ownership manifest. |
| `migration()` | Public MigrationReport owns structural/state/event/input/output/episode/provenance/loss/region/module classifications. Its public nested records expose stable subjects, event keys and rules, not CauseRef. Private FinalizedMigration cause maps are machine preparation, not returned report data. No extra root solely for report presence. |
| `provenance` and event explanation | Result view owns the above closure. Cloning ProvenanceView independently retains that selected closure. `explain_output_event` selects an actual result event and returns its own owned causal explanation. |
| `BoundTransactionResult` | Owns ordinary result plus one-to-one ProjectedOutputEvents. Projected causes/stamps/revisions equal ordinary ones, using source/target producing maps. `into_parts` moves both products; detached projected events require retaining ordinary result/view to resolve their handles. |
| `ForecastResult` | Owns an ordinary hypothetical result and a distinct ForecastState candidate. `into_parts` separates them. Current facts belong to candidate inspections, not otherwise empty hypothetical events. Neither TransactionResult nor ForecastResult currently offers public Clone; view sharing/owned retention is the existing path. |
| ActiveDiagnosticEpisode and inspection products | Active episode clones and direct node/output/module/typed temporal/stateful inspections own their exposed causes independently. They remain valid after continuation, compaction, patch and machine drop. Unchanged active evidence needs no transaction-result root solely because it is active. Beginning-result ownership and current material evidence remain separate. |

Source proof: `transaction.rs::publish_success` already seeds event/change roots,
then extends with candidate operation/input/output, standard history, stateful
and temporal maps and pending events. Those extensions manufacture the excess
ownership. `ProvenanceView::owned_roots`/`Records::owned_closure` select shared
nodes and rebuild exact selected lookup membership without altering source
nodes. Preserve the source deadline accumulation, `checkpoint_migration` result
roots, translation and `publish_candidate` machine-root selection. Never replace
the complete stream by final output values or trim a genuine predecessor.

## Identity, encoding and ownership effects

Narrowing the selected result view intentionally changes its len/is_empty,
Debug record count and lookup of unrelated facts. CauseRef allocation scopes and
ordinals, retained record bytes/digests and exposed supporter/contribution roles
stay unchanged: selection copies membership, not causal content or namespaces.
An excluded reference continues to use existing InvalidCause/ForeignCause
lookup behavior; do not promise one category irrespective of allocation scope.

Current-machine retention, projections and snapshots are separate. Their
P4/E4/O4 inputs, golden bytes, logical growth, atomic failure and supported
restore behavior must remain exact controls. `ReplayFrame::from_success` records
transaction metadata and before/after machine digests, not result provenance;
native frames/logs therefore retain their exact bytes. The decoder explicitly
rejects an embedded `transaction_result` audit artifact as unsupported after
validating its envelope. There is no native standalone result encoder or
TransactionResultContentDigest implementation to update or silently version.
Preserve that rejection boundary. Future persisted-result cause sufficiency is
an adjacent specification obligation, not delivered by this ownership task.

## Native consumer migrations

Searches covered result and provenance accessors/fields, result extension impls,
native tests/examples, bindings, forecasting, replay and private canonical
helpers. Event-derived explanation consumers remain on result views: pulse
fanout/merge/route/gate/convenience tests, emitted-edge tests, chronological
episode/scoped diagnostic tests, forecast event comparisons, binding tests,
and the lifecycle/live-bindings/diagnostics examples. Preserve their existing
explanation and lifetime assertions.

The following concrete sites ask about current facts rather than returned
events. Migrate the owner while preserving each original semantic assertion;
do not weaken an oracle merely to get a smaller view to pass.

| Site (under `crates/mossignal/`) | Intended fact and existing replacement |
| --- | --- |
| `src/transaction.rs::toggle_ready_reactions_commit_once_and_retain_latest_inversion_cause` | Even/quiet reaction's retained inversion: hold `inspect_toggle` and resolve through its provenance; genuine changed event explanations stay result-owned. |
| `src/transaction.rs::ready_advancement_overlays_delta_and_emits_only_genuine_changes` | Reasserted/empty unchanged output: own `inspect_output(output)` and resolve its current support. Initial/changed event causes stay on their results. |
| `tests/level_set_reset_latch.rs::exhausts_initial_and_ready_control_state_policy_laws` and `ordinary_history_and_same_value_requests_preserve_state_and_causes` | Stored establishment even without event: retain the existing `inspect_level_set_reset_latch` value and use its provenance throughout supporter traversal. |
| `tests/pulse_set_reset_latch.rs::ready_reactions_set_reset_retain_and_repeat_transient_conflicts` and `deadline_and_target_time_occurrences_keep_chronological_order_and_provenance` | Same-value refreshed or retained delayed establishment: hold `inspect_pulse_set_reset_latch`, use that view; preserve result-owned occurrences/events separately. |
| `tests/level_conveniences.rs::at_least_zero_provenance_is_independent_of_inputs`, `at_least_above_arity_provenance_is_independent_of_inputs`, `parity_provenance_cancels_even_source_multiplicity` | Unchanged current output support/blockers: acquire `inspect_output` and use its current-support view. Emitted initialization/change assertions remain result-owned. |
| `tests/edge_detectors.rs::remembered_observation_does_not_inherit_obsolete_comparisons`, `edge_family_patch_preserve_reset_and_cross_kind_support_survive_two_restores`, `edge_observation_initialization_patch_declared_checkpoint_restores` | Current remembered observation and its source checkpoint: retain `inspect_edge_detector` and traverse its view. Do not alter C/M derivations or both appends. |
| `tests/atomic_topology_replacement.rs::inherited_input_and_retimed_pending_work_restore_against_target` | Future restarted pending group: use the already acquired `inspect_pulse_delay` view (or `inspect_pending` for one group), not the patch result. |
| `tests/module_execution.rs::nested_instances_keep_state_pending_work_and_provenance_independent` | Qualified node/current scheduling and pending group from a module: use the containing `ModuleInspection::provenance` for its nested records. |
| `src/transaction/tests/incremental_freezing.rs::incremental_freezing_qualified_source_bytes_survive_changed_target_slots_and_restore` and `cause_remapping.rs::ordinary_remap_restore_and_qualified_source_translations_remain_real` | Selected current standard SOURCE facts: inspect the corresponding module/role, or use private machine view for machine-projection verification. Keep explicit-source recomputation and exact source-byte comparisons. Do not assume every current role belongs to an event. |
| `src/transaction.rs::quiet_growth_counts_appends_even_when_current_membership_does_not_grow` and private measurement/oracle helpers | Preserve logical append/current-membership checks against current machine provenance or a targeted node inspection. Result closure and machine closure must be compared under their own manifests. |

Some other tests obtain a cause from an inspection that is also a genuine
ancestor of a returned event, for example cumulative Toggle event history and
qualified emitted-node ancestry. Keep those as event-ancestry controls when
that relation is explicitly established. If their purpose is current state,
use the inspection's own view instead. Preserve canonical/reference coverage of
the complete machine in addition to comparisons of the now selected result.
The implementation must audit failing sites by intended question, not blindly
replace every result view or delete full-machine assertions.

Existing owned `inspect_node`, `inspect_qualified_node`, `inspect_output`,
`inspect_pending`, `inspect_module` and typed stateful/temporal methods, plus
their supported ForecastState equivalents, are sufficient. A whole-machine
inspection product is unnecessary. No host API addition is justified by these
consumers.

## Mossmoot audit and dependency boundary

Read-only host audit at clean
`9f0ff760db115c3bbc90ddcbd47ca856df0865ea`, still pinned in Cargo.lock to Mossignal
`69e26269c1279d0895a827cb167abffb103a49c9`. `signals/operations.rs::SignalReaction`
owns BoundTransactionResult; `events`, `changed_controls` and `fire_requests`
consume projected events, counts and producing stamps. Construction/dismantling
and mechanism consequence paths hold/move this same event product through
`signals/edits.rs`, `construction_settlement.rs`, `mechanical_devices.rs` and
`kernel/mechanism_consequences.rs`; they do not resolve unrelated core causes.
`SignalsState::hourglass_view` acquires an owned `Machine::inspect_pulse_delay`
through `BoundMachine::machine()` and reads pending counts/deadlines. It already
uses the recommended current-state boundary. Scoped searches found no result
provenance/current-cause resolver in Signals or its reaction consumers.

Recommended host migration: **none at the audited source baseline**. Keep owned
projected events and the existing pulse-delay inspection. Adoption of the later
Mossignal commit and host verification belong to separately coordinated host
work; do not update Cargo files or edit Mossmoot in this bead/preparation.

## Verification for later implementation

1. Regression first, source-visible failed-before witness: build unrelated
   genuinely cumulative stateful ancestry at finite 1/10/1000 scales beside an
   empty or fixed small result. Confirm the baseline result owns the unrelated
   cause, then require the new empty view/fixed selected closure. No arbitrary
   history cap or fabricated zero-growth transaction. Instrument the actual
   result-selection boundary (root visits, selected closure and lookup work),
   separately from required machine/episode/inspection selection, catalogue
   copying, new admission, logical growth and serialization. No measurements
   are claimed by this planning pass.
2. Retain all emitted/changed causes and their full supporter/contribution
   closure across multiple deadlines, source events before a patch, target
   events, removed subjects and episode Began/Changed/Resolved/Terminated.
   Exercise occurrences-only and quiet-active-episode results: occurrence
   evidence remains intact, while no cause root is invented. Retain a beginning
   result separately from a later active material-evidence inspection.
3. Hold explicit node/module/output/pending/typed inspections across later
   state changes, patches, compaction and machine drop. Resolve every exposed
   cause through its correct artifact. Split forecast result/state and bound
   ordinary/projected parts; clone a result ProvenanceView and independently
   retain an event explanation. Dropping one owner preserves other complete
   closures. Dropping machine/inspection while an empty result remains must
   release excluded payloads and lookup entries, verified with real weak handles
   and live node/index counters. Keep genuine event ancestry until its final owner.
4. Compare held/dropped artifact runs: exact machine manifest, observations,
   P4/E4/O4, snapshot/replay bytes, events/episodes/occurrences, append growth and
   budget boundaries stay equal. Inject meaningful extraction/provenance/digest/
   result/projection failures and compare complete warm predecessors and lookup
   ownership at the stages actually reached; preserve real exact-limit failures.
5. Keep uncached canonical and clone-preparation controls, now with distinct
   result/machine manifests. These share grammar/context or evaluation/publication
   and cannot alone prove the selected ownership boundary. Supplement with
   independent exposed-cause root enumeration, raw membership/lifetime, exact
   vectors and phase counters. Patch-event SOURCE ancestry may itself contain
   broad genuine support; result selection cannot remove it. Necessary artifact
   encoding, observable closure and current-machine work remain measured costs.
6. Nonzero focused tests for result ownership, affected current-fact consumers,
   edge/remap/freezing/clone controls, diagnostic episodes/occurrences, multi-
   deadline patch events, live bindings, inspection, forecast and replay;
   then `make check-dev`, `make check-final`, whitespace/source audit and exact
   baseline golden/spec diff. Never weaken a gate, oracle, budget or genuine
   ancestry to satisfy the optimization.

## Readiness and preparation checks

No conflicting requirement, missing fundamental semantic rule or frozen promise
blocks this slice. The coordinator independently accepted the recommended
existing-API boundary and bounded consumer migrations. Implementation requires
a separate assignment. Private root helper location, deterministic
deduplication and instrumentation remain implementation freedom.

Expected native implementation surface: result selection/rustdoc in
`transaction.rs`; a small private root helper/accounting addition if useful;
focused regression and the current-fact consumer tests named above. Preserve
canonical encoders, machine manifests, migration/restore logic and binding
publication. Exclude new result codecs/change-set fields, blanket inspections,
public history selectors, checkpoint redesign, arbitrary history limits,
observable digest redesign, host adoption and unrelated optimization.

Planning checks: catalog/status discovery completed; all eleven contract-tool
tests passed (`/private/tmp/result-ownership-contract-tool-tests.log`). Every
contract ID and selected rule resolves, and every source supporting those rules
is unchanged. The fourteen changed adjacent citations are excluded from the
bead's represented basis. Text/whitespace, native site references, bead
round-trip/dependency/readiness and exact implementation/spec/host change-surface
checks passed; final details are in
`/private/tmp/result-ownership-planning-checks.log`. No Rust implementation gates
were run for this planning-only task. Only this proposal and the unexported
`ms-am9` database entry are prepared; the coordinator owns export/commit.

Independent acceptance reviewed the complete exposed-root inventory, representative
current-fact consumers, owned inspection/forecast/binding boundaries, native replay
metadata and the host consumer audit. Recomputed status for all nineteen records
reproduced 646 unchanged and 14 adjacent changed citations; every selected rule
uses unchanged sources. All eleven contract-tool tests passed independently.
No contract, normative specification, Rust, API addition, identity profile or host
dependency change is part of this preparation. Product behavior, actual ownership
measurements and implementation gates remain the next assignment's obligations.
