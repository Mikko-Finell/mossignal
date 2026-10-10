# Binding and successor preparation: ms-0ap.3 evidence

This records the implementation proposal against planning commit `45d8bc3`,
following accepted `ms-0ap.2` at `d0063c5`. It is verification evidence, not
normative authority. Accepted by independent coordinator review on 2026-10-10.

## Implementation choices

The private `BorrowedInputs` helper projects observations against a slice of
immutable published mappings. Bound construction/spawn, rebind and target
reconfiguration retain complete mapping validation. Builder construction still
enforces endpoint existence, signal kinds and uniqueness. Ordinary bound
initialize/advance check exact compiled-definition context, borrow the mappings,
and let ordinary input builders validate the supplied observations. No public
borrowed API or new caller-key trait requirement was added. Caller IDs remain
opaque `Eq + Clone`; lookup remains a linear equality search.

The public standalone `InputProjector` still validates completeness at
construction and owns an independent copy of its input mappings. It uses the
same concrete projection helper for observations. Replacing a bound machine's
maps does not change an earlier standalone projector.

`Machine::stage` continues to duplicate the published predecessor once.
Initialization and ready execution capture the complete predecessor execution
digest and revision before extracting any candidate-owned fields. Initialization
also captures the declared source checkpoint before extraction when carrying a
patch. Ready execution moves owned maps, vectors, episode ownership and the
provenance handle out of this isolated successor into transition locals. It
never drains the live machine or uses an incomplete candidate for identity
projection. Successful publication reconstructs the coherent successor before
computing its E/O identities; rejection drops the entire private candidate.

Earlier internal reactions finish scheduling and episode reconciliation before
their evaluation outputs and causal associations move into the next working
state. The requested-time transition, migration, initialization, forecast,
replay and bound facade all retain the ordinary evaluator/publication path.
Bound projection still uses the producing source/target maps for each event
and finishes before joint machine/mapping publication.

Current-root selection, result and inspection ownership, SOURCE migration roots,
zero pulse normalization, append-growth admission, strict restoration, schemas,
semantic/domain versions and canonical vectors are unchanged. No specification
contract, evaluator, public API, host application or compatibility shim changed.

## Deterministic cost evidence

Counters sit at actual mapping completeness loop bodies, `MachineStore::clone`,
and the retained test-only preparation clone operation. `CountedKey::clone`
counts real input-ID cloning and implements equality without `Hash` or `Ord`.
Collection item counts are top-level lengths, not deep allocation/byte counts.

| Fixture/action | Staged store copies | Preparation copies | Input-ID clones | Completeness slots checked |
| --- | ---: | --- | ---: | ---: |
| Standalone owned projector, 32 inputs | 0 | — | 32 | 32 |
| Bound spawn, 32 inputs + 32 outputs | 0 | — | 0 | 64 |
| Ordinary bound initialization and advances | 1 per reaction | 0 | 0 | 0 |
| Complete rebind | 0 | — | 0 | 64 |
| Target mapping publication | 1 | 0 | 0 | 64 |
| Ordinary reaction after each replacement | 1 | 0 | 0 | 0 |
| Temporal ready transaction, optimized | 1 | 0 operations / 0 items | — | — |
| Same temporal transaction, copy reference | 1 | 41 operations / 134 items | — | — |

The temporal fixture initializes a toggle, enabled periodic source with period
5, and pulse delay of 7. It begins with two pending events. Advance to 16
processes boundaries at 5, 7, 10 and 15 before the requested reaction, preserving
one later periodic obligation. The maintained reference produces the identical
snapshot and predecessor identity.

Before the production change, the binding regression failed with `(32, 32)`
instead of `(0, 0)`. The temporal regression failed with 40 preparation clones
and 133 copied items instead of zero. That first measurement did not instrument
the additional retained provenance handle. The maintained copy reference now
includes that handle, giving 41/134; the optimized path reports zero under both
measurements. Output identifiers still receive the required per-event copies
when producing owned caller projections; these are not whole-map copies.

This demonstrates removal of the selected work, not a throughput or asymptotic
speedup measurement. One initial full store duplication remains. Equality key
lookup, evaluator allocation/settlement, required provenance construction and
closure emission, input validation, state/digest projection and owned output
projection still perform their necessary work.

## Refinement reference and rejection coverage

The test-only copy preparation mode clones each working collection instead of
extracting it. Its scoped guard restores the mode even on unwinding. It shares
the evaluator, scheduling, provenance construction, predecessor capture and
publication logic; it independently exercises collection ownership preparation,
not those shared semantic algorithms.

The reference comparisons cover initialization, zero/positive/even pulse
batches, same-time ordered reactions, earlier deadlines, diagnostic episode
begin/end transitions, preserving patches after source deadlines, stored-state
reset, initialization with a patch, and strict restore followed by continuation.
Twelve generated counted-pulse histories also use the copy preparation mode.
Comparisons include requested/processed stamps, before/after revisions,
before/after E and after O, lifecycle, snapshots, pending state, schedule,
output values/counts and producing identities, transient diagnostics, episode
changes and migration classification. Snapshot equality includes current causal
role associations and complete required machine/episode closure.

The result comparison resolves each exposed event/change cause through the
uncached vector/recursive canonical reference and compares all canonical record
payloads, preserving semantic relation roles, supporter multiplicity and stable
identities while removing allocation namespace differences. Production E/O
also agree with that uncached reference. This reference already existed for
digest verification; its new view helper does not use node content caches.

Actual lifecycle, stale revision/time and runtime/provenance budget rejections
are compared with the copy preparation reference. Existing periodic tests cover
all five budget categories after several candidate deadlines. SampleHold tests
compare exact/below/above event and provenance limits in both lifecycle phases.
The existing broader node, module, transition, restoration, replay and forecast
tests supplement the shared evaluator/reference limitations.

Injected faults occur immediately after state extraction, during provenance
construction, and after complete result construction/projection preparation.
Both lifecycle phases, ordinary/patched transactions, apply and forecast are
covered. Complete predecessor observations compare raw causal associations,
view ownership pointers, lifecycle/time/revision, dense and stateful state,
pending calendar/serial, phase/cancellation state, episodes, snapshots and E/O.
Rejected candidates release newly allocated causal nodes, and retained views
and episodes remain explainable.

The bound facade additionally injects rejection on the first or second output
projection after an earlier deadline, with and without a target replacement.
It checks producing maps, one staging copy, zero preparation copies, exact
target completeness validation, and unchanged machine plus old caller maps.
Borrowed bound projection is compared with the owning projector for missing
snapshot levels, unknown keys, both wrong-kind directions, duplicate levels and
pulses, and conflicting values/counts; each failure occurs before staging.

## Checks and handoff

- `cargo test -p mossignal --lib`: 301 tests passed.
- `make check-dev`: passed after the final test-helper corrections.
- `make check-final`: passed with the advisory-cache relocation below.

Gate runs use the locked repository `uv` environment with a writable temporary
UV cache. The final dependency gate uses the existing temporary wrapper from
`ms-0ap.1` to relocate only cargo-deny's advisory database into a writable cache.
The temporary configuration matches repository `deny.toml` after removing its
single `db-path` setting; advisory/bans/source checks remain enabled.

The first final-gate attempt rejected raw unchecked lookups in two test-only
reference helpers and did not recognize the new helper file as test code.
The file now lives under `transaction/tests`, and the reference lookups state
their invariants explicitly. The existing static guardrail itself is unchanged.

The coordinator reviewed the complete production and test proposal, including
predecessor capture, isolated extraction, mapping publication and reference
limitations. No further implementation correction was required. Independent
`make check-dev` and `make check-final` both passed on the final proposal; the
dependency policy was checked equal apart from advisory cache location. The bead
is closed and this evidence is included in its accepted implementation commit.
Mossmoot dependency integration remains a separate delivery step.
