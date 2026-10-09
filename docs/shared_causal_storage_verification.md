# Shared causal storage: ms-0ap.1 implementation evidence

This records implementation and independent acceptance evidence for `ms-0ap.1`,
based on `6acb565f226bf4f07802e14aa4f01052bb7e80f0`. Specifications and reviewed
contracts remain authoritative; this record introduces no permanent product promise.

## Representation choices

`causal_store` owns immutable semantic records in `Arc` nodes. Each node owns its
predecessors directly and has one frozen canonical version-2 payload and content
digest. Nodes never own a previous view or catalogue. Each view has an explicit
record list and exact-reference membership map. A clone shares its catalogue;
an advance copies catalogue handles and membership entries, then appends nodes.
Artifact views select sufficient backward closures, including transaction
current/change-set facts and every exposed inspection, episode and module cause.

The opaque reference keeps its existing private 32-byte scope and 32-bit ordinal
representation. The scope brands a lineage and an allocation batch; the tuple is
never reused. A logical ordinal alone is not a lookup key. Shared ancestors keep
exact references across ordinary advances, while separately allocated candidate
records remain foreign to a committed view even if they have the same ordinal.
View equality compares scope and selected cause keys, preserving value equality
for repeated owned observations without hashing their graph. The namespace
issuer is private and is absent from persistence, policy identity,
public event serials and both state projections. No global node/weak registry is
needed, so rejected candidates cannot leave registry entries behind.

All nodes are frozen using their creating topology before publication. Later
queries share cached byte buffers. A migration creates fresh source checkpoint
wrappers, preserving the accepted version-2 logical catalogue and full-source
TopologyChange supporters. Earlier source events and retained standard-module
reset/toggle/capture references translate into that checkpoint batch before the
following reaction's batch is created. Old owned source views remain unchanged.
Physical replacement wrappers are not new append-growth units. Restoration
builds fresh private nodes only after validating the supplied semantic graph;
claimed artifact hashes are never imported as trusted cache content.

The final owner releases predecessor chains iteratively. There is no public
Machine clone, mutable forecast escape, root pruning, version bump, policy change,
binding/staging optimization or whole-machine digest memoization.

## Executable references and independence

`state_digest_reference` retains the straightforward uncached version-2 encoder
and full graph/projection walk from the accepted implementation. It recomputes
payloads and hashes from semantic records, finds references by a linear scan,
and ignores production canonical caches and membership maps. Test-only source
metadata retains creating topology definitions for independent recomputation.
Ordinary unit-test stage and restore boundaries compare every frozen node's
complete bytes/hash and both complete canonical state inputs. Snapshot tests
also compare complete envelope bytes using an independently recomputed causal
index. Migration checks recompute source nodes before checkpoint wrapping.

The primitive CBOR writer and semantic enums are shared intentionally. Snapshot
field/envelope encoding is also shared; fixed existing artifact goldens, three
new version-2 migrated-input/artifact vectors, hostile restoration tests and
round-trip continuation tests supplement that limitation. No existing golden
was regenerated. The evaluator and clone-and-swap staging remain unchanged, so
forecast/apply and ordinary replay intentionally share evaluation semantics.
The uncached graph encoder, direct node/membership tests, expected ancestry
fixtures and existing independent public-state law oracles supplement this
limited execution-reference independence.

Integration comparisons normalize distinct private owners through structured
record facts, unordered supporters, port-associated counted contributions and
stamps. They preserve multiplicity, public pending IDs and event order. Bound
projection checks compare references with their own ordinary result and compare
separate machines through semantic state digests. Old raw-reference equality
across separate machines or newly allocated forecast/apply branches is not an
identity guarantee.

## Exercised domain and boundaries

- Twelve counted-merge histories vary input ordering, positive/zero counts and
  repeated reaction times; compare forecast, private clone apply and retained
  artifacts after machine drop.
- Nine generated mixed graphs vary inverter depth and Periodic period, combining
  counted merge, Toggle, SampleHold, PulseDelay, TransportDelay, InertialDelay,
  Periodic and all three stateful standard-module families. Histories include
  equal/exact deadlines, later batches, cancellations, phase preservation,
  repeated times, qualified module observations, quiet patches and restoration.
- Existing tests cover initialization and unchanged facts, contribution
  permutations/overflow, conflicts/episodes, stale identities and digests,
  topology preserve/reset/remove rules, due-before-patch source events, all
  current budget boundaries, bound publication, hostile graphs/versions/stamps,
  snapshots, replay and retained qualified/temporal artifacts.
- Direct tests cover exact ancestor-node sharing, foreign converged machines,
  candidate-only membership, held/dropped artifact independence across a patch
  and future sequence, and 32 discarded forecasts with no live-node increase.
- Test-only provenance, result and projection preparation failures use the
  ordinary structured rollback path. Each failure compares full stored
  observation and snapshot bytes, preserves old owned ancestry and releases all
  newly allocated nodes. The existing bound-projector rejection seam covers
  target bindings and source events after earlier deadline processing.
- A 30,001-node owned closure drops on a 64 KiB stack and frees every node.
  Safe teardown does not imply bounded retention of the current machine's
  complete version-2 logical catalogue.

## Work evidence and remaining costs

The new regression was run against the old production path before modification.
An advance after four prior reactions rewrote 7 old records, hashed the 8-record
view and encoded/hashed 23 canonical records; the zero-rewrite assertion failed.
The new path, at histories of both 4 and 40 reactions, appends exactly one node,
performs zero old-record rewrites or scope-record hashes, and encodes/hashes that
new node exactly once. Repeated state digest and snapshot queries encode/hash no
causal nodes and allocate no namespaces. One namespace is issued per reaction.

Independent review added a growing Toggle ancestry case at histories of 4 and
40 reactions. Each new node is encoded/hashed once, old ancestors remain
physically shared, and complete observable bytes equal the uncached reference.
Separate production counters measure closure records visited and emitted and
canonical payload bytes emitted by observable projection: all grow with the
required stateful ancestry even though causal encoding/hashing remains zero.
The initial check failed at the absent closure instrumentation before those
counters were wired. These counters exclude reference work and isolate a direct
observable projection when comparing closure costs.

Review also removed duplicate closure construction in pending-event inspection:
the returned artifact reuses the owned closure already built for its explanation.

Counters also assert the remaining linear metadata work: copying each prior
catalogue handle and membership entry. Canonical projections still construct
indexes/tables, select closure and emit required canonical bytes. Owned-view
selection scans the logical catalogue. Source migration still wraps the full
selected source catalogue. Transaction staging and bound projection retain their
existing costs. Required ancestry may legitimately grow with history. This
proposal claims elimination of repeated record rewriting and canonical content
work, not bounded history, constant-time advances or a throughput target.

## Validation

Focused regressions, generated differential cases and unchanged/new version-2
vectors pass. Independent acceptance reran both focused cost tests, `make check-dev`
and `make check-final` after the review corrections; all passed. `make check-dev` and the complete `make check-final` gate passed;
their logs are `/private/tmp/ms-check-dev-final.log` and
`/private/tmp/ms-check-final.log`. `git diff --check` also passed.

The final gate used
`QUIET_CHECK=/private/tmp/mossignal-ms-0ap-1-verification/quiet-check` to give
cargo-deny a writable advisory database under `/private/tmp`. Its temporary
configuration is byte-identical to repository `deny.toml` after removing only
that cache-path setting. All standard gate commands and dependency policy checks
ran; no check, advisory fetch, lint or oracle was skipped or weakened. The
repository gate and dependency policy files remain unchanged.
