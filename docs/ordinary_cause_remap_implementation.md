# Ordinary cause-reference remapping implementation

Implementation evidence for `ms-efv`, independently accepted on 2026-10-10.
This record is not a normative specification.

## Baseline and scope

Work began on clean `main` at planning acceptance commit
`03531d6a1e1478581ffab127875671537c459af0`. The product baseline is
`bdf70ae6c793e2872e11fe652280cf2a01d1990d` (accepted `ms-49a.2`). The coordinator
assigned implementation, including bead closure and export after successful
checks, while retaining the complete proposal uncommitted for independent review.
The eleven reviewed contracts listed in `ordinary_cause_remap_preparation.md`
remain unchanged; no new specification fact or draft contract was needed.

Production changes are confined to `transaction.rs` and
`standard/stateful.rs`. Ordinary pending, output, diagnostic and cancellation/
phase cause sweeps are removed. Required successor-owned maps copy their existing
handles directly; prior output and periodic-anchor maps are borrowed. Due and
cancellation supporters carry retained handles. New records go directly to
`Records::push`, removing the duplicate supporter/contribution traversal and
its unused private scope parameters. Standard reset/toggle/capture roles carry
their retained handles without a rebase callback. No replacement identity walk
was introduced.

Membership, predecessor validation and Arc ownership in `Records::push` remain.
The `causal_store.rs` edit only counts that work under `cfg(test)`. Required root
selection, closure ownership, canonical encoders, structural reassociation,
restoration and checkpoint/SOURCE translation remain intact. The separate edge
comparison and successor-memory records, including both logical appends, are
unchanged. No public API, schema, profile, domain or version changed; P4/E4/O4
and all existing vectors are preserved.

## Regression first and eliminated work

Before production removal, counters were placed at the actual old remap calls
and traversal bodies. The first two permanent tests ran with that instrumentation
against the accepted production behavior. Both failed their zero-obsolete-work
assertion after completing all semantic checks and measurements: **0 passed,
2 failed**, with six fixture/mode combinations in each test. After correction,
the same tests passed; the completed focused suite contains **6 passing tests**.

Counters are read immediately after apply/forecast, before inspections, snapshots,
detached canonical references, restore or replay work. The following baseline
counts were identical for apply and forecast. Every listed obsolete counter is
zero after removal at all three sizes.

| Actual obsolete boundary | Init, 1/8/32 groups | Jump to 5, 1/8/32 groups |
| --- | --- | --- |
| Same-lineage remap attempts | 103 / 152 / 320 | 511 / 707 / 1379 |
| Pending cause visits | 6 / 13 / 37 | 40 / 152 / 536 |
| Accumulated output visits | 0 / 0 / 0 | 26 / 26 / 26 |
| Accumulated diagnostic visits | 0 / 0 / 0 | 5 / 5 / 5 |
| Cancellation/phase map visits | 1 / 1 / 1 | 4 / 4 / 4 |
| Carried mutable map remap visits | 0 / 0 / 0 | 52 / 52 / 52 |
| Read-only map reconstruction visits | 0 / 0 / 0 | 24 / 24 / 24 |
| Standard causal-role remap visits | 0 / 0 / 0 | 24 / 24 / 24 |
| New-record supporter/contribution visits | 96 / 138 / 282 | 331 / 415 / 703 |

Category visits and generic attempts describe different boundaries and are not
additive totals. With the old loops deleted, their category counters have no
remaining production increment. The generic same-lineage counter remains at
`remap_cause`, alongside a separate nonidentity reconstruction counter. Source
audit supplements these measurements: a future identity-only walk bypassing
that helper would need corresponding accounting. These bounded measurements
do not claim a universal runtime speedup or zero total work.

## Necessary work retained

These measured costs are identical before and after removal for the original
fixtures, apart from the newly separated direct-map-copy count (the old run
counted its visits as obsolete remapping, not as direct copies). Apply and
forecast have the same counts within each row.

| Work | Init, 1/8/32 groups | Jump to 5, 1/8/32 groups |
| --- | --- | --- |
| Nodes / logical growth / encodes / hashes, each | 53 / 81 / 177 | 133 / 189 / 381 |
| Predecessors resolved and owned | 96 / 138 / 282 | 331 / 415 / 703 |
| Direct successor cause entries copied | 0 / 0 / 0 | 52 / 52 / 52 |
| Catalogue handles / membership entries copied, each | 0 / 0 / 0 | 521 / 773 / 1637 |
| Owned-closure visits | 106 / 162 / 354 | 224 / 308 / 596 |
| Projection-closure visits | 106 / 162 / 354 | 192 / 276 / 564 |
| Projection records emitted | 98 / 154 / 346 | 190 / 274 / 562 |
| Projection bytes emitted | 26644 / 39468 / 83436 | 54804 / 74418 / 141666 |
| Namespace allocations | 1 / 1 / 1 | 4 / 4 / 4 |

Old immutable-record rewrites and scope-record hashing remain zero for these
ordinary transitions. Retained future obligations still incur legitimate
catalogue and closure costs. Removal earns no logical-growth credit and changes
no budget threshold.

## Exercised semantics and boundary controls

The new transaction fixture uses 1/8/32 far-future PulseDelay groups, short
PulseDelay, TransportDelay set/reset controls, InertialDelay, Periodic with
preserved phase, a retained-conflict latch and three standard modules. Init at
0 schedules `groups + 5` pending events. A transaction at 1 adds future work
and establishes retained standard roles: three reset, two toggle and one
capture, including accepted even toggle batches.

Advancing directly to 5 processes distinct deadlines 2, 3 and 4, then the quiet
target reaction at 5. The latch episode begins at 3 and resolves at 4; at least
four earlier output events accumulate. `2 * groups + 2` events survive, including
future pulse work, inertial maturity and periodic work. Apply and forecast have
exact matching snapshots and digest observations, while forecast preserves the
complete predecessor observation. Surviving input, pending and standard-role
handles are unchanged, resolve through exact membership and share actual nodes
with the previous view. Detached canonical machine checks also pass.

At time 5, disabling set/enable cancels inertial time 6 and periodic time 7.
Both real cancellation derivations contain the canceled cause. Reenable retains
periodic phase and creates inertial time 11; the jump to 12 processes 7, 11 and
12, with nonzero predecessor ownership, successor copying and unchanged logical
growth accounting. Together the fixture executes all four due-event families.

Encoded restoration demonstrably reconstructs another private lineage: the
seeded fixture counts 151 nonidentity remaps and preserves its exact snapshot.
A qualified metadata-only preserving patch at time 5 runs source deadlines
2/3/4 before migration, counts 302 source translation lookups on the direct
machine, retains qualified `module_node` SOURCE facts and matches the restored
machine. The first patch wraps ordinary source facts without rewriting old
checkpoint records. A second preserving patch translates real existing
checkpoint predecessors (93 rewrites and 636 lookups on the direct machine).
Direct/restored snapshots remain equal; after another encoded round trip,
ordinary apply/forecast continuation through 8, 15 and 101 again counts zero
obsolete remaps and matches exact snapshots. Physical allocation/rewrite counts
may differ after canonical coalescing on restoration; private handles are not
required to match across that boundary.

Held results, node/module inspections, an active episode and a forecast survive
later execution and machine drop. Their saved causes still resolve and original
canonical result records stay unchanged. Held versus dropped owners produce the
same machine snapshot, canonical root manifest and measured work, except physical
node release timing. Held owners keep additional physical nodes alive; releasing
the last owners reclaims all tracked nodes. This does not require collected causes
to resolve through the latest machine.

Ten preparation-fault cases cover extraction, provenance, digest preparation,
result and projection, with and without a preserving patch requested after
earlier deadlines. Extraction rejects before deadline evaluation; provenance
rejects during the first internal deadline. Digest, result and projection faults
occur after the earlier deadlines and, in the patch cases, after actual SOURCE
translation. Coverage assertions distinguish these reached stages. Each case
preserves the complete warmed predecessor observation, live ownership and held
cause access. Existing edge controls retain exact C/M support, append
ledgers, coalescing, strict restoration, held-artifact behavior and budget/overflow
rollback. Existing temporal integration tests retain real exact-limit, one-below,
event/time overflow and late-deadline rejection checks.

## Reference limits and validation

The maintained clone-preparation reference shares evaluation and publication;
it cannot independently detect deletion of a remap call. Work instrumentation,
raw identity and shared membership, independent canonical recomputation, held
ownership tests, direct/restored continuation and unchanged existing vectors
provide additional controls. Canonical references share the specified grammar
and domain constants. Existing direct/stepwise tests normalize only permitted
reaction partition differences; this change adds no stronger equality claim.

- `cargo test -p mossignal --lib ordinary_remap_ -- --nocapture`: 6 passed.
- Focused integration suites: 114 passed across atomic replacement (36), current
  root reclamation (2), diagnostic inspection (3), edge detectors (13), ordered
  reactions (5), replay artifacts (13), standard stateful (14), temporal foundation
  (25) and transaction forecast (3).
- `cargo test -p mossignal --lib edge_observation`: 10 passed, including strict
  restore rejection, exact append budgets, overflow and held-artifact controls.
- `cargo test -p mossignal --lib clone_preparation`: 2 passed.
- `make check-dev`: passed.
- `make check-final`: passed, including static guardrails, clippy, the complete
  finite test suite, tooling tests, doctests, documentation and dependency policy.
- `git diff --check`: passed.
- `git diff bdf70ae6c793e2872e11fe652280cf2a01d1990d --exit-code --
  crates/mossignal/tests/golden docs/specs`: empty, exit 0. Other existing test
  files and inline vectors are unchanged.
- Final source audit confirms ordinary remap helpers/callbacks are absent and
  real restoration, checkpoint/SOURCE and structural-key translations remain.
  HEAD remains the planning acceptance commit; no code was staged or committed.

Gate commands use the locked uv environment with a writable temporary cache.
The existing temporary cargo wrapper redirects only cargo-deny's advisory DB
path to a writable temporary directory. Parsed policy configuration is otherwise
identical to repository `deny.toml`; advisory fetching remains enabled. No
repository gate, dependency policy, oracle or vector was weakened.

Raw execution logs are temporary implementation-session artifacts under
`/private/tmp/ms-efv-*.log`; the durable measurements and outcomes are recorded
above.

## Independent corrective acceptance

The coordinator reviewed the complete production/test proposal, governing
reviewed contract rules, actual work boundaries and surviving real translations.
No production correction was needed. Review corrected the fault-stage evidence
and strengthened the preparation-fault test to distinguish extraction before
deadlines from actual deadline work and publication failure after SOURCE
translation. This is an evidence/coverage correction, not a semantic change.

The corrected six-test suite, `make check-dev` and then `make check-final` passed
independently. The coordinator compared all twelve before/after measured
fixture/mode pairs: obsolete work is zero, while recorded necessary growth,
predecessor ownership, copies, encoding and closure work remain identical.
Specification and golden diffs against the product baseline remain empty;
diff checks pass. Only this bead changed in the tracked export, and it remains
closed. The accepted commit includes this implementation, its tests/evidence
and the synchronized bead. No push or Mossmoot dependency change is authorized
by this acceptance.
