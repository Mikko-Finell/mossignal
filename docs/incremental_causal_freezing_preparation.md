# Incremental causal freezing preparation

Preparation for one bounded implementation bead, independently accepted by the
coordinator on 2026-10-10. This document is not normative. No implementation is
part of this planning pass.

## Baseline and contract reuse

- Clean `main` at `a4ca810963d9d11e5dc527af96a1b523d9e78689`, accepted `ms-efv`;
  four commits ahead of `origin/main`. Governing specifications and contracts
  have no working-tree differences.
- Initial triage: all 98 beads closed; no ready or in-progress work. This is a
  separately assigned continuation, not a retired roadmap item.
- Generated bead: `ms-k1l`, depending only on accepted closed `ms-efv`.
  Independent planning acceptance is complete; keep it open and unassigned
  until the separate implementation assignment.
- Authors use `br --no-auto-flush`. The coordinator owns export and commit.

Catalog selection and selected-source status found all ten records below
`reviewed`, with all **390** citations unchanged (242 in the six requested
records, 148 in the four additionally selected records). No fingerprints were
refreshed or contracts reopened. The read-only status log is
`/private/tmp/ms-incremental-freezing-selected-contract-status.log`.

| Contract ID | Path under `docs/specs/contracts/` | Governing represented rules |
| --- | --- | --- |
| `mossignal.identity.machine_state_digests` | `machine-state-digests.yaml` | `content-addressed-provenance`, `canonical-profile`, `current-root-version-profile`, `equality-is-not-validation`, `digest-verification` |
| `mossignal.provenance.retention` | `provenance-retention.yaml` | `retained-root-closure`, `current-machine-root-manifest`, `independent-artifact-ownership`, `logical-growth-independent-of-reclamation` |
| `mossignal.inspection.semantic_projection` | `semantic-inspection.yaml` | `retained-cause-ownership`, `pure-committed-projection` |
| `mossignal.verification.optimized_paths` | `optimized-path-verification.yaml` | `executable-refinement`, `optimization-evidence`, `maintained-independent-references`, `complete-rejection-observation`, `limited-reference-independence` |
| `mossignal.reconfiguration.atomic_replacement` | `atomic-topology-replacement.yaml` | `effective-time-order`, `revision-provenance-and-episodes`, `current-source-topology-supporters`, `atomic-failure`, `committed-snapshot-and-later-replay` |
| `mossignal.persistence.machine_snapshot_restoration` | `machine-snapshot-restoration.yaml` | `events-episodes-and-provenance`, `current-root-version-validation`, `no-migration-or-repair`, `atomic-publication`, `round-trip-and-sufficiency` |
| `mossignal.provenance.causal_explanations` | `causal-explanations.yaml` | `current-versus-transition`, `authoritative-retention-boundary` |
| `mossignal.persistence.replay_artifacts` | `replay-artifacts.yaml` | `ordinary-fold`, `starting-identity`, `temporal-and-module-equivalence`, `history-and-verification` |
| `mossignal.runtime.transaction_forecast` | `transaction-forecast.yaml` | `shared-transition`, `source-unchanged`, `owned-forecast-result`, `agreement-and-failure` |
| `mossignal.runtime.temporal_event_execution` | `temporal-event-execution.yaml` | `chronological-ready-advancement`, `single-atomic-publication`, `explicit-runtime-budgets` |

Task-scoped specification discovery covered immutable derivations, creating-source
interpretation, Merkle relations, digest collisions, ownership, publication and
private storage. Applicable facets map to these unchanged rules. Runtime
architecture Part V / 30 and Part X / 63–70 establish immutable acyclic records
and source-qualified migration; persistence Part VI / 60 and Part VII / 61–66
establish collision handling and complete content-addressed closure. These are
consistent with the reused rules, not new product promises. Private cache/index
representation and timing within unpublished preparation remain implementation
freedom. No draft contract, specification amendment or genuine blocker is needed.

## Concrete source proof

`state_digest.rs::freeze_provenance` creates a fresh `ContentTable`, calls the
test-only `Records::freeze_sources`, then calls `index_view` over the entire
retained view. `index_view` allocates a full-length memo, cycle stack and returned
digest vector, then calls `digest_record` for every position. A cached node still
inserts its payload Arc into the new table. Thus zero old encodes/hashes does not
prove bounded freeze work. The existing projection `records_indexed` counter
also mixes this cost with required later indexing.

`Records::push` already admits each predecessor through exact membership and owns
its Arc before adding the new node. Every predecessor therefore exists before
its consumer, including supporter and contribution occurrences. This supports
direct new-record encoding using predecessor content, without recursively walking
old ancestry or allocating scratch for the old catalogue. Canonical relation
roles, contribution positions/counts and supporter multiplicity must still come
from the record, not a deduplicated ownership inventory.

`Node::canonical` is a shared OnceLock. Today `digest_record` compares both bytes
and digest if concurrent initialization loses the set race. Cached nodes preserve
their first interpretation. `ContentTable::insert` additionally rejects distinct
payloads under one digest. Neither check may silently disappear merely because
the freeze pass stops constructing a full table. The independently recomputing
reference also enforces collision consistency.

| Path | Creating context and required treatment |
| --- | --- |
| Initialization, including initialization patch | `build_initialization_provenance` starts a fresh catalogue; scheduling appends more records before `finalize_provenance_build`. All are new. Use the evaluated topology, which is the target for a patch. The declared-state checkpoint already contains its source bytes. |
| Ordinary advance, including multiple deadlines | Each `build_ready_provenance` forks the previous graph. Scheduling adds records, then finalization occurs for that deadline. New content uses the current source topology. Earlier frozen nodes stay shared. Preserve chronological events and both edge C/M appends. |
| Ready topology replacement | Strictly earlier deadlines finalize under the source. `checkpoint_facts(source, selected)` must keep source canonical bytes. `checkpoint_migration` creates a fresh catalogue of opaque source fact/checkpoint wrappers, patch, target migration and pending records, and currently returns it **unfrozen**. Target `build_ready_provenance` then forks that catalogue; its inherited prefix is therefore not necessarily frozen. Eager admission must cover these replacement records too. Source wrapper fact bytes remain opaque; target-owned subjects use `finalized.compiled`. |
| Restoration | `snapshot_restore::build_view` validates and derives a Kahn order, materializes every record, then `ProvenanceView::restored` creates a fresh lineage in that order. All nodes are new; use the validated compiled target and preserve checkpoint SOURCE payloads. Validation and genuine reconstruction remain complete. |
| `owned_closure`, `include`, collection | These share admitted Arc nodes, reindex catalogue positions and select/include ownership. They do not create new semantic records or justify refreezing under another topology. Preserve membership, selection and independent artifacts. Any consistency lookup follows precisely those members. |
| Test helpers | `append_record_for_test` and `duplicate_allocation_for_test` append genuine new allocations; `reverse_supporters` reconstructs nodes with existing content and source context. The deep-release test creates a raw chain. Adapt private construction helpers deliberately; do not leave an untracked empty cache or weaken permutation/coalescing/foreign-cause tests. |

The test-only source OnceLock currently records creating `CompiledNetwork` values
by a full-view sweep. The uncached reference uses these contexts while ignoring
candidate cached bytes/digests and its membership algorithm. Bind context only
for new admission; preserve the reference's source interpretation through forks,
closure/include and helper reconstruction. Do not retain compiled source graphs
in production merely to serve this test oracle.

`stage` operates on a private successor. `publish_success` selects result roots,
installs candidate state only on that successor and prepares completed digests;
external publication follows successful staging. The observable projection's
`ProjectionContext::build`, snapshot `cause_digest_index` and selected SOURCE
`checkpoint_facts` still require complete relevant closure indexing and payloads.
Preserve these paths and their `ContentTable` collision checks. Result roots can
include earlier events absent from the final machine manifest, so checking only
the final observable table is not a substitute for admitted-graph consistency.

## Bounded design and implementation freedom

Select eager freezing at new-node admission as the default simple deterministic
design: pass the creating compiled context into a private admission helper,
resolve and own real predecessors, read their already-frozen content, encode/hash
the new record, and finish its immutable canonical content before sharing or
publication. This removes the ordinary full-view freeze pass rather than adding
a new batch frontier protocol. All-new restore and replacement paths use the
same admission rule. Necessary logical appends and existing budgets stay intact.

Keep admitted-graph digest/payload consistency with an incremental, graph-local
lookup that checks new content against available prior content. Forks share it
without rebuilding or copying the entire old-content map as freeze bookkeeping.
It must not keep unrelated nodes, catalogue versions or dead lookup entries alive.
Selection/include update it alongside their existing actual membership work;
do not add another whole-old-view validation pass. The exact concrete lookup is
private implementation freedom: a small path-shared fixed-width digest index is
one conforming option, not a required generic framework or new dependency. An
equivalent simpler design is allowed if it demonstrates all cost, consistency
and ownership obligations, including old/new collisions outside final machine
roots. Rebuilding the old ContentTable at another unconditional boundary fails
the objective.

OnceLock can remain with its complete race check. Alternatively, exclusive eager
construction may finish content before any Arc sharing and make competing
initialization impossible; prove that invariant for every admission/helper path
and test concurrent readers. Changing the private field is not permission to
accept inconsistent interpretations. Ordinary retained nodes must never be
reencoded under a target topology.

Expected production surface: `causal_store.rs`, `state_digest.rs` and local
`transaction.rs` admission/finalization plumbing. Minimal restore helper plumbing
is permitted if necessary. Test accounting/helpers may touch `causal_work.rs`,
`projection_work.rs`, the uncached reference's context access and focused tests.
Canonical grammar, domain constants, root selection, migration semantics and
strict restoration checks are controls, not rewrite surfaces.

## Verification required during later implementation

1. Instrument actual old freeze traversal, memo/stack/result scratch slots,
   cached payload table visits/insertions and source-context visits before
   correction. Add a zero-old-work regression and observe it fail against this
   baseline. Accounting must encompass new admission/lookup work after the change,
   rather than becoming zero solely because an old function was removed.
2. Prepare finite old histories of 1/10/1000 genuinely reachable ancestry nodes,
   then measure a fixed added-node and fan-in workload. Include shared predecessors,
   duplicated equal content and pulse contribution multiplicity. Read counters
   before detached oracles. Old freeze visits and old-sized scratch/table rebuilds
   must be zero; new encodes/hashes and immediate relation/consistency work remain
   separately measured. No wall-clock or zero-total-work claim is required.
3. Exercise a valid zero-new-node frozen view/fork/selection (ordinary successful
   reactions usually add records). Require zero freeze work there; do not invent
   a zero-growth ordinary transaction. All-new restoration/replacement must show
   positive new-node work proportional to that actual new closure.
4. Compare per-record canonical bytes/digests and stable role bindings with the
   uncached creating-context reference; retain all P4/E4/O4 and inline/golden
   bytes. Exercise initialization/apply/forecast, multiple deadlines, qualification
   and removed/reset subjects, repeated checkpoint patching, source fact bytes,
   encoded restore, and subsequent patch-free replay/forecast.
   Capture source payloads before replacement and compare SOURCE facts with
   recomputation under an explicitly supplied source topology whose qualified
   slots differ from the target. Shared observed creating contexts alone cannot
   detect a consistently wrong source/target choice.
5. Inject equal-digest/different-payload cases into the private consistency
   boundary: new/new, new/old, shared/include and an earlier result-only root.
   Fatal rejection must not choose/overwrite content. Equal bytes may coalesce;
   growth and contribution occurrences remain charged/preserved. Exercise the
   OnceLock losing-set comparison if retained, or demonstrate exclusive completed
   construction and stable concurrent reads if that race is removed by design.
6. Hold results/inspections/episodes/forecasts across continuation, topology change
   and machine drop. Compare held/dropped machine roots, digests, growth and work;
   release owners and verify reclamation of lookup metadata as well as nodes.
   Reject after meaningful preparation and compare complete warm predecessor
   observations. Record the actually reached stages: extraction is early,
   provenance may fail during the first deadline, and publication faults occur
   after earlier deadlines/translation. Reuse real exact-limit/overflow tests.
7. Report required closure indexing/serialization, catalogue copying, predecessor
   ownership and logical growth separately. Retain executable clone, direct/
   stepwise, restored, replay and canonical controls. Clone preparation shares
   evaluation/publication; the uncached canonical reference shares grammar/domain
   constants and observed creating contexts, but not the candidate freeze or
   membership algorithms. Disclose these limits and supplement with exact vectors,
   negative consistency cases, raw membership/lifetime and measured work.
8. Later acceptance requires nonzero focused tests, `make check-dev`, then
   `make check-final`, whitespace/source audit and exact baseline vector diff.
   No gate, vector, oracle or logical budget may be weakened.

## Exclusions and preparation checks

No public/schema/profile/version changes, result-ownership narrowing, observable
digest or retention redesign, arbitrary history cap, global arena, unrelated
ownership, host dependency adoption, broad module refactor, new behavior or
follow-on slice. There are no unresolved product questions. Concrete private
lookup and helper spelling are implementation freedoms to test and record,
not planning blockers or reasons to create a contract.

Preparation validation:

- Selected contract source status: 390 unchanged, no other status.
- Contract tooling unit suite: 11 passed.
- All ten bead contract IDs/paths and named rule IDs resolve to reviewed records.
  Description matches the prepared body; evidence identifies the same bead.
- `ms-k1l` is open/unassigned, depending only on closed `ms-efv`.
- Tracked diff is empty; product/specification/vector/export diff against the
  accepted baseline is empty. The added planning document has no whitespace
  diagnostics (no-index diff exit 1 denotes the new file).
- HEAD is unchanged and the index is empty. The database has exactly one dirty
  issue; the author deliberately leaves its export to the coordinator.

This authoring pass claims no production tests or implementation gates. The only
repository file added is this document; the local bead database contains the new
task. No author export, stage, commit, close or push occurred.

## Independent planning acceptance

The coordinator reviewed the complete preparation and implementation bead,
checked all ten reviewed contracts' 390 cited source sections as unchanged,
and resolved every named governing rule. No contract, scope or design correction
was required. The source audit independently checked the retained-view freeze,
predecessor admission, initialization and ordinary builders, unfrozen checkpoint
migration prefix, validated restoration order, selected/shared membership,
creating-context reference and private-successor publication paths.

The verification design distinguishes removed freeze work from necessary
closure serialization, catalogue copying and genuine translation. It preserves
creating-topology bytes, canonical consistency, complete ancestry, independent
ownership, logical append budgets, atomic failure and strict continuation.
Concrete consistency-index representation remains private implementation freedom.
Planning validation passed the contract catalog/status checks, eleven contract
tooling tests, bead dependency/readiness/reference checks and whitespace checks.
The coordinator exports the accepted open bead and commits this planning pass;
implementation gates remain obligations of the separate assignment.
