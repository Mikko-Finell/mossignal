# Observable-digest cost assessment preparation

Planning only on clean accepted `main`
`bc9a88dcab6c0539a8cf54712f45dd290ca9dfd6`, after accepted edge observation,
ordinary remapping, incremental freezing and result ownership corrections.
Generated measurement bead: **ms-iri**, open/unassigned, dependent on closed
`ms-am9`, independently accepted for a separate implementation assignment. No measurement code,
optimization or digest projection change is implemented in this pass.

The task comes from Mossmoot `docs/scratch/refactoring_brief.md`, section 2,
“Keep the remaining observable-digest question explicit”. The brief conditionally
requests measurement after the four corrections, not a new projection. It is
task context, not Mossignal authority. Triage found 100 closed beads, no ready or
in-progress work. Related closed `ms-0ap.4`, `ms-fl1` and the four accepted
correction beads establish existing behavior; none is reopened. There is no
active roadmap claim, host adoption or push in this task.

## Recommendation and authority

Prepare one finite, unchanged-semantics measurement implementation. It should
attribute actual work and bytes at raw E4/O4 construction/hash boundaries and
warm successful publication, while keeping genuine ancestry and current facts
complete. The result is evidence for a later decision, without assuming a
bottleneck ranking or a digest redesign.

`machine-state-digests.yaml` / `observable-extension` and
`content-addressed-provenance` already require canonical content-addressed records
of current backward closure in O4. `execution-projection` excludes those graph
payloads from E4 while retaining required cause-content commitments. Persistence
sections “57. Execution-state digest”, “58. Observable-state digest” and the
current root-directed profile represent that distinction. Replacing O4 payloads
with only root hashes is a separate semantic projection decision, not an
authorized private removal. No O5, speculative root-only contract, history cap,
checkpoint substitution, normative amendment or version change is selected.

All thirteen records below are used unchanged, with **516 unchanged citations**
and no changed, missing, ambiguous or unfingerprinted sources. Selected rule
sources remain settled. No concrete uncovered task-required semantic facet or
contradiction warrants a new/changed contract. Current instrumentation choices
and finite test dimensions are implementation freedom.

| Record under `docs/specs/contracts/` | Contract ID | Unchanged citations | Governing role |
| --- | --- | ---: | --- |
| `machine-state-digests.yaml` | `mossignal.identity.machine_state_digests` | 57 | E/O scope, full canonical O closure, current P4/E4/O4, pure cached exposure and atomic identity |
| `optimized-path-verification.yaml` | `mossignal.verification.optimized_paths` | 9 | real-path evidence, complete rejection observation, independent reference boundaries and limitations |
| `provenance-retention.yaml` | `mossignal.provenance.retention` | 5 | independent artifacts, complete machine roots, unchanged logical growth/budgets and metadata lifetimes |
| `semantic-inspection.yaml` | `mossignal.inspection.semantic_projection` | 10 | complete owned observations and read purity |
| `causal-explanations.yaml` | `mossignal.provenance.causal_explanations` | 10 | current versus transition, complete explanation boundary and temporal support |
| `edge-detector-family.yaml` | `mossignal.node.edge_detector_family` | 44 | separate comparison C and successor observation M; genuine quiet-detector control |
| `temporal-event-execution.yaml` | `mossignal.runtime.temporal_event_execution` | 30 | real chronological reactions, pending scheduling and explicit budgets |
| `persistent-diagnostic-episodes.yaml` | `mossignal.diagnostics.persistent_episodes` | 17 | separate active evidence ownership, material changes and atomic state |
| `machine-snapshot-artifact.yaml` | `mossignal.persistence.machine_snapshot_artifact` | 65 | separate complete snapshot encoding and unchanged artifact identity |
| `replay-artifacts.yaml` | `mossignal.persistence.replay_artifacts` | 82 | ordinary replay and exact current canonical artifacts |
| `atomic-topology-replacement.yaml` | `mossignal.reconfiguration.atomic_replacement` | 81 | preserved source/target cause semantics and warm atomic controls |
| `transaction-forecast.yaml` | `mossignal.runtime.transaction_forecast` | 26 | private candidate/source invariance and forecast/apply controls |
| `machine-snapshot-restoration.yaml` | `mossignal.persistence.machine_snapshot_restoration` | 80 | current root validation and unchanged restore/continuation sufficiency |

Status output: `/private/tmp/ms-digest-assessment-contract-status.txt`.
Catalog: `/private/tmp/ms-digest-assessment-catalog.txt`. Task-scoped discovery
maps scope/ownership/persistence/failure facets to these represented rules;
performance targets, future projections and new artifact encoders are excluded.

## Existing observations and their limits

The coordinator supplied one passing existing test with four measured cases,
`state_digest::tests::raw_execution_does_not_index_growing_ancestry`, at the
accepted baseline. This preparation reads its log and source; it does not rerun
Rust tests. The log is evidence, not specification authority:
`/private/tmp/mossignal-digest-coordinator-baseline.log`.

| Toggle stimulus | History | E direct resolutions | O indexed / closure visits / emitted records | O provenance payload bytes | Snapshot artifact bytes |
| --- | ---: | ---: | --- | ---: | ---: |
| External pulse | 4 | 4 | 13 / 13 / 13 | 2521 | 4343 |
| External pulse | 40 | 4 | 121 / 121 / 121 | 23649 | 25473 |
| Periodic pulse | 4 | 7 | 29 / 29 / 25 | 6320 | 8938 |
| Periodic pulse | 40 | 7 | 245 / 245 / 205 | 53890 | 56517 |

Raw E performs one real build/hash and zero context/index/closure/emission work
in all four cases. The log's “raw O” calls `observable_digest_input`: one build,
**zero observable hash calls**. Its `bytes_emitted` is the sum of canonical
provenance record payload lengths, not the complete O input or bytes passed to
BLAKE3. Snapshot bytes are a separate complete artifact size. The observations
establish genuine O construction/emission growth at 4/40, not its hashing cost,
warm-transaction attribution, 1000-scale behavior or elapsed-time importance.

Periodic indexing/emission differences do not prove disposable ancestry.
Physical record identities are visited per view; byte-identical canonical
records coalesce into one emitted digest. Additional counters must distinguish
that from indexed-but-unemitted content and repeated episode-view indexing.
Fixed direct-resolution counts also do not measure BTree lookup comparisons,
allocation or total E execution time.

## Source proof and accounting gaps

| Actual boundary | Behavior and interpretation |
| --- | --- |
| `machine.rs::Machine::prepare_digests` | Builds/hashes both raw values on the private coherent successor, then installs one completed pair. Successful transactions still pay this cost; cached getters copy completed values. |
| `state_digest.rs::{execution,observable}_state_digest` | Hash the actual fully domain-separated input Vec. Existing counters count calls, not input bytes. |
| `projection_payload` / `CauseProjection` | E uses direct frozen cause-content lookup. O builds an indexed context and adds current observation and required provenance. |
| `ProjectionContext::build` / `index_view` | Index the machine view and each active-episode view into one content table, reading cached creating-topology content. `records_indexed` counts physical entries across those views. |
| `ContentTable::insert` | Same-digest unequal bytes violate an internal invariant and panic; equal canonical payloads coalesce. Existing counters do not separate attempts, equal-content hits and new content. |
| `reachable_digests` | Allocates per-view membership scratch and follows predecessor lists. Existing `closure_visits` counts first visits per ordinal, not root entries, duplicate pops or predecessor entries. |
| `write_provenance` | Unions machine/episode reachable content digests and emits each canonical payload once in digest order. Existing emitted-record and payload-byte counters are useful but exclude other O fields and domain framing. |
| `identity.rs::domain_separated` | Canonically frames domain, payload and version in CBOR. Actual hash bytes are the final Vec length, not a manually guessed prefix or provenance sum. |
| `persistence.rs` / `cause_digest_index` | Snapshot indexing/complete artifact construction remains separate. Cached digest reads inside snapshot assembly are not extra raw digest hashes. |
| `inspection.rs::causal` | Produces complete owned explanation roots/edges and selected provenance. There is no native standalone explanation binary encoder to time or assign serialization bytes to. Measure actual explanation traversal/selection and graph sizes; complete snapshot serialization supplies the existing serialization control. |

Reuse `projection_work`, `causal_work` (including result-selection/owned-closure,
freeze, growth and catalogue counters), and `execution_work`. Minimal missing
test-only accounting is:

- E/O **actual hash input bytes** at the existing BLAKE3 call sites; full canonical
  input lengths remain separately inspectable through existing input builders.
- Actual closure root entries, popped handles and predecessor entries, alongside
  existing first-ordinal visits. Do not infer traversed edges from indexed rows.
- Content-table insertion attempts, new payloads and equal-content coalescing
  hits, including machine/episode overlap. Do not claim actual byte-comparison
  work from payload length or prescribe BTree path choreography.

Source-view sizes/root counts can be reported from existing inventories; add a
small local breakdown only if needed to attribute machine versus episode calls.
Report processed reactions and existing operation-budget charges separately;
for these unpatched fixed-topology successful cells, charge is operation count
times real reaction count. If an actual evaluator-call count is needed, use
minimal test-only accounting at that boundary, not a guessed digest-derived
count or a new public report. Patch rollback remains an existing separate control.

## Small finite experiment

Use fixed keys/domain/policy and ordinary valid machine histories. One private
measurement suite and a deterministic evidence table are enough; no benchmark
crate, new dependency or external harness is needed. Setup and reference work
are outside every captured interval. Reuse accepted fixtures/helpers where useful.

| Dimension | Bounded choice and purpose |
| --- | --- |
| Primary history | 1/10/1000 real reactions for quiet edge detectors and genuine Periodic-to-Toggle inversion ancestry. Keep prior external-pulse 4/40 tests as existing controls; no slow repeated-outer 1000-pulse duplicate is required. |
| Current width | Widths 1 and 4 at each primary history. Use distinct keyed hidden detector/Toggle owners with shared stimulus and controlled visible endpoints; report topology operations, current root/view sizes, distinct content and fan-in. This separates history from current root/topology width. |
| Observable result | From each warm base, same next target and underlying hidden-state reaction, with either no visible event or one independent passthrough pulse. Keep hidden owners off external outputs so the result stays empty or has fixed small genuine ancestry; preserve all real deadline/target occurrences. |
| Old artifact ownership | Paired held/dropped older result plus targeted old inspection at width 1, histories 10/1000, for both families. Acquire at a real prefix; finish identical semantic histories. Compare exact inputs/digests/current manifests and observations while physical ownership differs. Do not retain an incidental whole machine view. |
| Distinct closure sources | The primary Periodic fixture already has a real future pending root. Add one paired active/inactive hidden LevelSetResetLatch episode at width 1/history 10 to expose separate episode-view indexing and coalescing. Do not expect equal digests across different episode states; verify each against reference. No additional temporal-family matrix is needed. |

For every primary cell measure raw E4 and raw O4 **including their real hash
calls**, then warm successful ordinary transactions from equivalent bases for
the two result modes. Capture complete per-phase work, output/episode sizes,
raw E/O full input lengths and hash bytes, required canonical record payload
sum, current view/root widths and logical reaction/operation charges. Record
held/dropped contrasts and the episode probe separately rather than multiplying
every factor into an unnecessary exhaustive benchmark matrix.

Full snapshot artifact encoding and an owned full explanation are separate
intervals at representative width-1 histories 10/1000, with actual sizes/work and
unchanged lifetime checks. Cached queries are a zero-work control already covered
by `completed_digest_queries_do_no_projection_work`; reuse it and ensure any
new counters participate in its default-zero check. Do not attribute setup,
reference encodes, snapshots/explanations or catalogue/admission/result work to
raw digest work. Save observations after counters are captured, or disable
accounting around oracle reads, so validation cannot contaminate measurements.

## Verification and decision packet

Require nonzero actual raw E/O build/hash/byte counts and equality with complete
uncached reference inputs/digests, using the existing version-4 reference and
exact vectors. Independently check byte lengths, exposed result cause closure,
canonical unique-record/payload totals and raw membership/lifetimes. The reference
recomputes canonical content without node caches but shares CBOR/domain grammar,
semantic laws and projection definitions. Clone-preparation shares evaluation/
publication. Neither alone proves cost counters or ownership; real-boundary
accounting and independent graph/length/lifetime witnesses supplement them.

Instrumentation regressions should prove newly added counters detect positive
real work, including warm publication; do not accept all-zero stubs. A measurement
task is not a semantic bug fix: no artificial failing product behavior is needed.
Quiet detector ancestry/emission remains bounded for fixed width, allowing clock
integer-width changes; genuine Toggle closure/payload growth remains visible.
Use justified bounds/scaling witnesses, not golden incidental traversal counts.
Measure and report full byte values; do not change existing input goldens.

Reuse warm complete-predecessor fault oracles. State-extraction/provenance faults
occur before final digest work. `DigestPreparation` is injected before the build,
so it must not falsely claim completed hashes. Result/projection faults happen
after preparation and need positive E/O build/hash/byte witnesses, yet preserve
published cached identities, snapshots, roots, state/pending/episodes and retained
explanations/lookup ownership. Preserve exact budget, qualified SOURCE/restore,
remap/freezing/result-ownership and clone controls without duplicating their suites.

Eventual implementation checks: nonzero focused new measurement/instrumentation,
raw/cached digest, edge C/M, result ownership, freezing, remap/clone and warm
failure tests, plus applicable snapshot/restore/replay/episode controls; then
`make check-dev`, `make check-final`, exact baseline vector/spec/policy diff and
source/whitespace audits. Optional timing is descriptive and never a gate.

Deliver reproducible commands, finite case metadata and actual per-phase tables
with assumptions, exclusions and reference limits. Explain which costs are
required O4 payload/hash work, which are current indexing/coalescing/membership
mechanics, and which are other transaction/artifact work. Do not sum unlike work
units into a “percent bottleneck” or infer frame-time impact from byte counts.
There is no product latency budget or predetermined go/no-go threshold here.

The packet can support retaining O4, separately preparing an O4-preserving private
optimization, or separately reviewing a different semantic projection if its
required byte burden warrants that product tradeoff. This bead makes none of
those later changes. Measurements alone do not approve a specification/profile.

## Readiness and preparation checks

Expected implementation surface: small `cfg(test)` additions in
`crates/mossignal/src/state_digest.rs` and `crates/mossignal/src/projection_work.rs`,
one private measurement test module, minimal existing
fixture/helper or transaction accounting plumbing only if necessary, and a compact
evidence report. Canonical grammar, root/record meaning, domain/profile/schema,
public APIs, policy, snapshots/explanations/replay and host work are controls.

No conflicting requirement, missing fundamental semantics or frozen promise
blocks this measurement. Concrete counter naming and deterministic helper layout
remain implementation freedom. The coordinator independently accepts this bead
before a separate implementation assignment. No drafts or normative amendments,
no Rust gates/measurements or generated-artifact edits in this preparation, and
no export/closure/stage/commit/push.

Planning validation: all eleven contract-tool tests pass in the locked `uv`
environment. Text/reference checks resolve all thirteen contract IDs/paths and
all 65 selected rule IDs with unchanged supporting sources; all 516 citations
remain unchanged. Proposal/bead whitespace and round-trip/readiness/dependency
checks pass. The bead is open/unassigned with only the closed `ms-am9` dependency;
its description and acceptance criteria preserve the full bounded scope.
Accepted HEAD remains `bc9a88dcab6c0539a8cf54712f45dd290ca9dfd6`.
Tracked implementation/spec/contract/golden/policy/host and `.beads` export files
are unchanged; the preparation document and unflushed local bead are this pass.
Coordinator owns planning export and commit. No Rust gate or new measurement
was run. Contract-tool log:
`/private/tmp/ms-digest-assessment-contract-tool-tests.log`.

## Independent planning acceptance

The coordinator accepted this bounded measurement plan without design, contract,
verification or scope correction. Independent checks reproduce all thirteen
reviewed records, 65 selected rules and 516 unchanged source citations, with no
adverse source status. All eleven locked-uv contract-tool tests, bead status/
dependency/readiness, zero active cycles, references/text and unchanged product
baseline/change-surface checks pass. No draft contract, normative amendment, new
measurement, Rust gate, production optimization, profile/version or host change
is claimed. The coordinator exports and commits this planning pass before a
separate implementation assignment; corrective product acceptance remains next.
