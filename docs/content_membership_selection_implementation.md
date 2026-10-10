# Exact canonical membership selection — ms-qjr

Independently accepted on 2026-10-11 from accepted planning `main`
`9c497768fcb953c56a0b4637c405cc9b83ed17f2`. Coordinator source, contract,
ownership, work and scope review required no code, contract or scope correction.
The coordinator closes, exports and commits this accepted implementation locally;
library distribution and host dependency adoption remain pending.
The accepted scope and source proof are in
`docs/content_membership_selection_preparation.md`. Its 11 reviewed contracts,
54 selected rules and 442 unchanged citations remain unchanged; no draft,
normative amendment or material scope expansion was needed.

## Implementation and ownership

`Records::owned_closure` still resolves every root, walks complete predecessor
ownership, preserves source/topological order and builds exact selected logical
nodes/positions. It chooses content membership by actual logical cardinality:

- Empty: empty index.
- Full: share the already validated exact source index.
- Proper dense, overflow-safe `N - k <= k`: gather selected distinct digests
  during the selected-node pass, recursively filter the source trie, discard
  excluded leaves, promote sole surviving children and rebuild only changed
  surviving branches at the original split bit. Share a branch only after both
  filtered child pointers prove unchanged exact membership.
- Sparse: keep selected-only checked insertion without scanning the source trie
  or catalogue.

Dense entry visits are at most `2U - 1 <= 4k - 1`; filtering creates no leaves
and at most `min(U - 1, 256e)` branches, where `e` counts excluded digests.
Digest-set preparation/query costs remain `O(k log s)` / `O(log s)`. Sparse
content work stays bounded by the selected population and 256-bit trie depth,
independent of unrelated source population. This is a private algorithm choice,
not a new product policy or history cap.

Entries own bytes and child entries only. No node, previous view, parent
catalogue or extra persistent metadata is introduced. Logical multiplicity is
preserved separately from canonical coalescing. A source leaf may retain an
equal byte allocation first supplied by an excluded logical node while a
selected distinct cause still needs that digest; the excluded node itself
releases. Unique excluded bytes release with their final owner. Tests observe
source drop, multiple selected owners and final node/entry/payload release.

Trust is limited to the existing validated immutable source index. New
push/freeze/include admission, repeated incoming causes, snapshot reconstruction
and genuine different-lineage SOURCE translation keep their existing
available-byte validation. Collision tests remain fatal without overwrite.
Production changes are confined to `causal_store.rs` and its private
`content_index.rs`; node/cause representation, graph selection and callers retain
their semantics.

## Actual-boundary evidence

Two genuine width-1/history-10 regressions were added and run before behavior
changes. Both failed on the accepted reinsertion path:

| Direct selection | Required logical / canonical records | Visits before → after | Entries before → after |
| --- | --- | --- | --- |
| Full | 55 / 45 | 480 → 0 | 251 → 0 |
| Nearly full | 54 / 44 | 469 → 89 | 245 → 4 |

Test-only `content_selection` attribution counts actual content-phase calls,
route branches, digest-set insert attempts, checked insert calls, entry visits,
`Entry::new`, leaf/branch sharing and promotion. It preserves existing global
counter definitions. A scope guard restores attribution through unwinding.
Graph visits remain separate. Oracle/setup reads occur outside captured windows.

With instrumentation alone and the old algorithm, the three existing ms-iri
fixture tests passed and captured 34 ordinary warm publications. After the
change, the same 34 projection structs agree field-for-field, including every
full E4/O4 byte, traversal, coalescing and hash count. Selected-set insertion
attempts are additional disclosed work:

| Warm case | Selection entry visits before → after | Selection entries before → after | Digest-set attempts after |
| --- | --- | --- | --- |
| Quiet width 1, history 1000, empty | 30 → 21 | 20 → 2 | 7 |
| Quiet width 1, history 1000, pulse | 36 → 29 | 28 → 10 | 8 |
| Genuine width 1, history 1000, empty | 100370 → 8019 | 47545 → 10 | 5010 |
| Genuine width 1, history 1000, pulse | 100407 → 8027 | 47567 → 16 | 5011 |
| Genuine width 4, history 1000, empty | 178859 → 14025 | 88893 → 11 | 8013 |
| Genuine width 4, history 1000, pulse | 178895 → 14033 | 88914 → 17 | 8014 |
| Episode inactive, width 1/history 10, empty | 68 → 33 | 45 → 7 | 12 |
| Episode active, width 1/history 10, empty | 67 → 33 | 44 → 8 | 12 |

The primary genuine cells happen to have all nonfreezing membership work inside
selection, as actual attribution establishes; this is not assumed for other
paths. For the active episode cell, global membership is 86 → 52 visits and
44 → 8 entries: 19 visits still validate incoming episode bytes at include.
Result selection overlaps content selection and global membership totals.
Existing freeze consistency hooks classify their visits/allocations separately
with `is_freezing`; do not add unlike or overlapping quantities into a saving.

Independent pulse results retain exactly 1/3/3/6/6
roots/handles/records/content-visits/entries; empty result work remains zero.
A targeted quiet edge beside growing genuine Toggle ancestry selects six
records at histories 1/10/1000 while its source grows 13/58/5008. Actual selected
content visits are 21/23/20 and entries 16/17/15; all three use sparse insertion.
The small scalar-dependent variation is bounded by selected membership.

Additional direct full/nearly full controls cover genuine history 1/10/1000 at
width 1, history 10/1000 at width 4, and quiet width 1/history 10/1000. Exact
selected cause/content maps, independent canonical reference bytes and every
genuine ready-reaction time remain present. Current snapshots and uncached
machine inputs remain unchanged across direct selection. Nine new tests cover
these matrices, all 256 split bits in both orders, threshold neighbors,
coalescing, excluded leaves, exact branch sharing/promotion, collision checks,
foreign-root rejection, physical release and shared depth-256 final release.

All existing ms-iri old-artifact held/dropped and both episode controls are
retained. Existing pending scheduling, full event/episode streams, logical
growth/budgets, ten reached-stage warm rollback cases, raw lookup identity,
SOURCE, detached artifact ownership and snapshot/replay/restore controls remain.
No old semantic, failure or vector oracle was weakened; the existing sparse
result accounting assertion was strengthened to exact observed work.

## Verification and limits

Independent coordinator verification reproduced all 34 warm rows exactly,
including every complete before/after projection field, all 14 direct controls,
three small inspections, two reduction witnesses and ten complete warm rollback
cases. The independent focused run passed 145 selected tests, with 517 outside
the filter. Independent `check-dev` then `check-final` each passed 662/662 with
zero skipped, warning or leaky results. All eleven reviewed contracts, 54 rules,
442 unchanged citations and eleven locked contract-tool tests were confirmed.
The worker-tested product hashes stayed unchanged through these gates; only
this report was reconciled to acceptance. Commands/results and reproduced work
are retained in `/private/tmp/ms-qjr-coordinator-gates.json`,
`/private/tmp/ms-qjr-coordinator-evidence.json` and their focused/dev/final logs.

The final focused run passes **145 selected tests**, with 517 deliberately
outside the filter and zero warnings/leaky results. It confirms all nine new
tests, 14 direct full/nearly full controls, three small inspections, two
regression controls and all ten complete warm rollback cases. `make check-dev`
passes with **662/662** mandatory Nextest tests and zero skipped, warning or
leaky results. The subsequent `make check-final` also passes **662/662** with
zero skipped, warning or leaky results, including strict Clippy, static
guardrails, locked tooling tests, doctests, documentation and dependency policy.
Source hashes remain unchanged across these checks. Final whitespace, exact
five-file proposal scope, excluded surfaces, unstaged state, accepted HEAD,
in-progress bead and closed prerequisite audits pass.

The initial narrow failing-before run selected two tests and failed both. An
intermediate compile exposed two unused test helpers before their oracle
consumers were connected; they are used in the final tests. Strict Clippy found
two `int_plus_one` assertions; equivalent strict inequalities fixed them without
weakening either bound. A nine-test run and the first 145-test focused run each
reported one leaked-handle result, so neither is treated as a clean validation
pass. Final verification uses `NEXTEST_TEST_THREADS=1`, preserving test selection,
arguments, default timeout/leak policy and all semantic oracles.

Exact before/after regression, old-path attribution and focused commands:

```bash
cargo nextest run -p mossignal --all-features -E 'test(content_selection_full_genuine_closure_does_not_rebuild_content) | test(content_selection_nearly_full_genuine_closure_reuses_content)' --success-output immediate --failure-output final
cargo nextest run -p mossignal --all-features -E 'test(unchanged_digest_cost_)' --success-output immediate --failure-output final
NEXTEST_TEST_THREADS=1 cargo nextest run -p mossignal --all-features -E 'test(content_selection_) | test(causal_store::) | test(cost_measurements::) | test(state_digest::tests::) | test(incremental_freezing_) | test(result_ownership_) | test(ordinary_remap_) | test(edge_observation_) | test(preparation) | test(private_clone) | test(forecast) | binary(edge_detectors) | binary(temporal_foundation) | binary(transaction_forecast) | binary(replay_artifacts) | binary(practical_inspection) | binary(diagnostic_inspection)' --success-output immediate --failure-output final
```

Existing temporary
advisory-cache relocation and pass-through gate-output capture are reused.
Parsed deny policy equals repository policy except for the writable database
path; normal fetch remains enabled. Gate environment uses
`PATH=/private/tmp/ms-iri-verification/bin:$PATH`,
`UV_CACHE_DIR=/private/tmp/mossignal-planning-uv-cache`,
`NEXTEST_TEST_THREADS=1` and `MS_IRI_GATE_NEXTEST_LOG` pointing to this bead's
separate dev/final Nextest logs. No repository gate/policy/configuration changed.

```bash
PATH=/private/tmp/ms-iri-verification/bin:$PATH UV_CACHE_DIR=/private/tmp/mossignal-planning-uv-cache NEXTEST_TEST_THREADS=1 MS_IRI_GATE_NEXTEST_LOG=/private/tmp/ms-qjr-dev-nextest.log make check-dev
PATH=/private/tmp/ms-iri-verification/bin:$PATH UV_CACHE_DIR=/private/tmp/mossignal-planning-uv-cache NEXTEST_TEST_THREADS=1 MS_IRI_GATE_NEXTEST_LOG=/private/tmp/ms-qjr-final-nextest.log make check-final
```

Remaining work includes selected-node canonical checks, full graph traversal,
selected vector/map construction, ordinary fork's full catalogue/position copies,
digest-set preparation and filter lookups, required full O4 payload serialization
and hashing. Warm width-4/history-1000 E/O hash bytes remain
2301/2117285 (empty) and 2301/2117520 (pulse). Setup keeps an equivalent
predecessor base uniformly alive, as in ms-iri; these intervals do not measure
complete predecessor reclamation or allocator latency. Canonical references
share grammar/projection definitions, supplemented by independent handle walks,
flat maps, trie-shape checks and physical lifetime controls. No elapsed-time,
CPU dominance, throughput or constant-total-work conclusion follows.

Evidence logs use `/private/tmp/ms-qjr-`: `regression-before.log`,
`regression-after.log`, `selection-tests.log`, `selection-final.log`,
`selection-accounting-before.log`, `selection-before.json`, `focused.log`,
`focused-final.log`, `clippy.log`, `contract-status.json`, and the dev/final gate
logs, `selection-after.json`, `selection-comparison.json`, `direct-evidence.json`,
`code-hashes.json` and `gates.json`. The retained clean focused log is
`focused-final.log`; both comprehensive Nextest logs confirm 662/662, zero
skipped. Evidence extraction and the final scope audit accompany the coordinator
report. The earlier skip-word audit mistakenly rejected the literal “0 skipped”
line; parsing its numeric value confirms the clean development result.
