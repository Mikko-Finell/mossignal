# Edge-detector observation provenance preparation

Prepared 2026-10-10 against clean `main` at
`69e26269c1279d0895a827cb167abffb103a49c9`. This is a task-scoped,
nonnormative design accepted by independent coordinator review on 2026-10-10.
No Rust, test, golden, or authoritative specification enactment is part of this
planning acceptance.
Native delivery: `ms-49a`. Independent design review: `ms-49a.1`.
Implementation brief: `ms-49a.2`; design review `ms-49a.1` is accepted. The
implementation remains a separate worker assignment and independent acceptance.
The return chat is `01a125c5-2f01-7691-999a-f12b680f2768`.

The task originates in Mossmoot's local `docs/scratch/refactoring_brief.md`,
section 1, and Mosstools' `docs/proposals/mossignal-refinement-continuation.md`.
Those documents select scope; native specifications govern correctness. This
does not restart the retired roadmap. The coordinator retains design approval,
implementation acceptance and downstream dependency adoption.

## Accepted correction

Keep the detector's current comparison and successor remembered observation
as two facts. Use the existing node-owned `Derived` representation for both;
introduce no public variant, record tag, history control or checkpoint facility.
For a detector evaluated at reaction stamp `r`, let:

- `T_r` be the existing reaction establishment cause used by evaluation;
- `I_r` be the existing cause of the fully settled Level input operation;
- `M_prev` be the retained previous observation cause, if established and not
  supplied by declared initialization/reset;
- `C_r = Derived(detector, set{T_r, I_r, M_prev when present})` be the comparison;
- `M_r = Derived(detector, set{T_r, I_r})` be successor observation establishment.

Retain `C_r` in operation causes and the existing output/result derivation path.
Retain `M_r` in `edge_observation_causes`. Append two logical records on every
detector evaluation, even when their canonical content is identical. Canonical
coalescing remains valid and does not erase the distinct logical append charge.
Read `M_prev` before replacing its map entry. The exact append order is private
freedom; both records follow `I_r` and `T_r`, and neither consumes the other.

This is the accepted choice consistent with the existing restoration
owner model: reusing `I_r` directly would make a state root belong to an upstream
node or external input, while current state assignment determines the owner
from the record subject. A new record variant would require additional public
inspection/encoding/parsing machinery without a needed new value schema.
Separate ordinary `Derived` records preserve node attribution and expose their
different support without that expansion.

`T_r` is not always an ordinary ReadyTransaction. Existing initialization with
a patch uses a declared-state checkpoint supported by initialization and patch
causes. Ready patch settlement uses TopologyChange. Preserve those boundaries.
They may themselves contain genuine historical SOURCE support. Do not replace
them with a fresh ungrounded transaction to force a retention bound.

## Source trace and ownership consequences

| Boundary | Existing flow | Proposed obligation |
| --- | --- | --- |
| Evaluation | `compile.rs::evaluate_reaction_with_due` compares previous EdgeObservation with settled input, emits zero/one, and proposes current input | Leave all laws and previous-state isolation unchanged for Rising/Falling/Any |
| Provenance | `transaction.rs::append_evaluation_provenance` builds one Derived from reaction/input/previous cause and installs it in both operation and memory maps | Preserve comparison supporters; append and install the independent observation establishment |
| Previous memory | `retained_edge_cause` requires a cause for established ready memory, except declared/reset premises | Continue requiring genuine established observation support; do not silently fall back to a transaction |
| Publication | `publish_success`, `publish_candidate` and `collect_machine_provenance` retain result roots separately and collect the machine's current closure | Existing operation and edge-memory roots select the two facts; leave result ownership breadth unchanged |
| Current roots | `causal_roots::machine` includes operation causes and edge-observation causes; `bindings` includes current and edge_observation roles | Keep both inventories and role association; obsolete comparisons disappear only when no genuine current/artifact ancestor requires them |
| Inspection | `inspection.rs::inspect_node` gets last_reaction_cause from the node operation; Edge.establishment/latest_transition from edge memory | Last reaction explains the comparison; memory establishment explains current observation; owned node inspection retains both exposed closures |
| Result | Emitted pulse output derives from the comparison and is exposed through result-owned provenance/explanation | Retained earlier pulses continue resolving both real observation premises after later execution and machine drop |
| Snapshot | `persistence.rs::state_roots` includes edge memory; `current_roots` separately captures current operation support | Persist M_r under state and C_r under current, preserving genuine SOURCE information and allowing content-equal first-reaction causes |
| Restore | `assign_state` infers state owner from record subject; `require_edge_observation_causes` requires every ready detector; current roles validate subject/kind/stamp | Restore M_r and C_r independently, add task-focused memory-role checks described below, and retain all existing strict checks |
| Migration | `migration.rs::finalize` remaps preserved memory to target owner; removes declared/reset edge causes; `checkpoint_migration` translates SOURCE facts and wraps preserved memory in Migration | Target comparison consumes migrated M_prev; target memory establishes current input. SOURCE manifests include actual current C and M, even for removed/reset owners |
| Canonical state | Execution binds current and edge_observation role identities; observable/snapshot emit required closure; completed digest pair is prepared before publication | Keep projections' responsibilities, cache preparation and reference checks; enact the explicit proposed namespace profile below |

Preserving migration wraps the old observation with the existing rule/patch
support. Reset uses the target Baseline/Assume policy and does not reuse the old
memory premise. Same-kind, cross-kind and rewired target evaluations apply their
ordinary laws to the actual migrated previous value. Migration can retain earlier
comparison content legitimately through the TopologyChange SOURCE cause. A
quiet post-patch reaction must not continue manufacturing a comparison chain
through the memory role alone.

Baseline has no established prior premise on the first reaction. Assume(Low)
and Assume(High) use declared initialization as the prior premise, under the
existing comparison construction and network definition. That first comparison
can emit; successor memory always establishes the actual settled input. For
these first reactions C and M can have byte-identical content. Private identity
equality/inequality is not the semantic oracle; canonical support and role facts
are. No additional declared-state record is required by this correction.

## Strict restoration seam

The current state-root assignment validates resolvable node ownership and family
coverage but does not distinguish a node's comparison Derived from its memory
Derived. `current_record_matches` already checks current operation owner, allowed
record kind, and a direct current reaction supporter (or declared-state checkpoint).
A direct upstream cause is therefore not an appropriate memory substitution.

For the proposed profile, validate the committed ready edge memory after current
operation causes have been recovered:

1. Exactly one state root per ready edge owner resolves to an ordinary Derived
   owned by that stable direct/qualified edge node. An upstream node/input,
   comparison with extra old-memory supporters, wrong kind, missing root or
   duplicate/conflicting owner is rejected through existing provenance failures.
2. Its canonical supporter set is exactly `{T_r, I_r}`. Recover `I_r` through
   compiled input-operation correspondence and restored operation aliases;
   compare canonical content, not transient allocation positions. Recover T
   from the admitted current reaction establishment cause with current physical
   time, same-time order and revision. Accept the existing ordinary
   initialization/ready, ready TopologyChange, and initialization-patch declared
   checkpoint shapes; validate the latter's underlying initialization/patch
   stamps and revision rather than accepting any unlabelled checkpoint.
3. The comparison continues satisfying existing strict current-role validation
   and includes the same current reaction/input premises. It can additionally
   include a genuine previous observation (or migration fact). C and M may
   coincide in canonical content where no additional prior cause exists.
4. Retain settled-value consistency, exact version/component checks, canonical
   digest recomputation, complete DAG closure, SOURCE checkpoint correspondence,
   complete role coverage and both independently recomputed machine digests.

No new persisted role or field is needed: state already names M's node owner,
current_roots names C's node owner, topology reconstructs input aliases, and
existing transaction/patch records establish stamp and revision. Ordinary
snapshots settle their ready target before capture; transient pre-settlement
migrated memory is not a valid committed snapshot. Reject a forged state root
relabelled to the comparison even after resigning the envelope and graph hashes;
also test valid same-content first-reaction roots. Do not require a replay of
all historical computations to accept an ordinary snapshot.

## Accepted target canonical profile

The current governing profile is Persistence section 16.10 and the unchanged
reviewed version rules. This design accepts an explicit revision of that
profile during implementation, not an exception or implicit freeze.
The causal interpretation and state-role projection change, so use new provenance
and E/O namespaces together. Keep unchanged components at their current values.

| Component | Accepted baseline | Accepted target |
| --- | --- | --- |
| ProvenanceSemanticsVersion, including nested SOURCE fact | 3 | 4 |
| Provenance record label / domain version | mossignal/provenance_record/v3 / 3 | mossignal/provenance_record/v4 / 4 |
| Execution/observable projection_version | 3 | 4 |
| Execution label / domain version | mossignal/execution_state_digest/v3 / 3 | mossignal/execution_state_digest/v4 / 4 |
| Observable label / domain version | mossignal/observable_state_digest/v3 / 3 | mossignal/observable_state_digest/v4 / 4 |
| Machine snapshot ArtifactSchemaVersion | 3 | 3 |
| Replay/input/transaction ArtifactSchemaVersion | 2 | 2 |
| Envelope, canonical encoding, digest suite | 2, 2, 2 | 2, 2, 2 |
| Core, built-in node, physical patch, diagnostic semantics | 2, 2, 2, 2 | 2, 2, 2, 2 |
| Snapshot, artifact-integrity, replay-log-content domains | respective /v2, version 2 | unchanged |
| Network/input-schema/module/catalogue/runtime-policy identity projections | current projections | unchanged |

Unkeyed BLAKE3-256 and canonical CBOR grammar remain unchanged. Record fields,
Derived kind, supporter role, state-value schema and snapshot current_roots shape
remain unchanged. The namespace/profile revision acknowledges changed causal
meaning; it is not a broader observable-digest redesign. Artifact bytes and
identities change through provenance/version payloads even for non-edge graphs.
Declared-state E/O changes through the new projection/domain. Node numerical
transition and migration laws, authored fingerprints and policy-ID calculation
do not change. Existing policy limits can admit fewer reactions because the
correction appends an additional required logical fact per detector evaluation.

Current readers accept only the new mixed vector after implementation. Check all
envelope/payload/nested replay components and nested SOURCE facts independently;
changing claims alone never validates old canonical records. No historical
reader, shim, upgrader or fallback is proposed. Mossmoot retains its accepted pin
until native acceptance; the coordinator records the coordinated profile cutover
in Mosstools before adoption. There is no downstream edit in this bead.

Golden impact is explicit: the E/O input and digest fixtures for declared/ready
state, three migrated machine vectors, uninitialized/families snapshot bytes and
their digest pairs, and replay_not_gate_frame/log bytes all change under P4/E4/O4.
Rename the three *_migrated_v3.hex files to *_migrated_v4.hex and their references;
keep the unversioned fixture names. Add a small exact edge fixture covering
distinct C/M roles. The seven authored fingerprint projection fixtures and
diagnostic catalogue/failure inventory remain unchanged. Regenerate affected
vectors only after independent canonical input comparisons pass. Record reasons
and parsed component/domain checks; blind golden replacement is not acceptance.

The authoritative profile amendment belongs in the accepted implementation
proposal after coordinator design approval. It will require independent review
of the then-affected current-profile contract rules in machine-state-digests,
machine-snapshot-artifact, machine-snapshot-restoration, replay-artifacts and
atomic-topology-replacement, without re-auditing their other unchanged rules.
Preparation does not insert speculative P4 truth into those contracts.

## Logical budgets and rollback

Always appending M in addition to C introduces exactly one extra logical record
per evaluated detector per reaction. For an ordinary outer transaction with E
detectors and R settled internal/target reactions, the delta relative to the
old construction is `E * R`. For a patch, sum source and target detector counts
over their actual reactions instead; other migration appends stay governed by
existing accounting. Identical canonical records, unowned/reclaimed records,
held artifacts and cached bytes do not change the charge. Source checkpoint
replacement wrappers retain their existing accounting rule.

Test below/exact/above required-provenance budgets using an explicitly enumerated
append ledger for initialization, ordinary advancement, same-time settlement and
an outer transaction with earlier deadlines plus target patch. Include the
content-equal Baseline/Assume first reaction: two logical edge appends still
charge even if artifact encoding coalesces them. Existing events/reactions/
operations/pending limits and structured consumed/limit evidence remain intact.

Use the ordinary isolated successor and current fault seams. Compare complete
predecessor fields, C/M roots, cached digest pair, snapshot, pending serials,
reaction stamp, episode state and retained artifact contents after budget,
overflow, state-extraction/provenance/digest/result and late bound-projection
failures. Include warm ready state, initialization, earlier internal deadlines,
and a patch with target bindings. Failed candidates release new observation
ownership; they must neither alter predecessor cause maps nor leave lookup
metadata or namespace ownership retained. Apply/forecast/bound apply share the
same construction. No new production fault or callback surface is needed.

## Executable verification plan

Implementation writes the bounded-ancestry regression first and observes its
failure on the accepted baseline before correction. This preparation ran no Rust
regression and makes no measured retention or throughput claim.

| Case family | Required witness |
| --- | --- |
| Constant input scaling | Fixed constant node and separately omitted external Level, each Low/High, all three kinds, Baseline/Assume Low/High; unrelated positive Pulse branch; sample after 1, 10, 1000 successful reactions |
| Actual transitions | Finite previous/current pairs and transitions both directions; emitted pulse supports previous/current genuine observations; non-emission follows each kind; no intermediate-glitch pulse |
| Same physical time | Low/High/Low through successive orders plus empty and unchanged reassertions; memory records actual observation order; comparisons use the immediately previous committed memory |
| Preserved topology | Same-kind and all cross-kind detector changes, changed initialization without reset, direct and module-qualified owners, preserved keys and explicit reassociation where supported |
| Reset/rewire | Reset to each target policy, rewire with equal and unequal levels, remove owner; genuine target-law pulses; SOURCE roots retained at effective time, result-only earlier events excluded from patch supporters |
| Continuation | Direct vs encoded/restored vs encoded/replayed patch-free continuation; snapshot before patch followed by two patches with an intervening restore; equal output histories with different memory origins stay causally distinct |
| Owned artifacts | Hold one early emitted pulse result, node inspection and forecast while running the long quiet continuation; compare complete content before/after later execution and machine drop; then release owners and confirm node release |
| Strict artifacts | Missing/wrong owner/wrong kind/old stamp/order/extra-comparison memory root, swapped C/M roles, altered component and nested SOURCE versions; resign hashes to reach semantic validation; valid coalesced first-reaction roots accepted |
| Failure | Complete rollback and exact logical append boundary cases from the prior section |
| Genuine ancestry control | Detector observes a Toggle or temporal state whose real history grows; retain that history completely instead of asserting a total-memory cap |

For the constant-input fixture, keep nonowning CauseRef labels for each known C
and M; a label list must not itself retain provenance. Walk sufficient closures
through public CauseInspection for semantic assertions and through a small
independent test-only graph traversal for counts. In each settled ordinary quiet
state, detector memory has no obsolete comparison solely as its predecessor;
current C can own previous M, and at most the current comparison is in the
detector-local ancestry beyond genuine input ancestry. Content-equal initial C/M
records must be classified by their observation role, not mistaken for a stale
comparison merely because their digest coincides. No prior comparison can
be reached through M unless the genuine I or existing patch-establishment cause
requires it. Assert this support relation directly, not just a smaller count.

Measure counts on fresh cases after dropping the just-returned result and all
temporary snapshots/inspections. Add a second run holding the fixed earlier
artifacts. The machine catalogue size, content projection and next reaction's
fork/closure work must be the same in held/dropped runs; physical live-node count
may include the independently held closures and must return to the appropriate
owner-relative value after drop. Use fixture-specific exact sets or constant
upper bounds derived from the construction, not arbitrary timing thresholds.

Production counters are taken at actual construction/traversal/encoding
boundaries: nodes created/released, logical_growth, live nodes, catalogue handles
and membership entries copied, closure records visited/emitted and bytes emitted,
canonical encodes/hashes. Add only needed test-only C/M append classification or
owned-closure traversal counters; `Records::owned_closure` is not currently
covered by the projection-emission counters. One quiet step's work must be
bounded independently of elapsed quiet reactions. Counting the whole history
run gives expected linear total work; do not mistake that for growing per-step
cost. Preserve zero old-record rewrites/hashes, one staged store, no duplicate
working collection clone and zero repeated-query projection work from the prior
campaign. Do not broaden this task into removing all remapping or freeze scans.

Record production counters before test-only reference execution; bracket or
separately reset accounting for reference traversals, fixture construction,
postmeasurement reads and destructive release. Keep physical ownership tracker
measurements meaningful while excluding reference-only owners. The uncached
Vec/map reference must continue checking exact P4 record payloads/hashes, E4/O4
bytes, independent current-role/root inventories and SOURCE wrapper selection.

Existing clone-preparation and replay/forecast paths share production comparison
construction, so they cannot independently detect this wrong dependency. The
canonical reference also reads production records: it detects encoding/root
mistakes, not invented premises. Supplement them with a small detached semantic
edge model that builds C/M support sets directly from supplied observations and
declared policy, without invoking append_evaluation_provenance, production root
collection or node caches. Assert expected supporters, well-founded order,
genuine root reachability, finite laws and observation-role associations. State
these independence limits explicitly in the implementation evidence report.

Suggested focused commands after tests exist:

```text
cargo test -p mossignal --test edge_detectors
cargo test -p mossignal --lib edge_observation
cargo test -p mossignal --test ordered_reactions
cargo test -p mossignal --test current_root_reclamation
cargo test -p mossignal --test atomic_topology_replacement
cargo test -p mossignal --test replay_artifacts
cargo test -p mossignal --test transaction_forecast
make check-dev
make check-final
```

Filters are selected around final test names; do not count a zero-test run as
verification. The implementer records the failed baseline regression, successful
finite matrix, exact counter ledger, reference limits and remaining costs in one
focused evidence report before independent acceptance.

## Scope of the retention claim

The correction removes a manufactured link from successor memory to every past
comparison. It does not promise bounded ancestry for every constant-valued input:
an upstream Toggle/Periodic/temporal obligation, a deliberately retained artifact,
a historical output baseline or repeated topology-change chain can still have
required history even when its current numerical value is unchanged. Never
reclassify those real premises as dead to meet a cost assertion. Observable and
snapshot encoding still traverse and emit required closure. Ordinary collection
and namespace copying/freezing remain existing costs; this task changes the
size of the genuinely selected closure in the demonstrated quiet edge fixture.

## Contract basis and task-required discoveries

All selected reviewed records were checked at the exact baseline. There are
459 cited source entries: 455 unchanged and four changed, all four in
edge-detector-family. Only affected rules were rechecked. Its changed sources
are Glitch-free semantics (settled-only scope), Causal order (ReactionStamp),
restricted network fingerprint (expanded exact provisional catalogue), and
module fingerprint (current exact record). Their existing task-relevant rules
remain supported. Old reviewed hashes are preserved for independent review;
none was refreshed by preparation.

| Contract ID | Path under docs/specs/contracts | Task facet / preparation status |
| --- | --- | --- |
| mossignal.node.edge_detector_family | edge-detector-family.yaml | Three laws, initialization, comparison/successor support, family migration; draft amendment |
| mossignal.provenance.retention | provenance-retention.yaml | Current/artifact ownership and exact logical append charge; reviewed unchanged |
| mossignal.provenance.causal_explanations | causal-explanations.yaml | Stateful distinction, real authoritative ancestry, unordered complete support; reviewed unchanged |
| mossignal.inspection.semantic_projection | semantic-inspection.yaml | Separate state/current/history and independently owned exposed causes; reviewed unchanged |
| mossignal.identity.machine_state_digests | machine-state-digests.yaml | Current roles, canonical DAG, version rule, exact bytes and completed identities; reviewed unchanged |
| mossignal.persistence.machine_snapshot_artifact | machine-snapshot-artifact.yaml | Self-contained state/current roots, canonical profile, pure capture; reviewed unchanged |
| mossignal.persistence.machine_snapshot_restoration | machine-snapshot-restoration.yaml | Strict roles, versions, state/closure/digests and future sufficiency; reviewed unchanged |
| mossignal.runtime.ordered_reactions | ordered-reactions.yaml | Previous committed occurrence, same-time order, rollback and continuation; reviewed unchanged |
| mossignal.reconfiguration.atomic_replacement | atomic-topology-replacement.yaml | Effective-time SOURCE roots, preserved/reset ancestry, atomic publication; reviewed unchanged |
| mossignal.verification.optimized_paths | optimized-path-verification.yaml | Independent reference limits, costs and complete failure observation; reviewed unchanged |
| mossignal.persistence.replay_artifacts | replay-artifacts.yaml | Ordinary fold, exact mixed nested profile and continuation; reviewed unchanged |

Task-required uncovered facts were researched in Built-in Node Semantics
section 18 and Reconfiguration section 44. They are represented by the two
new derived/direct edge-family rules, without prescribing record shapes as
permanent product truth. Snapshot/restore and SOURCE obligations map to existing
unchanged records above. Public checkpoint/history configuration, broad why-not
rendering, other primitives, alternate scheduling and later campaign removals
are adjacent coverage, not blockers. The proposed version-number selection and
exact Derived flow remain task decisions for coordinator acceptance.

## Expected implementation surface and handoff

Expected production edits: transaction.rs for the two fact flow, and
snapshot_restore.rs for task-focused memory validation; identity.rs,
state_digest.rs, persistence.rs and replay.rs for the coordinated P4/E4/O4
profile. compile.rs may expose a private input-correspondence helper if existing
input_port_operations is insufficient. Preserve all evaluator laws and schemas.
Inspection, migration and causal_roots should need no representation change;
touch only a concrete integration need discovered by the assigned implementation.

Test/evidence edits: edge_detectors integration cases, transaction unit tests or
a small test-only edge provenance fixture, causal_work accounting seams as
needed, state_digest_reference and strict snapshot tests, ordered/patch/
replay/forecast continuation tests, affected golden vectors and a focused
verification report. The exact authoritative profile edit and corresponding
changed contract rules require independent review after the concrete result.

No production implementation, acceptance commit, push, ordinary remapping
cleanup, freeze-new-nodes optimization, narrower result ownership, new node,
public API/history feature, checkpoint feature, observable-digest redesign,
Mossmoot edit or dependency adoption is authorized by this preparation.

There is no demonstrated conflicting requirement, missing fundamental semantic
rule or frozen compatibility promise. The design is semantically implementable.
Its implementation bead is gated by the separately requested independent design
acceptance, not by a fabricated specification blocker. The coordinator accepts
the two-appends accounting, exact P4/E4/O4 profile and derived contract boundary.
Implementation requires the separate bounded worker assignment; no follow-on
slice is authorized by this document.

## Preparation validation before independent review

All Python commands used the locked uv environment with
`UV_CACHE_DIR=/private/tmp/mossignal-planning-uv-cache`; the default cache path
was unavailable inside the sandbox. No dependency or gate was changed.

- Contract catalogue discovery and selected status inspection completed.
- Structural/source validation of the eleven selected records passed: unique
  rule IDs, valid source references, and no missing or ambiguous source paths.
  Final statuses are 455 unchanged, four changed existing edge citations and
  two new unfingerprinted edge facets, as expected for this draft.
- `uv run --locked python -m unittest scripts/test_contracts.py` passed 11 tests.
- Coverage diagnostic reported zero unresolved citations and one pre-existing
  unused alias (`provenance-retention: storage-freedom`), confirmed unchanged at
  HEAD. Its evidence-footprint warning is not a semantic-coverage pass claim.
- `git diff --check` passed; native issue descriptions/dependencies were read
  back with `br --json`.

Files changed are this proposal and the edge-family draft. Three generated native
beads remain uncommitted in the local database: delivery ms-49a, design review
ms-49a.1, and implementation brief ms-49a.2. Every mutation used --no-auto-flush;
the author leaves export/commit to independent planning review. No bead was
claimed, no contract promoted, and no Rust/test/golden/specification enactment or
Rust gate was performed. The review must decide or correct the exact fact flow,
strict restoration seam, two-appends ledger and proposed P4/E4/O4 profile before
assigning implementation.

## Independent design acceptance

Coordinator review on 2026-10-10 accepts the separate C/M Derived flow, focused
strict restoration after current aliases are recovered, two logical appends per
detector evaluation, and the exact P4/E4/O4 target profile above. The separate
record keeps node-owned memory attributable under the existing state-root model;
reusing an upstream record would require changing that ownership contract.
Neither record consumes the other. Canonical coalescing does not erase logical
append charges, and real input or patch ancestry remains complete.

The reviewer inspected comparison construction, node inspection, current roots,
state-root encoding and strict role assignment, migration establishment, all
44 heading-scoped edge-contract sources, and the relevant seams in the unchanged
retention/digest/restoration contracts. The amended edge-family contract is
promoted with current evidence hashes. No normative specification enactment or
Rust implementation is part of this planning acceptance. The target version
profile must be enacted and its then-changed contract citations reviewed with
the implementation; the current product remains P3/E3/O3 meanwhile.

The implementation bead now contains a focused accepted brief rather than a
copy of this entire preparation report. No semantic blocker or unresolved
product question remains in this bounded slice. Private helper organization and
append order remain free; record ownership, support relations, canonical profile
and verification obligations are settled. The coordinator will assign the
worker separately and review the complete uncommitted implementation proposal
before creating the accepted product commit. Broader result ownership and
observable-digest changes remain excluded.

Planning validation passed: all 44 promoted source citations report unchanged,
the locked-uv contract tooling suite passed 11 tests, catalogue discovery found
the reviewed record, and `git diff --check` passed. No Rust regression or native
implementation acceptance gate was run for this documentation-only planning pass.
