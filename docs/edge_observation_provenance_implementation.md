# Edge observation provenance implementation and acceptance

Implementation bead: `ms-49a.2`. Planning acceptance: `ms-49a.1`.
Prepared on `main` from `a8eb40380b40fcda0ba25da4b848f3956a444c1e` on
2026-10-10. This is implementation and acceptance evidence, not an authoritative
specification. The coordinator independently accepted the complete proposal on
2026-10-10. The accepted product commit records the implementation and delivery
beads closed together with the synchronized issue export.

## Result and change surface

Each evaluated RisingEdge, FallingEdge and AnyEdge now appends two node-owned
Derived records. The current comparison C consumes the admitted reaction T,
settled input I and established previous memory when present. Successor memory
M consumes exactly T and I. The previous memory is read before its replacement.
C remains the current operation/pulse-output premise; M becomes the remembered
observation premise. Both logical appends charge even when first-reaction
canonical content coalesces. Numerical laws and declared initialization/reset
premises remain as accepted in the bounded design.

Production changes are in `transaction.rs`, strict recovery in
`snapshot_restore.rs`, and current profile writers/readers in `identity.rs`,
`state_digest.rs`, `persistence.rs` and `replay.rs`. Existing compiled
`edge_input_operation` correspondence was sufficient. Production inspection,
migration, causal-root selection, evaluator laws and the ownership architecture
needed no representation change.

Restoration recovers current aliases before checking every edge memory owner,
Derived kind and exact canonical T/I supporter set. The comparison must be
owned by the same node, include the same T/I premises and satisfy current-role
validation. Cause-to-digest correspondence is built from the validated restored
view; private allocation positions are not treated as content identities.
Ordinary initialization/ready, current target TopologyChange, and initialization
with a patch are admitted establishments. The latter requires exactly current
initialization and target TopologyChange supporters at the committed time,
same-time order and revision. Complete DAG/SOURCE, settled-value, version,
current-role and recomputed digest checks remain in force.

The required initialization-patch witness exposed an existing current-role
restoration gap: a comparison whose direct reaction supporter was the declared
checkpoint was rejected as `cause kind/owner`. The new test was observed failing
before correction. Current-role and memory checks now share the same checked
establishment recognizer. A generic declared-state checkpoint no longer bypasses
the stamp/revision/target checks. This was necessary integration within the
accepted initialization-patch scope, not a change to the design.

Test infrastructure adds an actual `Records::owned_closure` visit counter and a
test-only immutable-record append helper for fully re-signed semantic forgeries.
The existing complete-predecessor observation is reused by edge rollback tests.
Existing clone-preparation/fault fixtures and the late bound-projection fixture
now contain an edge, so their deadline, episode, staging and publication checks
also exercise C/M ownership. `binding.rs` changes are entirely test code.

## Failed-before regression and semantic witnesses

The first new test,
`remembered_observation_does_not_inherit_obsolete_comparisons`, was run before
any production correction against the accepted old implementation. It failed
at `Rising/Baseline/Low, reaction 1`: the memory record had **3** direct premises
instead of **2**. After separating the two facts, the same test passed.

The detached support oracle constructs expected M={T,I} and C={T,I,M_prev}
sets from the admitted reaction, compiled input correspondence and the supplied
previous observation fact. It does not invoke evaluation, provenance appending
or production root selection. Initial C/M coalescing is checked by canonical
support, not handle inequality. Ordered and patch tests use explicit three-kind
transition tables, including declared Baseline/Assume rules.

| Coverage | Concrete witness |
| --- | --- |
| Quiet histories | 36 fixtures: constant or omitted external Level, three kinds, Baseline/Assume Low/High, both levels; each runs 1, 10 and 1000 quiet reactions with an unrelated positive Pulse branch |
| Finite laws | Existing exhaustive first/previous/current matrices, actual transitions, current-state isolation and reconvergent glitch tests pass with the new construction |
| Ordered continuation | Low/High/Low, empty/equal reassertions at one physical time; direct, forecast, encoded-restored and encoded-replayed executions agree |
| Topology | All 3×3 kind changes, all target initialization policies, preserving and reset paths, two patches with intervening encoded restore; equal/unequal rewiring and owner reassociation |
| Qualified ownership | Module-qualified edge roots restore initially, after a preserving patch, and after an explicit internal reset |
| Initialization patch | Each kind and initialization policy uses the declared checkpoint; encoded restore and a subsequent same-time reaction agree |
| Independent artifacts | Earlier actual pulse result, node inspection and forecast survive 1000 later quiet reactions and machine drop; complete result canonical content remains equal, roots still resolve, and releasing final owners frees all nodes |
| Genuine history | An edge observes a Toggle driven by real pulses; every previous pulse observation stays reachable and the required closure grows |
| Strict restoration | Eleven re-signed forgeries: comparison-shaped memory, swapped roles, stale memory, wrong kind, wrong owner, missing I, missing T, wrong I, extra comparison, wrong current order and wrong current time |
| Nested profile | Unsupported P2 and P3 SOURCE facts reject; existing rehashed false-SOURCE correspondence rejection remains active |
| Failure | Complete predecessor, caches, roots, pending identities, episodes and held views survive budget, overflow, extraction, provenance, digest, result and projection failures, including earlier deadlines, patches and late bound projection |

The stale memory and malformed root tests regenerate canonical graph hashes,
both machine digest claims, snapshot digest and envelope integrity. Decoding
therefore succeeds for the forged current-profile graphs; restoration rejects
their causal roles through provenance failures before a digest mismatch can
conceal the semantic seam. Missing edge-root and duplicate/conflicting role
coverage remains in the existing strict restoration suite. Valid first-reaction
coalescing and qualified roots restore without historical replay.

## Production work and exact budgets

Counters are sampled immediately around the next production apply, after
dropping its result, before observers/reference checks. Fixture construction,
semantic oracle walks and later snapshot/reference work are outside that sample.
The reference canonical encoder uses separate uncached Vec/map traversal and
does not increment these construction counters.

The next measured ordinary quiet step at physical time 2000 has the following
counts at history lengths 1, 10 and 1000, for every quiet fixture:

| Counter | Omitted external Level | Constant node |
| --- | ---: | ---: |
| Machine catalogue / live machine nodes | 9 | 10 |
| Created / released nodes | 6 / 6 | 7 / 7 |
| Logical growth | 6 | 7 |
| Canonical encodes / hashes | 6 / 6 | 7 / 7 |
| Catalogue handles / membership entries copied | 9 / 9 | 10 / 10 |
| Owned-closure records visited | 18 | 20 |
| Projection closure records visited / emitted | 18 / 18 | 20 / 20 |
| Namespaces allocated | 1 | 1 |
| Old record rewrites / scope record hashes | 0 / 0 | 0 / 0 |

Representative Low/Baseline samples emitted 3724/4106 closure bytes respectively.
Byte length is not asserted identical across old reaction times because canonical
integer widths vary. Record/copy/encode/hash counts are asserted identical across
the three history lengths. With the fixed earlier artifacts held, the current
snapshot, catalogue and next-step construction/copy/closure counts equal the
dropped-artifact run. Physical live ownership additionally includes those held
closures and falls to zero after both machine and artifacts are dropped.

Explicit logical ledgers, each tested one below, exactly at and one above:

- Ordinary initialization: T + input + C + M + two output facts = **6**, including
  all Baseline/Assume first-fact coalescing cases. A ready unrelated pulse step
  similarly costs T + pulse + C + M + two output facts = **6**.
- Initialization with a patch: initialization + topology + declared checkpoint
  + input + C + M + two output facts = **8**.
- Exact transport deadline: T + transport + C/M + two pulse output facts +
  changed Level output = **7**. A later quiet target adds **6**, so one outer
  transaction crossing the deadline costs **13**.
- Patch at the exact deadline: topology + new input + baseline migration + edge
  migration + transport migration + pending migration + pending record = **7**,
  followed by target settlement **7**, total **14**.
- Patch after the earlier deadline: source settlement **7** + migration without
  pending **5** + quiet target **6**, total **18**.

Equivalent SOURCE checkpoint replacements remain uncharged. Physical node
creation differs from logical growth in patch cases; for example the exact
deadline patch creates 24 nodes but charges 14 logical records. No sharing,
reclamation, held artifact or canonical coalescing provides budget credit.

## Canonical profile and vector rationale

Persistence section 16.10 now enacts the accepted **P4/E4/O4** profile and the
current edge role/strict restoration seam. Provenance meaning, record payload
component, record label/domain, E/O projection fields and E/O label/domains are
4. Snapshot schema remains 3; replay/input/transaction schemas and every other
listed semantic/envelope/encoding/suite component remain 2. BLAKE3, canonical
CBOR grammar, snapshot/integrity/replay-log-content v2 domains and other identity
projections remain unchanged.

Fifteen existing vectors are updated: declared/ready E/O inputs and their two
digest-pair vectors; uninitialized/families snapshot bytes and snapshot digests;
three migrated machine vectors; replay frame/log bytes. The three migrated
fixtures and their references are renamed from `_v3.hex` to `_v4.hex`. The new
`machine_snapshot_edge_roles_v4.hex` captures distinct C/M roles after a genuine
transition and a quiet step.

Regeneration followed exact uncached machine comparisons and independently
parsed snapshot profile/content checks. Temporary generation hooks were removed;
the permanent tests compare the checked-in bytes. Parsed E/O tests assert exact
literal v4 domain labels, domain version 4 and projection version 4; the separate
snapshot parser checks P4 record/envelope/payload components and record hashes.
Seven authored fingerprint vectors and both diagnostic inventories were checked
byte-for-byte against HEAD and remain identical.

Five affected contracts are independently **reviewed**: machine-state-digests,
machine-snapshot-artifact, machine-snapshot-restoration, replay-artifacts and
atomic-topology-replacement. Independent review reused 359 unchanged citations,
checked the six changed entries (five current-profile citations plus the snapshot
version-responsibilities parent section), and refreshed their evidence hashes.
All 365 citations now report unchanged at the reviewed proposal baseline. The
independently reviewed edge-family contract remains reused.
The contract coverage diagnostic has zero unresolved sources and the one
pre-existing unused provenance-retention alias; heuristic coverage is not a
semantic completeness claim.

## Verification and remaining limits

Both `make check-dev` and `make check-final` passed on the final source and
fixtures with the cache relocation described below. The combined Make invocation
ran their shared prerequisites once and printed both success lines.
Formatting, all-target/all-feature compilation, guardrails, Clippy with warnings
denied, all 628 nextest tests, Python tool tests, doctests, documentation and
advisories/bans/sources checks passed. `git diff --check` also passed.
Focused integration verification passed **72 tests** across edge detectors,
ordered reactions, current-root reclamation, atomic topology replacement,
replay artifacts and transaction forecast. Focused production edge tests,
clone-preparation, complete fault rollback and late bound projection passed.
The locked contract-tool suite passed 11 tests. The full finite nextest inventory
contains **628 tests across 31 binaries**; neither filtered zero-test runs nor
unavailable commands are counted as passes.

The default UV cache and cargo-deny advisory lock are outside writable sandbox
paths. UV used `/private/tmp/mossignal-planning-uv-cache`. The final Make gates use
a temporary cargo dispatcher under `/private/tmp/mossignal-ms49a-verification/bin`
that forwards every command unchanged except adding a deny configuration whose
sole parsed difference is the writable advisory `db-path`. The normal advisory
fetch remains enabled. No dependency, ignore, check category, gate or repository
policy was weakened, and no temporary helper is part of the proposal.

The retention claim is limited to manufactured predecessor comparisons in the
demonstrated quiet edge fixture. Real cumulative input support, topology/SOURCE
history, output baselines and deliberately held artifacts can retain history.
Observable/snapshot encoding still emits the required closure; existing namespace
copying/freezing/remapping costs remain. The canonical reference and
clone/replay/forecast refinement share production record construction and cannot
independently prove semantic premises; the detached support oracle supplies that
separate check and still relies on validated input-operation correspondence.
The finite tests do not prove arbitrary unbounded networks or histories.

## Independent acceptance

The coordinator reviewed the production correction, strict restoration seam,
current profile synchronization, regression and continuation witnesses, actual
cost counters, exact budgets and vector rationale against the accepted bounded
brief. No further code correction was required. The initialization-patch
establishment recognizer is a necessary integration correction within that brief.
The five changed contracts were independently promoted before final acceptance.

The coordinator separately ran `make check-dev`, then `make check-final`, and
both passed. Independent focused runs passed all 72 integration tests named above
and all 10 unit tests selected by `edge_observation`. The final gate includes all
628 nextest tests, the locked Python tool suites, guardrails, denied-warning
Clippy, doctests, documentation and dependency policy. The temporary deny
configuration was independently parsed and compared with `deny.toml`; its sole
difference is the writable advisory database path. No check was weakened.

The coordinator closed `ms-49a.2` and its bounded delivery parent `ms-49a`, flushed
the issue export and committed the accepted native result. No push or downstream
adoption is part of this acceptance; Mossmoot retains its previous dependency pin
until distribution and the host cutover witness are resolved.
