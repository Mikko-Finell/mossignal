# Exact canonical membership selection preparation

Independently accepted planning from clean `main` at
`ac7837758e410f35048525ebebbdb62c602e57bf`, after independently accepted,
closed `ms-iri`. Generated bead `ms-qjr` remains open and unassigned until
the separate implementation assignment. Implementation requires the later
separate assignment. No active roadmap item is involved.

## Objective and contract basis

Reduce canonical content-membership rebuilding in `Records::owned_closure`
without changing P4/E4/O4, logical CauseRef membership, complete ancestry,
current roots, budgets, owned artifact lifetimes or persistence. The source
operation already selects only reachable nodes but reinserts every selected
frozen record into a new `ContentIndex`.

The following 11 contracts are reviewed and reused unchanged. Contract status
inspection at the preparation baseline reports 442 unchanged source citations
and no changed, missing, ambiguous or unfingerprinted sources. These exact
54 represented rules govern this slice; other represented rules and
`known_uncovered` facets do not expand it. No draft contract or specification
amendment is needed for the private algorithm choice.

| Contract ID | Path | Selected rule IDs |
| --- | --- | --- |
| `mossignal.identity.machine_state_digests` | `docs/specs/contracts/machine-state-digests.yaml` | `execution-projection`, `observable-extension`, `content-addressed-provenance`, `stable-order-independent-of-storage`, `excluded-non-semantic-state`, `current-root-version-profile`, `equality-is-not-validation`, `pure-machine-and-result-exposure`, `failure-leaves-digests-unchanged`, `digest-verification` |
| `mossignal.verification.optimized_paths` | `docs/specs/contracts/optimized-path-verification.yaml` | `executable-refinement`, `maintained-independent-references`, `optimization-evidence`, `complete-rejection-observation`, `limited-reference-independence`, `independent-algorithms`, `recompute-dynamic-invariants`, `preparation-fault-injection` |
| `mossignal.provenance.retention` | `docs/specs/contracts/provenance-retention.yaml` | `retained-root-closure`, `required-limits-are-operational`, `current-machine-root-manifest`, `independent-artifact-ownership`, `logical-growth-independent-of-reclamation` |
| `mossignal.inspection.semantic_projection` | `docs/specs/contracts/semantic-inspection.yaml` | `complete-owned-observation`, `retained-cause-ownership`, `pure-committed-projection`, `inspection-purity-verification` |
| `mossignal.provenance.causal_explanations` | `docs/specs/contracts/causal-explanations.yaml` | `structured-owned-support`, `authoritative-retention-boundary`, `explanation-verification` |
| `mossignal.diagnostics.persistent_episodes` | `docs/specs/contracts/persistent-diagnostic-episodes.yaml` | `owned-active-and-change-records`, `candidate-owned-atomic-state`, `committed-ordered-publication`, `ready-inspection-and-causal-retention` |
| `mossignal.persistence.machine_snapshot_artifact` | `docs/specs/contracts/machine-snapshot-artifact.yaml` | `self-contained-provenance`, `pure-creation`, `encoding-verification` |
| `mossignal.persistence.replay_artifacts` | `docs/specs/contracts/replay-artifacts.yaml` | `ordinary-fold`, `canonical-artifacts`, `temporal-and-module-equivalence`, `history-and-verification` |
| `mossignal.reconfiguration.atomic_replacement` | `docs/specs/contracts/atomic-topology-replacement.yaml` | `revision-provenance-and-episodes`, `current-source-topology-supporters`, `atomic-failure`, `finalization-verification` |
| `mossignal.runtime.transaction_forecast` | `docs/specs/contracts/transaction-forecast.yaml` | `shared-transition`, `source-unchanged`, `owned-forecast-result`, `agreement-and-failure` |
| `mossignal.persistence.machine_snapshot_restoration` | `docs/specs/contracts/machine-snapshot-restoration.yaml` | `current-root-version-validation`, `events-episodes-and-provenance`, `atomic-publication`, `round-trip-and-sufficiency`, `bounded-fault-injection` |

Task-scoped source discovery located the collision rule in the persistence
specification's section 60, “Digest equality and collision handling,” and
current ownership/retention in the runtime architecture's section 68. These
facets are already represented by the unchanged contracts above. Available
unequal canonical bytes under one digest remain fatal; equal hashes are not
permission to bypass validation when combining newly available content.

## Source invariant and construction proof

Normal `Records` has an exact node vector and CauseRef position map. Every node
has completed immutable canonical content. Its content index contains exactly
the distinct digests of those nodes and validated bytes for each digest.
Multiple logical causes may share a digest. The source proof is inductive:

| Path | Preservation of the invariant |
| --- | --- |
| `causal_store.rs::Records::new` | Empty nodes, positions and index establish the base. |
| `Records::push` | Resolves already-owned predecessors, freezes against the creating compiled topology and completed predecessor digests, then validates/inserts canonical content before publishing the node Arc and position. Equal content coalesces; unequal available bytes under one digest panic before publication. |
| `Records::clone` / `fork` | Copy exact node/position membership and share the immutable index; fork uses a new nonwrapping private allocation scope for future nodes. Existing causes and canonical content remain unchanged. |
| Current `Records::owned_closure` | Resolves every root, walks complete predecessor ownership, sorts source positions and builds an exact selected vector/map and selected-content index through checked insertion. Foreign roots reject. |
| `Records::include` | Inserts every incoming node's canonical bytes before CauseRef deduplication. Normal repeated CauseRefs name the same immutable node, or the equivalent test reconstruction, so their canonical content cannot add an orphan digest. Distinct scopes prevent normal reconstruction from aliasing an existing cause. Keep checks even for repeated causes. |
| `transaction.rs::build_initialization_provenance` / `build_ready_provenance` | Use new or fork followed by ordinary push; there is no unchecked canonical cache import. |
| `ProvenanceView::restored` and snapshot restoration | Remap decoded records into a fresh scope and ordinary push. Snapshot restoration subsequently includes active episode views and selects current roots through the same operations. |
| `checkpoint_migration` / SOURCE reconstruction | Select source/result roots, encode source facts, translate genuine different-lineage causes into fresh target checkpoint records and ordinary push. Source bytes are not reinterpreted against the target topology. |
| Test reconstruction | Append/duplicate-allocation helpers use ordinary fork/push. `reverse_supporters` reconstructs equivalent unordered support while preserving the same frozen digest/payload and predecessors, leaving the index valid. The deliberately corrupted incoming-node fixture is outside the normal invariant and must still fail at include; it is never a trusted selection input. |

`ContentIndex` is an immutable compressed binary digest trie. Branch bits
strictly increase from 0 through 255. Entries own only byte payload Arcs or
child entries, never a logical Node, parent `Records`, previous view or global
arena. Node predecessors independently own the complete ancestry and release
iteratively. Source inspection found no other normal `Records` initializer or
canonical-node mutation bypassing the paths above.

Selecting a subset of this validated index cannot introduce a new collision.
That observation applies only to existing trusted selection. Freeze/admission,
include, restore and different-lineage SOURCE construction continue validating
newly combined available bytes. No hash-equality shortcut replaces those checks.

## Concrete alternatives and chosen private freedom

Always reinserting selected records is the current conforming baseline, but
rebuilds leaves and paths for dense selection. Always cloning the parent index
for subsets retains unused content and is unacceptable. Always filtering the
entire source trie makes a fixed-small selection scan unrelated content.
A sorted-key/prefix selection could visit only selected paths, bounded by the
256-bit depth, but adds prefix/partition bookkeeping. The simpler route is:

Let `N` be source logical node count and `k` the selected logical count after
the complete root walk. Let `U <= N` be source unique digests, `s <= k` selected
unique digests and `e = U - s` excluded digests. Use actual cardinality:

1. Empty (`k = 0`): return an empty content index without scanning or admission.
2. Full (`k = N`): clone the validated source index. Complete selected logical
   membership proves every source digest is needed; content work is one root
   Arc clone, with no entry traversal or creation.
3. Proper dense (`0 < k < N` and `N - k <= k`): build a distinct selected-digest
   set during the selected-node pass, then filter the source trie exactly.
   Keep a leaf only when its digest is selected. Recursively filter both
   children; share the original branch only if both resulting child pointers
   are unchanged, promote a sole surviving child, or create a branch at the
   original bit when both survive with a change. Empty subtrees disappear.
4. Sparse (`N - k > k`): retain the selected-only checked insertion path.
   Do not scan the whole source index or catalogue.

The comparison avoids overflow and is a conservative structural work choice,
not a product history cap, cache or new policy. No extra persistent metadata or
second canonical store is needed. The implementer may refine private helper
names; material strategy or scope changes return to planning review.

Dense filtering visits at most `2U - 1 <= 4k - 1` entries, so its source scan
is proportional to required selected membership. Set preparation costs
`O(k log s)` and leaf queries `O(log s)`. It creates no leaves. A recreated
branch must be an ancestor of an excluded digest, so new branches are bounded
by `min(U - 1, 256e)`. Promotion preserves strictly increasing bits. Shared
subtrees contain only selected digests; an unchanged pointer is accepted only
after recursive exact membership checking.

Sparse content work/allocations remain `O(256k)` and independent of unrelated
`N`; selected-set/graph and map costs remain separate. The route counts logical
nodes because those are already known without an extra source scan. Duplicate
canonical content affects `U`, `s` and `e`, never logical multiplicity. A logical
subset needing every digest may legitimately share the full resulting index
after exact filtering.

The deletion boundary is exact selected digests and selected logical causes.
An index leaf may share the equal byte allocation first supplied by an excluded
logical node while a different selected node needs that same digest. This is
needed canonical content, and does not retain the excluded logical Node/view.
Do not require that equal payload allocation to die while the digest remains
needed. Unique excluded-digest payloads and nodes must release when their last
owner disappears; after the final selected owner drops, all associated node,
entry and payload ownership must return to the observed baseline.

## Accounting and finite implementation evidence

Use accepted `ms-iri` fixtures and actual accounting, with only minimal private
test-helper extraction if needed. Add failing reduction assertions first and
observe their failure on the reinsertion implementation before correction.
Use existing `membership_content_visits` and `membership_content_nodes_created`
around direct `owned_closure` calls: full genuine selection must create zero
entries, and nearly full genuine selection must demonstrate fewer entries than
the reinsertion baseline with exact membership and unchanged bytes/digests.
Choose the concrete nearly full fixture so the asserted reduction is supported
by its excluded paths, rather than claiming every dense subset must improve.

If phase attribution needs new cfg(test) fields/scopes, count only executing
content-selection calls, insert calls, entry visits, actual `Entry::new` and
actual branch reuse. Preserve existing global counter meanings and freezing
classification. Instrumentation may precede the failing performance regression,
but must not change selection behavior. Separate content-phase accounting from
the graph walk, freeze, include, result selection, fork copies, serialization
and hash work. Result subtotals overlap global totals; do not add them together.

Finite packet:

- Index unit tests against an independently assembled flat map: empty/full,
  sparse/proper dense and immediately below/at the cardinality threshold;
  equal canonical content from multiple causes; excluded sibling/unrelated
  leaves; every split bit 0 through 255 in both insertion orders; branch sharing,
  promotion and bounded allocations/visits. Collision insertion and corrupted
  incoming include remain fatal; foreign-root selection rejects without mutation.
- Reuse genuine ancestry history 1/10/1000 at width 1, plus history 10/1000 at
  width 4, for direct full/nearly full selection and actual warm ordinary empty
  and fixed-pulse publication. Independently use quiet controls at width 1,
  history 10/1000. Reuse the existing matrix rather than multiplying every axis.
  Keep all genuine reaction times and exact full reference inputs/digests.
- Fixed-small independent result and targeted inspection at history 1/10/1000
  as unrelated source population grows must keep selected-only work. Existing
  independent-pulse 1/3/3/6/6 roots/handles/records/content-visits/nodes remains
  a sparse control; empty result stays zero. A tiny full-source case may
  truthfully use zero content rebuilding, rather than manufacture positive work.
- Keep the ms-iri old-artifact held/dropped controls, both episode-view states
  at width 1/history 10, and a pending genuine scheduling root. Require complete
  exposed event and episode-change streams, current root-qualified membership,
  logical growth/budget charges, active episodes and pending/SOURCE obligations.
- Preserve independent uncached canonical/reference inputs, golden vectors,
  actual O4 encoding/hash bytes and cache-hit zeros. Old assertions demanding
  incidental reinsertion may become truthful work bounds; semantic and failure
  oracles cannot be weakened.
- Reuse all ten reached-stage complete warm rollback cases (ordinary and
  preserving patch), including raw lookup identity, complete predecessor
  observation and digests. Preserve detached results, inspections, explanations,
  forecasts and snapshot/replay/restore behavior with source/target semantics.
- Observe live/weak node, trie and payload ownership with shared branches and
  excluded leaves: dropping the source releases excluded unique content while
  a selected artifact remains complete; multiple selected owners retain only
  needed content; final-owner release returns to baseline. Include long genuine
  ancestry and depth-256 release controls; no parent-view back-reference.

New selection tests should use the `content_selection_` name prefix. Future
implementation runs exact new tests first and confirms a nonzero match. The
focused acceptance command is:

```bash
cargo nextest run -p mossignal --all-features -E 'test(content_selection_) | test(causal_store::) | test(cost_measurements::) | test(state_digest::tests::) | test(incremental_freezing_) | test(result_ownership_) | test(ordinary_remap_) | test(edge_observation_) | test(preparation) | test(private_clone) | test(forecast) | binary(edge_detectors) | binary(temporal_foundation) | binary(transaction_forecast) | binary(replay_artifacts) | binary(practical_inspection) | binary(diagnostic_inspection)' --success-output immediate --failure-output final
make check-dev
make check-final
```

Record test counts and require zero skipped, unavailable, warning, timeout or
flaky mandatory outcomes. These are future implementation checks, not claims
that planning ran Rust gates or measured performance.

## Limits, change surface and preparation checks

Accepted `docs/observable_digest_cost_implementation.md` reports aggregate
membership visits 178859/178895 and created entries 88893/88914 for genuine
history 1000/width 4 ordinary warm empty/pulse admission. They include several
phases and are not a standalone closure-selection baseline. Attribute actual
selection first before claiming any saving. No latency or CPU dominance claim
follows from entry counts. Complete O4 encoding/hash, selected graph traversal,
selected vector/positions rebuilding and ordinary fork's full catalogue/map
copies remain. Disclose key-set/filtering overhead and any fixture limitations.

Future code scope is `causal_store.rs`, `causal_store/content_index.rs` and
cfg(test) `causal_work.rs` attribution. Tests may extend existing causal-store,
state-digest cost, transaction remapping/preparation/ownership and artifact
coverage, with minimal private fixture reuse. Evidence belongs in one concise
implementation report. No record-catalogue/vector/position-map redesign,
public index API, generic persistent collection, dependency, projection/version
change, history cap/pruning/checkpoint policy, compatibility shim/freeze, host
pin/adoption, unrelated work or retired-roadmap claim is included.

Planning checks: clean accepted baseline and closed prerequisite inspected;
contract catalog/status and exact selected rules checked; source creation paths
and collision/ownership boundaries inspected; locked Python contract tooling
tests passed 11/11. Exact rule/source validation confirms 11 reviewed records,
54 rule IDs and 442 unchanged citations; the bead's contract table matches this
document. Text whitespace, generated bead state/prerequisite/readiness and
protected-surface checks pass. No product, test, instrumentation, specification, contract,
golden, generated artifact, Cargo, Make or verification-policy edits occurred.
Preparation leaves one open/unassigned bead, with no claim, closure, export,
staging, commit or push. No blocking product question was identified.

Independent coordinator review accepts the exact membership invariant,
construction/include/reconstruction proof, full/dense/sparse route and work
bounds, collision obligations, physical-versus-logical ownership distinction,
finite regression/ordinary-publication/lifetime evidence and exclusions without
a design, contract or scope correction. Eleven reviewed records, 54 selected
rules and 442 unchanged source citations were independently verified. All
eleven contract-tool tests and primary bead/document basis, cardinality/bounds,
dependency/readiness, exact preparation-only surface, fences and whitespace
checks passed. No Rust gate, new measurement or implemented saving is claimed
by planning. The coordinator exports and commits this accepted preparation;
implementation remains a separately assigned proposal for corrective acceptance.
