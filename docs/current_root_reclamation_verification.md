# Current causal roots: ms-0ap.2 implementation evidence

This records the accepted implementation of `ms-0ap.2` against planning
commit `22b34ac`. It is verification evidence, not normative authority.
Independent coordinator review on 2026-10-10 examined root ownership, sparse
source migration, logical growth, role-aware persistence, zero normalization,
and the independent references. No further code correction was required.
The coordinator reran `make check-dev` and `make check-final`; both passed.
The final gate used the existing temporary advisory-cache relocation with
unchanged dependency policy.

## Ownership and implementation choices

`causal_roots` enumerates typed current-machine facts: external level origins,
operation and port support, output baselines, establishment/transition and edge
facts, transport/inertial/periodic scheduling and cancellation facts, pending
origins, current diagnostic evidence, and qualified standard-module latest
reset/toggle/capture causes. A publication selects their complete backward
closure. Current membership does not retain an old full-machine view or a
previous-view chain. The existing immutable DAG and iterative release mechanism
from `ms-0ap.1` remain the storage foundation.

An active episode owns its current evidence closure and retains its beginning
stamp and identity. It has no separate initial-cause root after evidence changes.
A retained original `Began` result independently owns the original explanation.
The episode regression changes real material evidence through a preserving
patch, checks strict restoration, drops the machines, and explains both retained
old and current causes through their respective owners.

Results select every cause exposed by the complete outer event/change stream,
including earlier internal reactions and source-revision events. Inspections,
explanations, episodes and forecasts select sufficient closures independently.
There is no global weak registry accumulating an entry per historical reaction;
catalogue and membership maps belong to finite owning views.

Migration uses an explicit sparse old-cause/new-cause translation. It captures
current SOURCE roots after strictly earlier deadlines and before source removal,
reset or target settlement. Source wrappers preserve canonical creating-topology
facts; an existing source checkpoint fact is reused without nesting another
wrapper. `TopologyChange` supporters are the deduplicated source **content**
identity set. Ancestors and result-only roots belong to selected closures but
are not additional topology supporters. The newly created topology cause is
passed directly to reaction construction. Initialization has no runtime SOURCE
roots; declared source-state checkpoint facts retain initialization and topology
ancestry without fabricating supporters.

Standard-module semantic latest causes are separate from the optional raw
last-reaction inspection cache. Strict restoration preserves the semantic
reset/toggle/capture associations; it does not synthesize a raw reaction cache.
The next ordinary reaction rebuilds that cache while retaining applicable latest
semantic causes. Topology-derived aliases are reconstructed from actual bound
inputs and outputs rather than persisted as redundant role rows.

Explicit zero-count pulse input entries produce no separate observation cause.
They use the same ordinary transaction support as omitted pulse occurrences.
This is the deterministic implementation choice allowed by
`reaction-scoped-pulse-foundation`; it also makes ordinary execution agree with
the required positive-count-only input/replay artifact projection. Builder
validation and duplicate-observation detection remain in place. Initialization,
two later transactions, restoration between them, and encoded/decoded replay
exercise this equivalence. Both zero initialization and encoded replay failed
before the correction; the latter reported
`replay.resulting_execution_digest_mismatch`.

The reviewed contract's `implementation_freedom` says:

> Explicit zero pulse observations may be retained or normalized to absence
> internally because both mean zero in the same reaction; choose one
> deterministic representation, test it, and record the decision without making
> it a persistence promise.

This is a v3 causal adaptation in ordinary initialization and ready reaction
construction, shared by apply, forecast and replay, rather than a replay adapter
exception. It changes which records are produced: accepted v2 construction
implies five facts for two explicit zero entries in the two-input Merge fixture;
v3 measures three
for both zero and omission (transaction, Merge derivation, output derivation).
No appended fact receives a reclamation credit. Each produced logical fact
remains charged once, even if it later becomes unreachable. Forecast and apply
both reject limit two with consumed three and admit limits three/four in both
lifecycle phases; rejected execution preserves the complete predecessor.
Positive counts one/two still create five charged records and emit three pulses.
The route/gate tests compare exact selected positive observation sets and retain
current control ancestry for both positive and zero batches.
An additional resigned-artifact regression gives a zero-pulse role a transaction
fact with the current stamp but an old topology revision. It failed first with
an execution-digest mismatch: the role validator had accepted the contradiction.
The role check now requires the current reaction's stamp and revision (or the
explicit declared-state checkpoint alias), and rejects the forged association
as `persistence.provenance_conflicting_record` before digest comparison.

## Current profile and strict artifacts

| Component | Current value |
| --- | --- |
| Provenance semantics; provenance record domain/version | 3; `/v3`, 3 |
| Execution and observable domains; projection version | `/v3`; 3 |
| Snapshot artifact schema | 3 |
| Implemented replay, transaction and input artifact schemas | 2 |
| Digest suite, canonical encoding and artifact envelope | 2 |
| Core, built-in node, topology patch and diagnostic semantics | 2 |
| Snapshot, artifact integrity and replay-log-content domains/version | `/v2`, 2 |

Snapshot `provenance.current_roots` records genuinely missing stable qualified
subject/role-to-CauseDigest associations. It is sorted and unique. Existing
authoritative input/state/pending/baseline/episode fields and topology aliases
remain the source for already represented associations. Execution projection
commits the non-derivable causal-role identities needed by future migration and
freshness, without serializing the derivation graph, raw reaction cache or
persistent pulse values. Observable and snapshot projections emit their complete
required causal closure.

Restoration validates exact role coverage and record kind/owner/stamp coherence,
role ordering and uniqueness, complete acyclic provenance, source checkpoint
record shapes and semantic versions, and source/outer predecessor correspondence.
It recomputes canonical record identities and both state digests before
publication. A generic transaction/snapshot checkpoint cannot replace missing
current operation support. Nested source facts are parsed iteratively with
depth/record charges; unsupported provenance components are rejected even when
the enclosing integrity and graph identities have been recomputed.

The hostile tests cover missing, duplicate, unsorted, wrong-owner, wrong-kind and
contradictory current roles. The forged checkpoint predecessor test recomputes
all affected graph identities before resigning the envelope, so its rejection
reaches checkpoint ancestry validation. Existing hostile closure, temporal,
episode, budget, framing and version-vector cases remain enabled. Replay checks
each kind-specific component vector through nested frame, transaction and input
envelopes and still materializes ordinary transactions.

Fifteen affected canonical vectors were regenerated after uncached record and
E/O comparisons passed. An independent small CBOR reader additionally checked
the exact mixed version/domain profile, source-record semantics, ordered unique
current-role rows and nested replay envelopes. Digest pair files are raw 64-byte
values, not CBOR. The migrated fixture names now say `v3`; no historical decoder,
translation or compatibility shim was added.

## Executable evidence and independence limits

The detached Vec/map reference independently enumerates typed roots and stable
role subjects, walks predecessor closure, constructs source facts and translated
ancestry, and encodes/hash-checks canonical records and full E/O inputs. It does
not use production root collection, sparse cause lookup or cached canonical
payloads/hashes as its oracle. Ordinary test builds call the independent
comparison and root/acyclicity checks during successful publication and migration.
The migration boundary also independently enumerates actual typed SOURCE facts
after earlier deadlines and compares them with the production source manifest
before wrapper selection. It does not accept that manifest as its root oracle.

Its node evaluator, compiled network and input/state semantics remain shared
with production. The private clone/forecast comparison is a second publication
path, not an independent node-semantics oracle. Snapshot/envelope writers are
also shared; the independent field reader, strict re-encoding, hostile resigned
artifacts, frozen vectors and future direct/restored/replayed continuations
supplement that limitation. This proposal claims independent root selection,
source translation and canonical causal encoding, not a separately implemented
runtime for every primitive.

The accepted v2 baseline counterexample measured 19 equal unlabeled roots and
identical snapshots despite different qualified latest-toggle associations.
The v3 acceptance fixture, with zero pulse causes normalized to absence, has
17 equal unlabeled roots and complete ancestry. Its qualified latest-toggle
associations differ and E3/O3/snapshots differ immediately. Identical selective
updates then produce different source roots and migrated pending cause identities
while pending identity/origin/deadline/count agree. The same expected-execution
guard accepts one machine and atomically rejects the other. Direct, strict-restored
and encoded/decoded replayed histories agree, including two preserving patches
with restoration between them and an unrelated pending pulse.

The allocation/coalescing regression supplies three canonically identical source
allocations, while strict restoration coalesces them to one. It checks equal
snapshots, E/O and logical growth across two real preserving patches and a restore
between patches. Original source ancestry remains explainable after machine drop.
Private allocation counts never define semantic supporter multiplicity or budget
consumption.

No supported independently variable required-observation facet has been
established for equal-E/different-O evidence in this profile. No such witness is
claimed. The former inconsistent baseline mutation now explicitly tests O
sensitivity and strict-restore rejection. Domain separation, checkpoint
sensitivity, full canonical inputs, included-state sensitivity,
policy/presentation/optional-history/allocation exclusions, snapshot metadata,
inspection purity and rejection purity remain tested. A future independently
variable required O facet requires its own separation witness.

## Reclamation, work and logical growth

The quiet/stateless regression was observed failing before reclamation: by the
third reaction it retained six records against a bound of five. Forty dropped
reactions now bound both current catalogue/membership and live nodes by the
current required closure. Held results, inspections and forecasts deliberately
keep their own ancestry alive; they do not change machine snapshots, patch
supporters or future digests. Thirty-two discarded forecasts release candidate
nodes. Machine drop preserves retained-artifact explanations. The existing
30,001-node chain still tears down on a 64 KiB stack.

At histories of four and forty quiet reactions, an advance creates exactly one
node, encodes/hashes it once, and rewrites/hashes no old records. Repeated pure
digest/snapshot queries create no nodes or namespaces and perform no causal
encoding/hashing. Growing Toggle and periodic phase ancestry remain retained;
their required observable closure visits, records and bytes legitimately grow.
Current-view handle/membership copying and canonical closure enumeration can
still be linear in the **required** closure. Transaction staging and binding
projection retain their earlier costs. There is no whole allocation-arena sweep,
constant-time emission or measured throughput claim.

Growth is a transaction-local logical append count, including intermediate
reactions whose records are later superseded. Equivalent source wrapper
replacements are uncharged replacements; new topology/migration/reaction facts
are charged. A quiet level wire appends one Ready fact even when that fact is
immediately unneeded by current M. Limits zero/one/two reject below and admit
exactly/above this independently counted total, preserving the full snapshot
on rejection. A two-node periodic transaction performs three earlier due
reactions and the final requested reaction: `3 * 8 + 9 = 33` logical appends.
Limits 32/33/34 reject below and admit exactly/above; retained result explanations
cover this full outer transaction rather than net live growth. Coalescing and external artifact
ownership do not reduce the count.

Failure checks compare lifecycle, topology, revision, time, level/node/temporal
state, pending keys, baselines, module semantic causes, current roles/provenance,
episodes, E/O and complete snapshot bytes. Existing provenance/result/projection
fault seams and bound projection failure preserve the predecessor and retained
views, including after earlier deadlines. Candidate causes remain outside
predecessor membership.

## Validation

The final proposal passes the focused zero-normalization/growth tests,
27 strict snapshot/restoration tests, the two current-root integration tests,
36 atomic replacement tests, eight Level-controlled pulse tests, and the
periodic 33-append boundary test. Earlier full library, generated/reference,
retention, migration, ownership, replay and golden checks are included again in
the finite repository gates.

Final runs:

- `make check-dev`: `/private/tmp/ms-0ap2-check-dev6.log`, exit zero,
  `OK: development checks passed`.
- `make check-final`: `/private/tmp/ms-0ap2-check-final4.log`, exit zero,
  `OK: final checks passed`.
- `git diff --check`: passed after the final code changes.

Both gates use the locked repository Python environment with
`UV_CACHE_DIR=/private/tmp/mossignal-contract-uv`. The final gate additionally uses
`QUIET_CHECK=/private/tmp/mossignal-ms-0ap-1-verification/quiet-check`, the accepted
previous-slice wrapper that changes only cargo-deny's advisory database location
to writable `/private/tmp`. The default dependency check failed to lock its
read-only home cache; the relocated check ran successfully. The temporary
configuration is byte-identical to repository `deny.toml` after removing only
the `db-path` line. All standard commands, advisory fetching and dependency
policy checks run, without ignored advisories, disabled tests, lint relaxation
or oracle bypasses. Repository gate/policy files are unchanged.

Acceptance synchronization is intentionally deferred to independent coordinator
acceptance while the bead remains `in_progress`. No staging, implementation
commit, parent closure, push or Mossmoot change is included in this handoff.
