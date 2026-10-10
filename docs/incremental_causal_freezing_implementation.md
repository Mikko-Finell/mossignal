# Incremental causal freezing implementation proposal

Implementation of `ms-k1l` from accepted planning commit
`8ab55a1eb836db0978efbca4f2aabab7d21be109`, independently accepted by the
coordinator on 2026-10-10. This record is evidence for the bounded optimization,
not normative specification.

## Implementation and preserved behavior

`Records::push` receives the creating compiled topology. It validates and owns
the record's actual predecessor occurrences, reads their completed content,
encodes/hashes only the new record and checks graph-local digest/payload
consistency before constructing its shared node. Every production constructor
now fills canonical content before `Arc<Node>` exists. Test constructors and
supporter-permutation reconstruction preserve the same invariant. OnceLock
remains as a private container; there is no shared late initialization or losing
set to reconcile. Concurrent readers see completed immutable content.

The private consistency index is a concrete compressed binary radix tree over
the 256 digest bits. Forks share its immutable root and unchanged paths;
insertion copies only the search path. Leaves own payload Arcs, not causal nodes,
catalogues or source topologies. Different bytes for an available equal digest
remain fatal before insertion, including shared old and result-only facts.
Equal bytes reuse an existing leaf without eliminating the logical allocation.
Branch bits strictly increase, so traversal and release depth are bounded by
the fixed key width. No dependency or generic persistent-container framework
was added.

Selection builds exact lookup membership inside its existing selected-node
visit. Include checks content inside its existing other-member visit. Selection
does not inherit a lookup holding excluded nodes' payloads. These operations
have separately measured metadata costs; they are not free.

The whole-view freeze, memo/stack/result allocation and old source-context sweep
are removed. Initialization, each source deadline, ready target settlement,
pending scheduling, checkpoint wrappers and restoration use eager admission.
Opaque SOURCE facts are computed under the source topology; newly constructed
target records use the finalized topology. Production retains no compiled source
graph for old nodes. Creating contexts are captured only at admission under
`cfg(test)` for the existing uncached oracle.

Required observable indexing, snapshot cause indexing, selected SOURCE fact
encoding and their ContentTable consistency checks remain. P4/E4/O4, canonical
grammar and domains, complete ancestry, root selection, current/transition edge
roles and both appends, logical growth and runtime budgets are unchanged.
Restoration validation and genuine SOURCE/private-handle translation remain.
No specification or contract changed; the ten reviewed contracts selected in
the preparation are reused at their unchanged evidence baseline.

## Regression-first work evidence

Before changing production behavior, actual old freeze loops and allocation
lengths were instrumented. Two zero-old-work regressions ran all three finite
samples, preserved their semantic assertions and then failed on old traversal:
**0 passed, 2 failed**. The old implementation already encoded only three new
nodes; that alone did not establish bounded freezing.

The isolated fixture retains a complete 1/10/1000-record chain using private
opaque checkpoint facts, then adds exactly three records with seven predecessor
occurrences. Two new allocations have identical canonical content. The opaque
facts isolate storage work; they are not presented as valid encoded snapshots.

| Measured old work | 1 old record | 10 old records | 1000 old records |
| --- | ---: | ---: | ---: |
| Fixed addition: old records indexed | 1 | 10 | 1000 |
| Fixed addition: memo/stack/result slots | 12 | 39 | 3009 |
| Fixed addition: cached payload visits and insertions, each | 1 | 10 | 1000 |
| Fixed addition: source-context visits | 1 | 10 | 1000 |
| Zero-new frozen view: scratch slots | 3 | 30 | 3000 |
| Zero-new frozen view: old visits, cached visits/inserts and source visits, each | 1 | 10 | 1000 |

After eager admission, every old-freeze field above is zero. The replacement
counts actual encoding, immediate reads and concrete index visits/allocations.
Measurements are captured before detached observers or selected-closure work.

| Fixed three-node addition after correction | 1 | 10 | 1000 |
| --- | ---: | ---: | ---: |
| New encodes / hashes / nodes | 3 | 3 | 3 |
| Immediate predecessor reads / owned occurrences | 7 | 7 | 7 |
| New-content consistency lookups | 3 | 3 | 3 |
| Consistency path visits | 7 | 23 | 49 |
| Consistency tree entries created | 4 | 11 | 21 |

Lookup work varies with branching; the test requires the fixed-width bounds
of at most 513 visits and 258 new tree entries per new key. There is no
constant-total-work or wall-clock claim. Catalogue membership copying remains
outside the measured admission interval and retains its existing counters.

A second fixture initializes a genuine network with 1/10/1000 All operations,
each with shared fan-in. Its retained catalogues contain 4/13/1003 records. Adding
one new record with two immediate occurrences gives one encode/hash/lookup and
two reads/owned occurrences in each sample, with 7/8/18 path visits and 4/5/10
tree entries created. Old content remains unchanged and reachable after machine
and result drop; final causal-node and lookup-entry counts return to zero.

The valid zero-new fork/selection has completely zero FreezeWork. Its existing
selected membership visits are 1/10/1000; the chosen index adds lookup path visits
of 0/47/18097 and creates 1/32/9870 tree entries during those visits. This
explicitly exposes selection's added lookup reconstruction cost instead of
calling the complete operation constant time.
No zero-growth ordinary reaction was fabricated.

The qualified source patch sample processes deadlines before target settlement:
268 new nodes/encodes/hashes and consistency lookups, 646 immediate reads/owned
occurrences, 3627 consistency path visits and 2010 new index entries. Separately,
it charges 5408 membership index visits, 3059 membership entries created, 475
catalogue handles copied, 443 ownership-closure visits and 302 genuine SOURCE
translation lookups. Logical growth is 162; required observable closure visits,
records emitted and bytes emitted are 306, 304 and 138368. These categories are
retained work, not removed freezing. All-new encoded restoration also has
positive new-node work equal to its actual admitted closure.

## Controls and limits

Nine focused incremental-freezing tests cover isolated and genuine ancestry,
zero-new selection, completed concurrent reads, real shared pulse contributions,
explicit source qualification, index structure, collision rejection and lookup
reclamation. The index test compares an independent flat map for all 256 split
bits in both construction orders, verifies unchanged leaf sharing and forces
equal-key/different-byte rejection without overwriting the root. Include checks
a corrupt new record against an earlier result-only fact absent from selected
machine roots, then demonstrates excluded payload/index reclamation.

The qualified fixture explicitly recomputes source bytes with observed creating
contexts disabled, verifies qualified subjects occupy different target slots,
finds each retained standard-role payload verbatim in SOURCE facts, preserves
held source content, round-trips the target and compares subsequent ordinary
continuation. The original six ordinary-remap controls now also require zero
old freeze work, positive real restore admission, final lookup release and
unchanged lookup counts across ten reached-stage rejection cases. Extraction
fails early; provenance can fail at the first deadline; publication faults occur
after earlier deadlines and, for patch cases, translation.

Existing edge append/growth limits, clone preparation, strict restore corruption,
direct/stepwise, forecast/replay and inline/golden canonical controls remain.
Clone preparation shares evaluation/publication. The uncached vector reference
shares grammar/domain constants and normally observed creating contexts, while
using its own recursive content and membership algorithm. Explicit-source
recomputation supplements that context limitation. Exact vectors, negative
consistency, raw identity/lifetime checks and actual work counters supplement
shared semantic code; none constitutes an unbounded proof.

The obsolete counters instrumented the removed loops before correction. The
replacement's real boundaries account for new freezing and membership lookup
work separately. A future unrelated whole-view walk requires an appropriate
phase counter and source audit; permanently zero fields alone are not a proof
that arbitrary future implementation changes preserve these costs.

## Verification record

Final proposal checks are recorded below after completion. Logs use the
`/private/tmp/ms-k1l-` prefix. The existing verification environment places the
locked uv cache and cargo-deny advisory database under `/private/tmp`; the
temporary cargo wrapper changes only the advisory database path, with normal
fetch enabled and identical deny policy. Repository gates and configuration
are unchanged.

- Regression first: `/private/tmp/ms-k1l-before.log` (two expected failures).
- Initial correction: `/private/tmp/ms-k1l-after.log` (two passes).
- Focused freezing: nine passes, `/private/tmp/ms-k1l-focused.log`.
- Ordinary remap: six passes, `/private/tmp/ms-k1l-remap-controls.log`.
- Edge provenance and restoration: ten passes,
  `/private/tmp/ms-k1l-edge-controls.log`.
- Clone preparation: two passes, `/private/tmp/ms-k1l-clone-controls.log`.
- Selected integration suites: 114 passes across atomic replacement,
  reclamation, diagnostics, edges, ordered reactions, replay, standard stateful,
  temporal execution and forecast,
  `/private/tmp/ms-k1l-focused-integrations.log`.
- `make check-dev`: passed after the last lint correction,
  `/private/tmp/ms-k1l-check-dev.log`. The first invocation found a misplaced
  test-module cfg guard, which was corrected.
- `make check-final`: passed, `/private/tmp/ms-k1l-check-final.log`. Its first
  invocation found the restoration helper's leftover return binding after freeze
  removal. The binding was simplified; no gate policy changed.
- Exact baseline vector/spec diff and translation/constructor source audit:
  no grammar, domain/version, golden, normative source or strict validation
  changes; `git diff --check` passed.

## Independent corrective acceptance

Coordinator review checked the complete implementation and regression surface,
including every eager admission context, immutable index insertion and branch
ordering, selected/include lookup membership, complete concurrent reads,
SOURCE wrappers, uncached explicit-source reference and strict restoration.
No production, verification or scope correction was required. All ten reviewed
contracts' 390 source sections remain unchanged; no draft or permanent private
representation promise was introduced.

Independent verification passed fifteen focused tests (nine incremental-freezing
and six ordinary-remap controls), all six transaction measurement fixtures,
`make check-dev`, then `make check-final`, exact baseline specification/golden
comparison and whitespace/source audits. The ten measurement records reproduce
the tables above, including 1/10/1000 histories, positive qualified replacement,
zero old freeze work, actual bounded new-node lookup work and separately charged
selected-lookup reconstruction. This remains a freeze-work claim; required
closure encoding, catalogue copying and genuine translation still perform work.
Logs are under `/private/tmp/ms-k1l-coordinator-`. The independently parsed
temporary deny configuration preserves the repository policy except the
writable advisory database path; fetch is enabled and repository gates are
unchanged. Final closure, synchronized bead export and the accepted product
commit belong to the coordinator. No push or host dependency adoption occurred.
