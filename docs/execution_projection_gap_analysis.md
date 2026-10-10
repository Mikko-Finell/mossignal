# Remaining execution projection work after ms-0ap.3

Preparation/audit evidence at accepted `main`
`dba9020406b89871c75e7d5f31d390112e059f73` (2026-10-10). The starting tree was
clean; `ms-0ap.1`, `.2`, `.3` and design review `ms-hks` were closed. The delivery
epic remains open. This is a planning proposal, not implementation acceptance or
normative authority.

## Disposition of the original fourth slice

| Planned facet | Current implementation evidence | Disposition |
| --- | --- | --- |
| Root-selected owned closures | `causal_store.rs::Records::owned_closure` visits selected roots and predecessors; `transaction.rs::publish_candidate` publishes typed current roots. Results/episodes/inspections own independent closures. | Delivered in slices 1–2. |
| Cached canonical causal content | `causal_store.rs::Node::canonical` freezes payload/hash under its creating topology; `state_digest.rs::digest_record` reads those cached values. | Delivered; old causal records are not re-encoded/rehashed on ordinary reactions. |
| Root-selected observable/snapshot output | `state_digest.rs::cause_digest_index` and `write_provenance` emit the deduplicated canonical root closure; `persistence.rs::write_provenance` writes those records plus stable role associations. External artifact owners are not roots. | Correctness/integration delivered. Index/table construction remains repeatable work. |
| Strict snapshot restoration | `snapshot_restore.rs::restore` checks state, pending references, episodes, graph, settlement, re-encoded causal bytes and all digest claims before returning a machine. | Delivered; caches must preserve these checks. |
| Migration | `transaction.rs::checkpoint_migration` selects SOURCE plus result-required closure, translates source facts, and gives TopologyChange exactly deduplicated SOURCE content identities. Result-only roots do not become patch supporters. | Delivered, including v3 semantics and stable creating-topology bytes. |
| Forecast and replay | `Machine::forecast` stages the ordinary transition; `replay.rs::apply_sequence` folds ordinary apply with identity checks. | Delivered. Their repeated identity queries inherit the remaining projection cost. |
| Whole-machine E/O memoization | `machine.rs` digest getters directly call `state_digest.rs`, which builds canonical input and hashes it every time. Neither Machine nor MachineStore stores completed E/O values. | Still absent. |
| Execution projection independent of full ancestry traversal | `projection_payload` always builds `ProjectionContext`, including `index_view` and `reachable_digests`, even with `observable=false`. | Still absent; execution needs current cause identities, not record payload/closure enumeration. |
| Cross-feature refinement/cost acceptance | Three accepted reports and maintained reference/fault/cost tests already cover ownership, migration, formats, bindings and staging. Existing pure-query counters measure causal encoding, not whole projection construction/hash work. | Reuse delivered evidence; add the narrow missing measurements and cache/refinement matrix. |

The remaining native implementation is therefore one bounded digest preparation
and memoization task, with final cross-feature acceptance. The older fourth-slice
wording does not justify another format, migration, replay or retention rewrite.
Prepared child `ms-0ap.4` is open and unassigned, depends on accepted `ms-0ap.3`,
and awaits independent coordinator review before implementation.

## Avoidable work and costs that remain legitimate

`ProjectionContext::build` allocates per-view digest vectors, memo/stack buffers,
a canonical content table and reachable sets. It indexes the machine view and
each active episode view, then walks roots. Cached node content avoids causal
encoding/hashing but does not avoid these structures or traversal. Execution
projection builds them even though it never writes full provenance records.

Both public digest queries subsequently encode their full canonical input and
hash it again. Result construction requests after E/O; predecessor capture and
freshness/forecast/replay checks request E; snapshot creation requests E/O and
then independently builds its payload causal index. Restoration similarly
re-encodes graph content and recomputes E/O. Those calls currently rebuild
projection data, regardless of whether the committed machine changed.

A private completed digest pair can remove repeated pure-query reconstruction.
An uncached execution encoder can resolve each represented current cause through
its owning view's frozen canonical content without enumerating ancestry. Its
work still follows represented state, pending entries and current role bindings;
equality lookup and state sorting are not promised constant time.

Observable and snapshot **emission** still requires complete relevant canonical
records, sorting/coalescing and bytes. Required Toggle/phase ancestry and held
artifacts may grow. Current-view catalogue/member copying, source checkpoint
wrapping, one initial staging copy, evaluator settlement and required artifact
encoding remain legitimate documented work. This task does not redesign those
representations, memoize full snapshot artifacts, prune ancestry, add checkpoints
or introduce a second evaluator. A per-emission closure index may still be built.

## Bounded implementation and validation proposal

Prepare E/O values from each complete immutable semantic version: declared
AwaitingInitialization state, successful initialized/advanced/patched successor,
validated restored state, and hypothetical forecast state. Preserve original
predecessor identities before private extraction; invalidate/rebuild candidate
cache state at the coherent transition boundary. Never read a predecessor's
cached value as a successor identity or publish a partial pair. Restore must
derive values from validated state/content, not import artifact digest claims.
No cache owns extra causal roots or a previous-machine chain.

Keep the raw canonical E/O encoders callable independently of memoized getters.
Extend `state_digest_reference::assert_machine` to check stored/getter identities
against uncached recomputation as well as complete input bytes. Existing tests
that deliberately edit internal semantic state must explicitly invalidate or
refresh private caches; their sensitivity and hostile checks cannot be weakened.
Private cache layout/eager preparation helpers are implementation freedom.

| Finite case family | Required agreement and cost evidence |
| --- | --- |
| Declared/initialized/ready state; zero/positive pulses and ordered equal time | Cached E/O equal uncached reference and fixed inputs/goldens; repeated public queries perform zero projection builds, outer digest hashes, content-table indexing or closure visits. |
| Stateless histories 4/40, held/dropped results/inspections/forecasts | Preserve bounded current ownership and old-view isolation; zero old causal rewrite/hash, zero repeated binding proof/key copies and one staged store; artifact holdings do not affect cache values or projection work. |
| Growing Toggle/Periodic ancestry at 4/40 | A forced uncached E computation resolves only represented facts and visits/indexes/emits zero closure records; forced uncached O and snapshot still emit exact full closure and increasing required bytes. |
| Existing nine mixed graphs and twelve counted histories | Cover temporal families, all three stateful standard modules, qualified facts, cancellation and phases; compare complete results, state, schedules, episodes, causal content and future continuation. |
| Preserve/reset/remove patches, deadlines before/exactly at patch time, role swap and coalesced allocations | Preserve SOURCE manifest, result source/target identities, role-sensitive E/O and two-patch direct/restored continuations; cache refresh precedes joint bound publication. |
| Snapshot/restore/replay/forecast | Exact canonical bytes and strict hostile rejection; restored/forecast query values are prepared independently; ordinary replay chain and partial-progress failures retain their semantics. |
| Warm-cache failure, digest-preparation fault and late bound output rejection | Complete source fields, cached pair, snapshot, roots/ownership, cursors and retained views unchanged; include both lifecycle phases and earlier-deadline patch work. |
| Existing below/exact/above budgets | Cached/uncached query history cannot alter logical append charges, runtime-policy identity or success/failure; preserve the 33-append periodic and zero-normalization boundaries. |

Add counters at the actual projection builder, canonical-index insertion/visit,
cause resolution and outer machine-digest hash operations. Existing causal-work
and execution-work counters continue to measure causal hashes, closure output,
ownership and staging/bindings. Isolate production measurements from reference
recomputation. Observe failing cost regressions before implementation; do not
substitute cache-hit/helper-call counts or wall-time thresholds for removed work.

Reference independence remains bounded: the detached Vec/map encoder independently
enumerates roots/roles and recomputes graph bytes/hashes without production node
or machine caches. The copy-preparation reference independently varies ownership
preparation but shares evaluation/publication. CBOR primitives and snapshot field
writers are shared. Existing fixed vectors, hostile resigned artifacts, law
oracles and direct/restored/replayed continuation tests supplement those limits;
no independent second production runtime is proposed.

## Contract reuse and audit checks

Reuse unchanged reviewed machine-state-digests, provenance-retention,
optimized-path-verification, machine-snapshot-artifact/restoration,
transaction-forecast, replay-artifacts, atomic-topology-replacement,
semantic-inspection, causal-explanations, application-bindings and live-bindings.
All 462 cited source entries in those twelve selected records are unchanged. No
unrepresented fundamental semantic facet or conflicting requirement was found;
no contract or specification change is proposed.

The separately inspected older runtime-policy record has two changed citations.
Current runtime budget section 91 is already represented by the unchanged
reviewed retention contract; it explicitly excludes cache hits from growth
accounting. The current catalogue still has the same missing/invalid policy-limit
mapping. Policy construction/identity revision is outside this task. Its old
fingerprints were not refreshed or relied on to override the current rules.

Read-only audit reran the existing sharing/ordinary-advance and growing-ancestry
cost tests, generated mixed-graph reference test, and contract-tool unit tests.
Results are recorded in `/private/tmp/ms-0ap4-audit-*.log`. This planning pass
does not claim a new full repository acceptance gate. The implementation bead
requires focused regressions, `make check-dev`, then `make check-final` before
independent acceptance. No implementation, epic closure, commit, flush, push
or Mossmoot change is included.

## Independent planning acceptance

The coordinator accepted this bounded proposal on 2026-10-10 after inspecting
the current raw projection and digest-query paths and reviewing the complete
ms-0ap.4 scope and verification matrix. Existing unchanged reviewed contracts
are reused. No semantic, schema, public API or contract change is required.
Implementation remains subject to independent acceptance and the stated gates;
this approval is not implementation or delivery-epic completion.
