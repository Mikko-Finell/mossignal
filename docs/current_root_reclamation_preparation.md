# Current-root reclamation preparation

This is non-authoritative preparation evidence for `ms-0ap.2`, the next child of
`ms-0ap`.
Baseline: accepted `main` commit
`2100fc027ead42e5526c81eda611443af2de8f83`. The specification deltas and six contracts were independently reviewed on
2026-10-10. The execution-role and snapshot-schema decisions below are accepted;
implementation remains a separately reviewed delivery.

## Accepted scope and review decisions

The independently accepted parent already chooses current SOURCE machine roots
after strictly earlier deadlines and before patch removal/reset, target input or
target settlement. Deadlines at the effective time belong to target execution.
External artifacts, including earlier-deadline event roots needed only by the
outer result, never enlarge the TopologyChange supporter set. Required ancestry
stays complete; no public pruning/history-disable feature or total-memory bound
is introduced. The immutable DAG, creating-topology canonical cache, isolated
membership and iterative release from `ms-0ap.1` are reused.

The parent also approves provenance semantics/domain 3 and execution/observable
domain/projection 3, with otherwise unchanged components. Independent review accepted two additional
decisions:

1. Snapshot artifact schema 3, adding only missing current-role associations.
2. Execution identity including non-derivable current-role CauseDigest bindings,
   with reconciliation of the unconditional observable-only verification witness.

The normative specification text records these accepted deltas. Their justification
is the continuation evidence below, not merely the domain-version change. No
release freeze exists.

## Executable counterexample on the accepted version-2 library

Two PulseResettableToggle modules share pulse input P, with separate reset inputs
R20 and R21, both initially Low. A separate PulseDelay retains one event from Q at
time 0 until deadline 100. Positive even pulse batches are intentional: they
update latest accepted toggle evidence without changing stored parity.

| Time | Both histories' P | History A reset | History B reset |
| --- | --- | --- | --- |
| 0 | 0; Q = 1 | neither | neither |
| 1 | 2 | neither | neither |
| 2 | 2 | M20 | M21 |
| 3 | 0 | both | both |
| 4 | 0 | neither | neither |

At time 4, all stored levels and baselines are Low and current reset evidence is
the same. M20's latest accepted toggle is P@1 in A and P@2 in B; M21 has the
opposite association. The executable independently follows every cause-exposing
current path used by this fixture and recursively normalizes facts, stamps,
stable subjects, labeled/grouped contributions and complete ancestry, ignoring
private CauseRef identity. Its **19 unlabeled root contents and their complete
ancestry are identical**, while qualified role associations differ. Execution
digests, observable digests and full version-2 snapshot bytes are identical.
Even including the correct unordered root union would not recover the ownership
association. Latest-by-time reconstruction fails for M20 in A and M21 in B.

Both machines next accept P=2 while resetting M21 at time 5, then a quiet reaction
at time 6. M20's latest toggle becomes P@5; M21 retains its different prior toggle.
The current unlabeled source root sets now differ. At time 7 the same preserving
patch adds a level output, retaining the separate delayed event. Its public key,
owner, scheduling revision, origin, deadline and count agree, but its migrated
cause and execution digest differ. The same expected-execution digest at time 8
accepts A and rejects B as `runtime.stale_execution_state`; B's complete snapshot
is unchanged.

These are measured **version-2** results. Its patch still selects full retained
history, so that implementation has an additional historical supporter difference.
It does not experimentally prove the accepted narrowed version-3 design. The
version-3 deduction is separate: the newly different current source roots feed
TopologyChange; a preserved pending event's migration supports that change;
the pending cause content identity is already execution state, so future execution
freshness can distinguish the successors. Implementation must execute this
deduction against the independent current-root reference, including restore.

An identical unlabeled union also cannot infer the lost associations after a
source checkpoint: deterministic source wrapping of equal facts adds no missing
module-to-cause information. Test this with two actual preserving migrations,
snapshot/restore between them, and identical subsequent selective input.

The probe was compiled against the baseline library and all assertions passed.
Reproduce from the source below (save it as `/private/tmp/ms-root-role-probe.rs`):

```sh
cargo build -p mossignal --lib --locked
rustc --edition=2024 /private/tmp/ms-root-role-probe.rs --extern mossignal=target/debug/deps/libmossignal-3044d3946fc14569.rlib -L dependency=target/debug/deps -o /private/tmp/ms-root-role-probe
/private/tmp/ms-root-role-probe
```

The rlib name is from this recorded build; use the current build's emitted rlib if
the toolchain changes. The included semantic helper is the accepted repository
test reference, not a production root collector. Scratch `unwrap` calls belong
only to this assertion fixture. No repository Rust source or test was edited.

## Ownership, execution consequences and minimal persisted information

`M` is the current machine retention manifest; `S` is required snapshot state.
All M roots can affect future migration and, through migrated pending/episode
cause identities, later digest guards. Their causal contents do not generally
affect numeric evaluation, and root count is not a provenance-growth charge.
No table row proposes putting full record payloads or derivation graphs in E.

| Current role | Evaluation / migration / admission / later guard consequence | Existing version-2 persistence | Proposed minimal E / S delta |
| --- | --- | --- | --- |
| External level origin | value evaluates; origin feeds M; direct growth charge unchanged; later guard through migration | level valuation plus input roots identified by input subject | E needs origin identity; S reuses uniquely keyed roots |
| Current operation/port and explicit last reaction | current value evaluates; support/last reaction feeds M; no persistent pulse valuation; later guard through migration | no exact role bindings; restore installs generic operation causes | E needs independent stable operation/role identities; S adds missing associations; topology aliases derive |
| Stored-state establishment/latest transition | value evaluates; causes feed M and future establishment chains; admission charges new appends; later guard through migration | state value plus subject-identified state roots | E needs cause identities absent from value entry; S reuses unique state-family roles |
| Edge observation | remembered value determines next pulse; cause feeds M/next derivation; later guard | remembered state and subject-identified primary root | E needs primary cause identity; S reuses existing root |
| Transport/inertial remembered facts and cancellation | values/candidate determine scheduling; causes feed M and cancellation ancestry; later guard | state/pending entries plus typed primary/cancellation roots | E needs identities not already in pending; S reuses uniquely discriminated roots |
| Periodic anchor/phase/cancellation | phase determines deadlines; causes feed M and future boundary ancestry; later guard | phase state/pending entries plus typed primary/cancellation roots | E needs identities not already in pending; S reuses existing roles |
| Output establishment/latest baseline | comparison determines change publication; cause feeds M; later guard | explicit output baseline keyed cause in S/O | E needs baseline identity not already derivable; S reuses keyed entry |
| Pending scheduling/obligation | payload determines firing; cause feeds M and migration; identity already affects E/freshness | explicit event-keyed CauseDigest | E/S already identify it; no duplicated binding |
| Active diagnostic beginning/current evidence | evidence controls episode publication; feeds M and migration; identity already affects E/freshness | explicit episode/evidence root identities | reuse existing identities; add only if a current role is not uniquely represented |
| Qualified module latest reset/toggle/capture | reset/capture/toggle values evaluate in their reaction; latest association feeds future M; guard via migration | latest-role map absent; cause subject can be a shared external input | E needs qualified role identities; S adds only missing associations; derivable capture/state aliases reuse existing facts |
| External result/inspection/explanation/forecast; transient outer-result event roots | own sufficient closure for every exposed cause; no effect on machine evaluation, M, admission, digest guards or S | separate owned artifact | no machine E/S additions; independent lifetimes and membership |

Three information layers remain distinct: role identity identifies the owner and
meaning; cause-content identity commits the current binding and can matter to
future execution freshness; full payload/ancestry supplies required explanation,
canonical observable emission and strict restore validation. Sparse private
translation maps or stable handles are implementation freedom; ordinal density,
allocation order and current-topology reinterpretation of old facts are not rules.

The accepted S field `provenance.current_roots` is a sorted unique array of stable
typed subject, semantic role and CauseDigest. It contains only roles not already
uniquely identified by input/state/event/baseline/episode facts. Full qualification
is necessary; dense compiler indices are excluded. Uninitialized roles are empty.
Restore checks exact coverage, permitted subject/role kinds, agreement with other
facts, resolvability, complete closure and canonical ordering. Ambiguous latest
selection or generic snapshot-state causes cannot reproduce future source truth.
Any claimed derivability must hold after multiple migration wrappers, not just
for ordinary original records.

The accepted E addition is the same principle across **all** non-derivable M
bindings, rather than a special module-only patch. Existing event/episode cause
entries are not repeated. Existing topology determines equivalent port aliases.
Output baseline values are reconstructible at a committed ready boundary; their
establishment cause identities need separate consideration. Physical sharing,
dead-node release and held artifacts cannot change any semantic projection.

## Accepted exact version profile

| Component | Snapshot | Replay frame/log, transaction, input snapshot/delta |
| --- | --- | --- |
| Envelope schema | 2 | 2 |
| Canonical encoding | 2 | 2 |
| Digest suite | 2 | 2 |
| Artifact schema | **3** | 2 |
| Core / built-in node / topology patch / diagnostic semantics | 2 / 2 / 2 / 2 | 2 / 2 / 2 / 2 |
| Provenance semantics | 3 | 3 |

Provenance record label/domain version: `mossignal/provenance_record/v3`, 3.
Execution and observable labels/domain versions: their respective `/v3`, 3;
both projection versions are 3. Snapshot, artifact-integrity and replay-log-content
labels/domain versions stay `/v2`, 2. Fingerprint and policy projections remain
unchanged. A snapshot artifact-schema change does not change replay field shapes
or require artifact-schema equality between kinds.

Validate every component and corresponding payload independently, including
nested artifacts and provenance source facts inside checkpoint wrappers. Exact
per-kind mixed vectors are supported; blanket all-2/all-3, relabeled old record
bytes, nested semantic mismatches and unsupported components fail structurally.
Private cached content is not trusted decode evidence. Complete canonical digest
inputs and artifact bytes must agree with an independent root-directed encoder;
golden replacement alone proves nothing. No old decoder, upgrade or shim is added.

## Independent execution/observable verification reconciliation

Independent review reproduced the complete version-2 probe, including identical
snapshot bytes and the later divergent freshness guard. The version-3 deduction
still requires implementation/reference tests; the baseline probe does not prove
new code that has not been implemented.

Review accepted the minimal role-to-CauseDigest additions to execution identity:
these current bindings can determine future migration causes, including pending
causes that already affect execution identity and guards. Full derivation payloads
remain outside execution projection. Snapshot schema 3 supplies associations
that existing persisted roles cannot reconstruct uniquely.

The old digest contract's unconditional observable-only witness overstated its
cited sources. Testing policy section 89 requires optional-history execution
invariance, checkpoint observable sensitivity, snapshot metadata sensitivity and
allocation/presentation invariance; it does not require equal execution identity
for the checkpoint case. Its other cited verification sections likewise do not
impose that unconditional witness. Review corrected this source-support error
and made the current-profile qualification explicit in persistence section 58.

For every supported independently variable required-observation facet, tests
must retain a coherent equal-execution/different-observable pair. No such facet
has been established for this profile after future-determining current cause
identities enter execution state. This is not a waiver of domain separation,
canonical-byte comparisons, checkpoint sensitivity, inclusion/exclusion,
rejection, inspection purity or a future independent facet's separation test.
Inconsistent settled values, optional history and hash collisions cannot supply
a manufactured witness. No testing-policy requirement was weakened.

The runtime-policy budget section's added logical-growth rule preserves its
existing rejection invariant; the prior policy-code source change was already
reconciled. Unchanged adjacent reviewed facets remain reused.

## Verification shape for the bounded implementation

Reuse unchanged reviewed contracts and the accepted independent canonical
reference discipline. Independently enumerate typed current fields into a
detached root/role inventory; do not call the production collector or reuse its
translation logic. Normalize complete causal facts, not opaque IDs. Compare
full canonical inputs and complete artifacts with the independently selected
root-directed reference, including reference limitations already documented in
`docs/shared_causal_storage_verification.md`.

Exercise quiet/stateless histories with dropped results and deterministic live
node/metadata/release counters: unrelated records must not accumulate. Contrast
growing Toggle establishment and periodic phase ancestry, which must remain
complete and may grow. Holding/dropping old results, inspections, explanations,
episodes and forecasts changes only their own owned closure; test machine drop,
cross-view membership isolation, source removal/reset, earlier-deadline result
causes, effective-time target events, active diagnostic beginnings and forecast
rejection/publication. Count actual closure visits/emitted bytes without claiming
bounded canonical output for growing required ancestry.

Verify logical growth below/exactly/above the same independently counted boundary,
including reclamation during the transaction, multiple internal reactions and
equivalent checkpoint replacements. New appends count even if no longer current
at publication; reclaimed nodes never offset growth. Failure injection covers
admission, earlier reactions, migration/root construction, target evaluation,
budget, result/digest/snapshot/episode construction and facade projection where
currently reachable, preserving full predecessor state, owned views, metadata
and semantic public IDs.

Direct, strict-restored and replayed continuations must agree after multiple
preserving patches, role-sensitive updates, deadlines and guards. Retained source
events resolve even when excluded from TopologyChange. Hostile schema/profile,
missing/duplicate/wrong-kind role bindings and bad closure/cycle/subject/stamp/
digest claims must reject without publishing a partial store. Run narrow tests,
then `make check-dev` and `make check-final`; no gate is weakened for this slice.

## Reproducible probe source

<!-- The full assertion fixture is embedded below; it is preparation evidence. -->

```rust
use mossignal::key::*;
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel, PulseCount};
use mossignal::time::Time;
use mossignal::time::NonZeroSpan;
use mossignal::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
#[path = "/Users/mikkofinell/code/mossignal/crates/mossignal/tests/support/causal.rs"]
mod causal;

fn policy() -> RuntimePolicy {
    RuntimePolicy::builder().max_internal_reactions(100)
        .max_evaluated_operations(100_000).max_pending_events(100)
        .max_events_created_per_transaction(10_000)
        .max_required_provenance_growth(100_000).build().unwrap()
}
fn compiled() -> CompiledNetwork<()> {
    let mut b = NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
    let p = b.add_pulse_input(ExternalInputKey::from_u128(10), DiagnosticMeta::default()).unwrap();
    for (module, reset, output) in [(20, 11, 30), (21, 12, 31)] {
        let r = b.add_pulse_input(ExternalInputKey::from_u128(reset), DiagnosticMeta::default()).unwrap();
        let out = b.add_pulse_resettable_toggle(ModuleInstanceKey::from_u128(module), p, r,
            LogicLevel::Low, DiagnosticMeta::default()).unwrap().into_outputs();
        b.add_level_output(ExternalOutputKey::from_u128(output), out, DiagnosticMeta::default()).unwrap();
    }
    let q = b.add_pulse_input(ExternalInputKey::from_u128(13), DiagnosticMeta::default()).unwrap();
    let delayed = b.add_pulse_delay(NodeKey::from_u128(22), q,
        PulseDelayConfig::new(NonZeroSpan::from_ticks(100).unwrap()),
        DiagnosticMeta::default()).unwrap().into_outputs();
    b.add_pulse_output(ExternalOutputKey::from_u128(32), delayed, DiagnosticMeta::default()).unwrap();
    b.finish().require_artifact().unwrap().compile().require_artifact().unwrap()
}
fn apply(c: &CompiledNetwork<()>, m: &mut Machine<()>, at: u64, p: u64, r1: u64, r2: u64) {
    let input = c.input_delta().pulse(ExternalInputKey::from_u128(10), PulseCount::new(p)).unwrap()
        .pulse(ExternalInputKey::from_u128(11), PulseCount::new(r1)).unwrap()
        .pulse(ExternalInputKey::from_u128(12), PulseCount::new(r2)).unwrap().finish().unwrap();
    drop(m.apply(Transaction::advance(Time::from_ticks(at), m.revision(), input)).unwrap());
}
fn history(c: &CompiledNetwork<()>, left: bool) -> Machine<()> {
    let mut m = c.spawn(policy());
    let input = c.input_snapshot().pulse(ExternalInputKey::from_u128(13), PulseCount::new(1)).unwrap().finish().unwrap();
    drop(m.apply(Transaction::initialize(Time::from_ticks(0), m.revision(), input)).unwrap());
    apply(c, &mut m, 1, 2, 0, 0);
    apply(c, &mut m, 2, 2, u64::from(left), u64::from(!left));
    apply(c, &mut m, 3, 0, 1, 1);
    apply(c, &mut m, 4, 0, 0, 0);
    m
}
type Sem = Arc<causal::SemanticCause>;
fn roots(m: &Machine<()>) -> (BTreeSet<Sem>, BTreeMap<u128, Sem>) {
    let mut all = BTreeSet::new();
    let mut roles = BTreeMap::new();
    for module in [20, 21] {
        let view = m.inspect_module(ModuleInstanceKey::from_u128(module)).unwrap();
        let s = view.stateful_standard().unwrap();
        assert_eq!(s.state, LogicLevel::Low);
        let toggle = s.latest_accepted_toggle_cause.unwrap();
        roles.insert(module, causal::semantic(&s.provenance, toggle));
        for cause in [s.latest_reset_cause, s.latest_accepted_toggle_cause, s.latest_capture_cause].into_iter().flatten()
            .chain(s.public_causes.iter().map(|(_, v)| *v))
            .chain(s.internal_causes.iter().map(|(_, v)| *v)) {
            all.insert(causal::semantic(&s.provenance, cause));
        }
        for n in view.nodes() {
            let n = m.inspect_qualified_node(n.node().clone()).unwrap();
            let roots = [Some(n.last_reaction_cause), n.current_support, n.latest_transition].into_iter().flatten()
                .chain(n.inputs.iter().filter_map(|v| v.current_support))
                .chain(n.outputs.iter().filter_map(|v| v.current_support));
            for cause in roots { all.insert(causal::semantic(n.provenance(), cause)); }
        }
    }
    for output in [30, 31] {
        let o = m.inspect_output(ExternalOutputKey::<Level>::from_u128(output)).unwrap();
        for cause in [o.current_support, o.latest_transition].into_iter().flatten() {
            all.insert(causal::semantic(o.provenance(), cause));
        }
    }
    let n = m.inspect_node(NodeKey::from_u128(22)).unwrap();
    for cause in [Some(n.last_reaction_cause), n.current_support, n.latest_transition].into_iter().flatten()
        .chain(n.inputs.iter().filter_map(|v| v.current_support))
        .chain(n.outputs.iter().filter_map(|v| v.current_support)) {
        all.insert(causal::semantic(n.provenance(), cause));
    }
    for p in m.inspect_pending_events().unwrap() {
        all.insert(causal::semantic(p.provenance(), p.cause));
    }
    (all, roles)
}
fn preserve(m: &mut Machine<()>) -> CompiledNetwork<()> {
    let source = m.compiled().graph().external_outputs()[0].source();
    let patch = m.patch().add_external_output(mossignal::authored::ExternalOutputDef::new(
        ExternalOutputKey::<Level>::from_u128(33).into(), source, DiagnosticMeta::default())).unwrap().finish();
    let prepared = m.prepare_patch(patch).require_artifact().unwrap();
    let target = prepared.resulting_compiled().clone();
    let input = target.input_delta().finish().unwrap();
    drop(m.apply(Transaction::advance(Time::from_ticks(7), m.revision(), input)
        .with_patch(prepared, ReconfigurationPolicy::RejectStateLoss).unwrap()).unwrap());
    target
}
fn main() {
    let c = compiled(); let mut a = history(&c, true); let mut b = history(&c, false);
    let (ar, aa) = roots(&a); let (br, ba) = roots(&b);
    assert_eq!(ar, br, "unlabeled current root sets and complete ancestry must coincide");
    assert_ne!(aa, ba, "qualified latest-accepted-toggle role assignments must differ");
    assert_eq!(a.execution_state_digest(), b.execution_state_digest());
    assert_eq!(a.observable_state_digest(), b.observable_state_digest());
    assert_eq!(a.snapshot(), b.snapshot());
    println!("before continuation: {} identical unlabeled roots and ancestry; role assignments differ; v2 snapshots identical", ar.len());
    apply(&c, &mut a, 5, 2, 0, 1); apply(&c, &mut b, 5, 2, 0, 1);
    apply(&c, &mut a, 6, 0, 0, 0); apply(&c, &mut b, 6, 0, 0, 0);
    let (ar, _) = roots(&a); let (br, _) = roots(&b);
    assert_ne!(ar, br, "selective role replacement exposes the lost association");
    println!("after identical selective acceptance and quiet reaction: source root sets differ");
    let before_a = a.inspect_pending_events().unwrap().remove(0);
    let before_b = b.inspect_pending_events().unwrap().remove(0);
    assert_eq!(causal::semantic(before_a.provenance(), before_a.cause),
        causal::semantic(before_b.provenance(), before_b.cause));
    drop((before_a, before_b));
    let ca = preserve(&mut a); let cb = preserve(&mut b);
    let pa = a.inspect_pending_events().unwrap().remove(0);
    let pb = b.inspect_pending_events().unwrap().remove(0);
    assert_eq!(pa.event, pb.event); assert_eq!(pa.owner, pb.owner);
    assert_eq!(pa.origin, pb.origin); assert_eq!(pa.deadline, pb.deadline);
    assert_eq!(pa.revision, pb.revision);
    assert!(matches!(pa.payload, PendingPayload::Pulse(count) if count == PulseCount::new(1)));
    assert!(matches!(pb.payload, PendingPayload::Pulse(count) if count == PulseCount::new(1)));
    assert_ne!(causal::semantic(pa.provenance(), pa.cause), causal::semantic(pb.provenance(), pb.cause));
    assert_ne!(a.execution_state_digest(), b.execution_state_digest());
    println!("after identical preserving patch: pending identity/origin/deadline/count agree; pending causes and execution digests differ");
    let guard = a.execution_state_digest(); let before_b = b.snapshot();
    drop(a.apply(Transaction::advance(Time::from_ticks(8), a.revision(), ca.input_delta().finish().unwrap())
        .expect_execution_state(guard)).unwrap());
    let failed = b.apply(Transaction::advance(Time::from_ticks(8), b.revision(), cb.input_delta().finish().unwrap())
        .expect_execution_state(guard)).err().unwrap();
    assert_eq!(failed.code().as_str(), "runtime.stale_execution_state");
    assert_eq!(before_b, b.snapshot());
    println!("same expected-execution guard: first machine accepts; second rejects runtime.stale_execution_state atomically");
}
```
