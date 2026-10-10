# Result-specific transaction provenance implementation

Independently accepted implementation for `ms-am9` on 2026-10-10, from accepted
planning `main` at `ff39ce6fe5fa7f94cf4543963dc4ff5d4c9b9789`. Coordinator review
required no production, verification, contract or scope correction. The accepted
commit includes the closed, synchronized bead and complete implementation.

`TransactionResult::provenance()` now owns every complete output-event and
episode-change cause and its full backward closure. It no longer adds unrelated
candidate-current facts. Empty cause-bearing streams have empty views. Existing
owned inspections supply current facts; the public accessor documents that
boundary. No public type/method, specification, contract, profile, version,
encoder, golden, runtime policy or host dependency changes are proposed.

## Concrete implementation

`publish_success` collects the full chronological output stream and all episode
changes through existing cause accessors. `Records::owned_closure` already
deduplicates identities and selects complete shared nodes and exact lookup
membership, so no additional root framework or redundant deduplication pass is
needed. Source deadline accumulation, result-root translation in migration,
machine publication/retention, canonical frozen content and genuine predecessors
are unchanged. Removed the now-unused private `StandardHistory::retained_causes`
helper whose only production caller was the excess result-root extension;
machine `StandardCauses` role selection is unchanged.

Current-fact assertions now use owned Toggle, output, latch, edge, PulseDelay or
module inspections. Private SOURCE/canonical controls use the complete machine
view for current machine facts, preserving their exact source-payload comparisons
and restored continuation checks. Genuine result-event ancestry controls remain
on results. In addition to the preparation's sites, the full gate identified a
pending PulseDelay initialization assertion and three edge-memory assertions;
these migrations preserve the original semantic questions. The Parity helper
continues holding the supplied result while inspecting current output support.

No Mossmoot files or Cargo pin were changed. Its audited event/inspection callers
need no source migration; dependency adoption remains separate coordinator work.
Standalone result audit encoding stays unsupported. A future cause-bearing
result field must deliberately extend this manifest.

## Regression-first evidence and actual work

Before correction, both permanent ownership regressions failed over genuine
hidden Periodic-to-Toggle histories at 1/10/1000 reactions. A single chronological
advance builds each history, avoiding repeated outer digest preparation while
still executing every real deadline and preserving cumulative stateful ancestry.
The initial separate-transaction fixture was stopped and replaced for finite
gate cost; it is not claimed as a pass. The completed failed-before witness is
`/private/tmp/ms-am9-before.log` (two expected assertion failures).

| Hidden history | Empty result before → after | Independent pulse result before → after | Hidden cause lookup before → after |
| ---: | ---: | ---: | --- |
| 1 | 15 → 0 | 16 → 3 | resolves → absent |
| 10 | 60 → 0 | 61 → 3 | resolves → absent |
| 1000 | 5010 → 0 | 5011 → 3 | resolves → absent |

Test-only accounting captures actual closure/index counter deltas around result
selection, before machine publication. The closure counter also counts duplicate
handle visits. An independent stack/set traversal starts from all public
event/change causes and compares the exact resulting membership with the result;
it does not call the selection/publication helper. It shares low-level record
predecessor laws and the ordinary cause accessor, not the selected-root algorithm.

| History | Empty selection: roots/handles/records/index visits/index entries | Pulse selection: roots/handles/records/index visits/index entries | Other closure visits: empty / pulse | Catalogue handles copied | Logical appends: empty / pulse |
| ---: | --- | --- | ---: | ---: | ---: |
| 1 | 0/0/0/0/0 | 1/3/3/6/6 | 15 / 16 | 10 | 6 / 7 |
| 10 | 0/0/0/0/0 | 1/3/3/6/6 | 60 / 61 | 55 | 6 / 7 |
| 1000 | 0/0/0/0/0 | 1/3/3/6/6 | 5010 / 5011 | 5005 | 6 / 7 |

These are selected-boundary measurements for this finite fixture. Index branching
and genuine event ancestry can vary elsewhere. Required machine/episode/inspection
selection, catalogue copying, admission, logical growth and canonical encoding
remain real work. There is no bounded-total-history, constant-total-work or
throughput claim. Measurements are in `/private/tmp/ms-am9-ownership.log`.

## Ownership, preservation and failure witnesses

Five new private tests cover the two scaling regressions, an empty split forecast,
complete deadline/patch streams and occurrence/episode ownership. Their independent
manifest check resolves every exposed cause and traverses full predecessors.
The patch case includes source-revision events before time 5 and episode changes,
then continues through time 101 before dropping the machine. Retained node and
pending inspections, cloned result views and an independent event explanation
remain valid. Actual weak payload handles and live node/index counts demonstrate
excluded data releasing while an empty result remains, and genuine event data
remaining until its final explanation owner drops.

The quiet diagnostic fixture preserves a real transient occurrence with an
unchanged active episode but no result roots. Beginning and resolved results
remain independently explainable after machine drop. Existing Changed/replay
and Terminated/reset fixtures now also explain complete episode causes after
all relevant machines and the earlier initial result are dropped.

A new binding test splits ordinary/projected products, compares cause/stamp/
revision tuples, continues and drops the bound machine, then drops the ordinary
result while a cloned result view still resolves the projected handles. Current
output inspection ownership remains separate. Existing live-binding producing
map and forecast/apply controls remain intact.

Warm extraction/provenance/digest/result/projection rejection controls still
compare complete machine observations, exact snapshots/digests/root references
and live node/index ownership. New reached-stage assertions require nonzero
result selection for digest/result/projection faults and zero result selection
for earlier faults. Held/dropped artifact comparisons, exact-limit growth,
edge C/M appends, real remapping, explicit qualified SOURCE content, freezing,
clone-preparation, snapshot/restore and replay controls are preserved. Canonical
and clone references share grammar/context or evaluation/publication and are
not treated as independent proofs of the ownership boundary.

## Validation and handoff

The corrected focused run passed 161 tests, with no warning or leaky status.
Five final affected/lifetime/fault checks also passed. Earlier focused failures
were current-fact consumers and were corrected in scope; an early failed run
also reported one existing combinational control as leaky, absent from the
corrected focused and complete final runs. No retry configuration or test/gate
policy was changed.

`make check-dev` passed after the pending-event consumer correction, then
`make check-final` passed. Its retained Nextest output confirms **649 tests run,
649 passed, zero skipped**, with no warning or leaky status. Comprehensive checks
include static guards, strict Clippy, contract/guardrail tooling, doctests,
documentation and dependency policy. Source/whitespace audits passed: accepted
HEAD is unchanged; the proposal is unstaged; specifications, contracts, goldens,
Cargo files, Makefile, deny policy and exported beads have no diff. Mossmoot
remains clean at its audited HEAD and dependency pin. The existing
advisory-cache relocation preserves parsed deny policy except its writable
database path and leaves normal fetch enabled. A temporary pass-through wrapper
retains mandatory Nextest output without changing arguments or exit status.

Validation logs: `/private/tmp/ms-am9-focused-final.log`,
`/private/tmp/ms-am9-final-corrections.log`, `/private/tmp/ms-am9-check-dev.log`,
`/private/tmp/ms-am9-check-final.log` and `/private/tmp/ms-am9-final-nextest.log`.

Nineteen reviewed contract records are reused unchanged at the accepted planning
evidence baseline; all selected rule sources remain unchanged. No reusable new
specification fact or material planning gap was discovered. Coordinator owns
corrective acceptance, bead closure/export, staging and the accepted commit.
No push or follow-on work is performed.

## Independent coordinator acceptance

The coordinator reviewed the complete production, test, measurement and caller
migration surface against the bounded bead and reused reviewed contract rules.
Selected membership, full event/change ancestry, independent artifact lifetimes,
machine-root controls and reached-stage rejection witnesses meet the accepted
design. No code or evidence correction was needed.

Independent checks passed 159 focused ownership/remap/freezing and affected
consumer tests, then `make check-dev` and `make check-final`. Retained final
Nextest output confirms 649 run, 649 passed, zero skipped and no warning or leaky
status. All six 1/10/1000 empty/pulse measurements reproduce the table's actual
selected-boundary work and separately disclosed necessary closure/copy/growth
costs. Specifications, contracts, goldens, Cargo files, Makefile, deny policy
and host dependency remain unchanged. Whitespace and final change-surface checks
passed. Advisory-cache relocation remains parsed-policy-equivalent except its
writable database path, with normal fetch enabled; the temporary capture wrapper
preserves Nextest arguments and exit status.

Independent logs: `/private/tmp/ms-am9-coordinator-{ownership,remap,freezing,
affected,check-dev,check-final,nextest}.log`. The coordinator closes and exports
`ms-am9` for the accepted product commit. No push, host adoption, throughput
measurement or broader digest redesign is included.
