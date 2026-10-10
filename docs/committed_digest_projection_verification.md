# Committed digest preparation and projection verification

Implementation handoff for `ms-0ap.4`, based on accepted planning commit
`29168d54f6eae315e7328ae9e4cc783c6b91cd6d`. Accepted by independent coordinator review on 2026-10-10. No Mossmoot
dependency update or push is included in this native acceptance.

## Implemented choice and coherent completion

Each completed MachineStore carries one private execution/observable digest
pair. The pair contains only the two value identities: no causal owners, previous
machine, closure index or snapshot bytes. Cloning the isolated staging store
copies those values. Admission/freshness and original before-E capture use the
complete predecessor; only then is the candidate pair invalidated. After all
semantic fields and current roots are installed, the candidate recomputes both
raw projections and assigns the completed pair together. Result identities read
that pair before the ordinary core or joint binding publication boundary.

Spawn prepares declared-state E/O. Initialization, ordinary advancement and
topology replacement all share the same successful completion helper. Forecast
uses the ordinary stage and owns its prepared hypothetical pair. Restoration
builds an unprepared private machine, validates state, pending work, episodes,
settlement and re-encoded causal content, then derives E/O and compares artifact
claims before return. The temporary restoration skeleton does not prepare an
unused declared-state pair. A query on an incomplete private candidate is an
internal invariant violation, with an explicit panic message; no public machine
is returned in that state.

Raw execution encoding now reads each represented role, pending-event or
episode cause's frozen content digest through its owning view. It constructs no
full causal table, indexes no ancestry and emits no derivation records. Missing
frozen content is a committed-state invariant violation. Raw observable encoding
and snapshot creation retain the existing complete root-selected index, exact
collision checks, content coalescing and canonical record emission.

These are private implementation choices under unchanged reviewed contracts.
Public API, policy identities, canonical formats/versions and checked-in vectors
are unchanged. No independently variable coherent equal-E/different-O facet is
claimed; the baseline-corruption fixture continues to test observable sensitivity
and strict rejection of incoherent state.

## Regression and deterministic cost evidence

Both cost regressions were run and observed failing before implementation:

- Twenty public query pairs on declared state built 20 execution and 20
  observable inputs, performed 40 outer hashes and constructed 40 contexts.
- Raw E after four Toggle reactions indexed and visited 13 causal records.

Logs are `/private/tmp/ms-0ap4-query-regression.log` and
`/private/tmp/ms-0ap4-raw-e-regression.log`.

`projection_work` counts actual input builders, outer machine hashes, index
entries, closure visits, direct frozen-cause resolutions and emitted record
bytes. It is test-only and separate from causal-node hashing/construction and
binding/staging counters. Independent assertions and the shared snapshot field
writer's reference invocation suppress only these new measurements through a
scoped guard; assertions still run, including on unwinding. This avoids counting
reference verification as optimized production work.

Twenty repeated E/O pairs now perform zero work in every new counter on declared,
initialized, ready, restored and forecast states, including patched successors.
Initialization and zero/positive same-time or later-time reactions build/hash E
and O exactly once each. Both declared and ready restoration prepare exactly one
pair from validated state. Original before-E agrees with the stored predecessor.

The growing-ancestry fixture holds its initialization result, executes 4 or 40
reactions, and compares forced raw E/O against the detached encoder. Its Periodic
variant feeds an enabled period-one source into Toggle. All raw E cases build and
hash once, with zero contexts, indexed/visited/emitted records and emitted bytes.

| Fixture / history | Raw E current-cause resolutions | Raw O and snapshot records indexed/visited | Canonical records emitted | Record bytes emitted | Snapshot artifact bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| Toggle / 4 | 4 | 13 | 13 | 2,521 | 4,343 |
| Toggle / 40 | 4 | 121 | 121 | 23,649 | 25,473 |
| Periodic + Toggle / 4 | 7 | 29 | 25 | 6,320 | 8,938 |
| Periodic + Toggle / 40 | 7 | 245 | 205 | 53,890 | 56,517 |

Each snapshot builds one required closure context and no machine E/O input or
outer hash. Its emitted canonical records/bytes agree with raw O; full artifact
bytes agree with the maintained reference projection. Indexed allocations can
exceed emitted canonical records because byte-identical content coalesces.
Measurements are in `/private/tmp/ms-0ap4-digest-tests.log`.

The earlier accepted cost/ownership tests remain required: stateless histories
4/40 encode/hash only new nodes, rewrite/hash no old records, and retain bounded
current ownership when artifacts are dropped. Deliberately held results,
inspections and forecasts retain sufficient independent ancestry without changing
machine identity. The 30,001-node iterative-release case remains part of the
suite. Ordinary bindings repeat no complete proof or caller-key copies; staging
uses one store clone and zero working-collection copies. The temporal copy
reference still performs 41 preparation copies / 134 copied items for identical
state, results and continuation.

## Refinement, strict validation and rollback

`state_digest_reference::assert_machine` now checks completed getter identities
against independent uncached hashes, in addition to full E/O input bytes,
root/role manifests and each retained node's canonical bytes/hash. It runs at
successful stage and restoration boundaries. A deliberate stale-pair regression
confirms recomputation detects an unrefreshed semantic mutation. Private mutation
fixtures explicitly rebuild the pair, preserving their real corruption and
storage-order sensitivity. Malformed watermark/baseline/required-root tests still
forge consistent digest claims and require semantic validation to reject them.

The finite exercised domain includes the existing nine mixed graphs with all
temporal families, qualified state, three stateful standard modules, cancellation,
phase and six updates at times 1/2/2/5/7/11. Twelve counted histories of ten updates
compare ordinary, forecast and copy-preparation results/state and retained causal
content. Existing patch suites cover preserve/reset/remove, before/exact-deadline
ordering, source/target attribution and two-patch direct/restored continuation.
The role-sensitive histories with equal unlabeled root unions and coalesced source
allocations retain their required distinct or equal canonical identities.

The new digest-preparation fault occurs on the completed private semantic/root
version before pair construction/publication. The existing complete-observation
matrix now injects it alongside extraction, provenance, result and projection
faults: both lifecycle phases, ordinary/patched transactions, apply/forecast and
copy preparation, including earlier candidate deadlines. Rejection preserves
the warm source pair, all semantic fields/cursors, exact snapshot, retained views
and live-node counts. Existing late bound output rejection still discards both
the candidate and target maps after earlier due work.

Logical append accounting is unchanged. The quiet wire's 0/1/2 limits, omitted
versus explicit-zero pulse batches, all five budget categories, and the two-node
periodic fixture's `3 * 8 + 9 = 33` appends at 32/33/34 limits remain exercised.
Cache reuse, content coalescing, collection and external artifact ownership do
not discount those appends or alter policy identity/failure evidence.

Reference independence is limited and explicit: the detached Vec/map projection
independently enumerates roots/roles and computes content bytes/hashes without
production node or machine caches. It shares semantic enums and CBOR primitives.
Snapshot graph indexing/digests are independent; snapshot field/envelope writers
are shared. Copy preparation independently varies collection ownership but
shares evaluation/publication. Unchanged fixed vectors, hostile re-signed
artifacts, node-law/field checks and direct/restored/replayed future continuation
supplement these limits. There is no second evaluator.

## Validation and remaining costs

All 18 focused digest tests passed, including the new cost and stale-pair cases;
the final additional restoration preparation-count assertions passed separately.
`make check-dev` and `make check-final` passed. The final gate includes the full
configured suites, compilation, formatting, static guardrails, clippy with warnings
denied, Python tooling tests, doctests, documentation and dependency policy.
No golden was regenerated, test skipped or gate weakened. `git diff --check`
also passed.

Commands and logs:

```sh
UV_CACHE_DIR=/private/tmp/mossignal-contract-uv make check-dev
# /private/tmp/ms-0ap4-check-dev.log
UV_CACHE_DIR=/private/tmp/mossignal-contract-uv make check-final \
  QUIET_CHECK=/private/tmp/mossignal-ms-0ap-1-verification/quiet-check
# /private/tmp/ms-0ap4-check-final.log
```

The existing accepted wrapper invokes every normal check and relocates only the
cargo-deny advisory database to writable temporary storage. Its temporary deny
configuration equals repository `deny.toml` after removing that single db-path
entry: all advisories, bans and sources checks remain enabled, with no ignored
advisories or offline override. The locked uv environment is used throughout.

Full raw O and snapshot encoding still visits, sorts/coalesces and writes required
ancestry; successful state changes eagerly pay the full O digest preparation.
Raw E still encodes/sorts represented semantic state and resolves current facts.
One initial staging clone, current required-view metadata copying, evaluator
settlement, new provenance, SOURCE checkpoint wrapping and owned binding/output
projection remain. Required ancestry and deliberately held artifacts may grow.
No throughput measurement, constant-cost full emission, history pruning,
checkpoint feature or optional cache API is claimed.

## Independent acceptance

The coordinator reviewed the complete implementation and test diff, cache
preparation and mutation boundaries, direct frozen-cause lookup, strict restore
ordering, independent stale-pair detection and fault coverage. No corrections
were required. Independent make check-dev and make check-final passed, as did
git diff --check. The advisory-cache wrapper was checked to preserve every
policy setting except its writable db-path. Logs are
/private/tmp/ms04-coordinator-dev.log and /private/tmp/ms04-coordinator-final.log.

Together with the three previously accepted slices, this completes the bounded
ms-0ap library delivery. Cross-repository adoption remains separate. The costs
and reference-independence limits above remain explicit acceptance qualifications.
