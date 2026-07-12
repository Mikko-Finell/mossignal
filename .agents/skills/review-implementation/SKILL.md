---
name: review-implementation
description: "Independently audit and directly correct a complete uncommitted mossignal implementation in Pending Approval. Use when assigned a fresh implementation-review pass: materially corrected work remains Pending Approval for another reviewer, while a clean pass runs final gates, closes and flushes the bead, and creates the accepted implementation commit."
---

# Review Implementation

Perform the most thorough complete review you can and directly correct every in-scope issue determined by the approved contract and authoritative specifications. Independence comes from a fresh review of the resulting artifact, not from sending comments back to the implementer.

If you materially change the implementation, you become a partial author of the current implementation version and cannot independently approve it in the same pass. Leave the corrected implementation `Pending Approval` for a fresh reviewer. A fresh clean pass may complete final acceptance directly; there is no mandatory separate `final-review` stage.

## 1. Establish the complete review object

Run:

```bash
git branch --show-current
git rev-parse HEAD
git status --short
git diff --stat HEAD
git diff HEAD
br show <id> --json
br dep list <id> --direction both --json
```

If the current branch is not `main`, stop without modifying the bead or repository.

The ordinary review target is:

```text
branch: main
HEAD: last accepted repository state
bead status: in_progress
lifecycle label: workflow:implementation-review
implementation approval: Pending Approval
working tree: complete uncommitted proposal for this bead
```

Inspect the complete approved bead, dependencies, concern flags, implementation evidence, all staged changes, all unstaged changes, relevant untracked files, surrounding unchanged code, relevant call sites and module boundaries, tests, test oracles, fixtures, and verification infrastructure.

The review object is:

```text
authoritative specifications
        ↕
approved locked bead contract
        ↕
complete working tree, tests, and evidence relative to HEAD
```

Do not trust the implementer's report or prior command results as proof. Stop on unexplained, unrelated, or unsafe-to-distinguish changes. Never clean, overwrite, stage, or commit unknown work.

## 2. Perform complete adversarial review passes

### Contract coverage

For every obligation and forbidden outcome, locate real implementation and evidence. Inspect every relevant code path, not only the obvious diff hunk. Verify non-goals did not enter accidentally and no required enforcement is missing.

### Specification and architecture

Determine authority and precedence from `docs/specs/` and `AGENTS.md`. Read every cited section with adequate context and search adjacent specifications for omitted invariants. Inspect ownership, enforcement layer, module responsibility, visibility, Rust types, conversions, trait boundaries, public and internal API contracts, persistence, canonical encoding, restoration, replay, reconfiguration, migration, and compatibility where applicable. Detect unapproved architecture.

### Failure and state transitions

Inspect applicable initialization, ordinary absence, validation, rejection, failure atomicity, cleanup, state transitions, ordering, determinism, exact boundaries, diagnostics, inspection, and resource exhaustion. Ensure errors carry relevant domain context and production code contains no new `unwrap` or `expect`.

### Test sensitivity

Confirm tests would fail under plausible violations. Prefer behavior and invariant assertions over implementation choreography. Review test assertions, fixtures, and changed or reused oracles as critically as production code.

Use the confidence-appropriate layer:

- unit tests for deterministic core branching;
- integration tests for real filesystem, process, serialization, or other boundaries;
- property, differential, or bounded exhaustive checks for semantic state spaces;
- fuzzing for untrusted inputs;
- compatibility vectors for persisted promises;
- fault injection for hard-to-trigger failure atomicity.

### Evidence accuracy

Verify every recorded implementation location, test, command, and compliance claim exists and says no more than the artifact proves. Changing evidence is material when it changes what compliance is claimed.

## 3. Correct directly and finish the pass

Fix every unambiguous in-scope defect the approved contract and authoritative material determine. Strengthen tests, correct evidence, remove forbidden fallbacks or bypasses, fix panic paths, and improve enforcement directly.

Do not create a request-changes relay and do not knowingly leave correctable work for another reviewer merely because a fresh approval pass may be required.

After each correction, exercise the affected code paths and rerun focused checks. Use `make check-dev` when useful, but do not treat it as the final acceptance gate. Re-review the complete resulting implementation when corrections are finished.

This validation stabilizes and completes the current work. It is not independent approval of material edits made by this reviewer.

## 4. Classify implementation changes by materiality

A review edit is **material** when it changes behavior, enforcement, verification strength, architectural meaning, or the truth of the compliance claim. Material changes include:

- changing production logic or runtime behavior;
- changing validation, rejection, failure behavior, initialization, state transitions, ordering, or determinism;
- changing persistence, canonical encoding, restoration, replay, reconfiguration, migration, or diagnostic semantics;
- changing public or internal API contracts, type distinctions, visibility boundaries, ownership, or module responsibility;
- adding missing enforcement or removing a forbidden fallback or bypass;
- fixing a panic path;
- changing test assertions, fixtures that affect behavior, or a test oracle;
- adding missing regression, property, differential, exhaustive, fuzz, compatibility, or fault-injection verification;
- changing implementation evidence in a way that changes the compliance claim;
- changing mandatory verification commands or their meaning.

A change is **nonmaterial** only when it cannot affect behavior, architecture, verification strength, or the compliance claim. Examples include formatting, import ordering, spelling, harmless comment cleanup, equivalent mechanical cleanup, correction of an obvious evidence typo without changing the claim, removal of obsolete review-state text, and normalization that cannot affect behavior or tests.

When uncertain, treat the change as material.

## 5. Keep the implementation record clean

Do not retain a findings ledger, resolved defects, correction history, reviewer chronology, or advisories in the accepted bead. Resolved issues disappear into corrected code, strengthened tests, accurate evidence, and Git history.

The next reviewer must independently examine the artifact rather than being primed by resolved-review narration. Record only an exact active unresolved blocker. A worthwhile future concern may become a separate bead.

## 6. Choose one terminal outcome

### Implementation Pending Approval after material revision

Use when the complete review is finished, every resolvable issue is corrected, no unresolved blocker remains, and at least one material implementation correction was made.

Set:

```text
Implementation approval:
Status: Pending Approval
Reason: The current implementation was materially revised during review and requires fresh independent approval.
Known blockers: None
```

Keep status `in_progress` and `workflow:implementation-review`. Use `--no-auto-flush` for any bead update.

Leave `HEAD` unchanged and the complete corrected working tree uncommitted for a fresh implementation reviewer. Do not close the bead, run `br sync --flush-only`, stage, commit, or push.

This is completed review work, not partial work. The current implementation is believed complete; its current material version has not received fresh independent approval.

### Approved after a clean independent pass

Use only when this reviewer is fresh relative to the current material implementation version, reviewed the complete proposal adversarially, made no material correction, found no unresolved blocker, verified every obligation and evidence claim, and passed all mandatory acceptance checks against the exact tree to be committed. Nonmaterial cleanup is permitted.

Set a clean approval record:

```text
Implementation approval:
Status: Approved
Reviewed against accepted HEAD: <full baseline HEAD>

Verification:
- make check-final: passed
- <mandatory bead-specific command>: passed
```

Do not include resolved findings or review chronology. Then complete final acceptance as described below.

### Returned to bead approval

Use when the approved normative contract is materially wrong or incomplete but existing authoritative material appears sufficient for renewed bead review. Follow the restoration and return procedure below. Do not edit the normative contract in this role.

### Blocked

Use `Blocked — Design Required` when a required semantic or architectural answer is genuinely missing or contradictory. Use `Blocked — Rescoping Required` when task boundaries require a material human judgment. Record only the exact active unresolved decision and follow the restoration procedure.

For an infrastructure, environment, or coordination blocker that does not change the approved contract, stop in implementation review, record only that active blocker, and do not flush or commit. Do not misclassify it as a design decision.

## 7. Complete final acceptance after a clean pass

Do not rely on checks reported by the implementer or a prior reviewer. Rerun against the exact working tree being accepted:

- `make check-final` as the canonical full repository gate;
- every mandatory bead-specific verification command;
- relevant regression tests;
- any required compatibility, differential, persistence, migration, or failure-atomicity checks activated by the bead.

A warning designated as failure, unavailable command, timeout, hang, flaky result, or failed mandatory check blocks acceptance.

Before closure, inspect:

```bash
git status --short
git diff --stat HEAD
git diff HEAD
```

Confirm every change belongs to the bead and the exact tree satisfies the approved contract. Ensure the clean approval and final evidence are accurate, then close and synchronize without allowing an intermediate auto-flush:

```bash
br update <id> --remove-label workflow:implementation-review \
  --no-auto-flush --json
br close <id> --reason "Completed: <specific semantic outcome and proof>" \
  --no-auto-flush --json
br sync --flush-only
git status --short
git diff -- .beads/issues.jsonl
make acceptance-record-check BEAD_ID=<id>
```

If closure, synchronization, or acceptance-record validation fails, do not commit. Reconcile the state first.

Stage implementation, tests, required documentation, final evidence, and synchronized bead state using explicit paths. Inspect the complete staged artifact:

```bash
git add <explicit paths>
git diff --cached --stat
git diff --cached
```

Commit once with the bead ID first:

```text
<bead-id>: <imperative semantic outcome>
```

The accepted implementation commit must contain implementation, tests, required documentation, final evidence, explicit approval, closed bead state, and synchronized JSONL. Verify the working tree is clean afterward. Do not push unless separately authorized.

## 8. Return a defective contract to planning safely

Stop implementation review when the approved objective, obligations, forbidden outcomes, scope, non-goals, evidence, mandatory verification, dependencies, concern flags, or completion criteria are materially wrong or incomplete. Do not rewrite the normative contract.

Revert only code, tests, and implementation evidence positively identified as belonging to the current bead implementation, restoring the working tree to accepted `HEAD`. Use targeted edits or targeted restoration of known paths. Never use blanket destructive cleanup or alter unexplained work.

If safe restoration is not possible:

- stop;
- do not flush;
- do not commit;
- invoke the exceptional recovery procedure;
- report the exact repository and bead state.

### Existing authority can determine the correction

After safe restoration, record:

```text
Bead approval:
Status: Pending Approval
Reason: Implementation review exposed a material defect in the approved contract requiring renewed bead review.

Implementation-discovered issue:
- <exact omitted, contradictory, or infeasible contract requirement>
```

Then:

```bash
br update <id> --status draft \
  --remove-label workflow:implementation-review \
  --add-label workflow:pending-approval --no-auto-flush --json
```

Do not use transient `workflow:bead-review`. Leave the planning change unflushed and uncommitted for the bead-review skill.

### Genuine missing design

After safe restoration, record only the exact active unresolved decision and use:

```bash
br update <id> --status blocked \
  --remove-label workflow:implementation-review \
  --add-label workflow:blocked-design --no-auto-flush --json
```

### Material rescoping decision

After safe restoration, record only the exact active boundary decision and use:

```bash
br update <id> --status blocked \
  --remove-label workflow:implementation-review \
  --add-label workflow:blocked-rescoping --no-auto-flush --json
```

A bead returned to planning must not remain mixed with uncommitted production implementation changes.

## Stopping rule

Finish every correction available within the approved contract, rerun affected verification, and re-review the complete result. Then classify the pass:

- material correction made: `Pending Approval`;
- no material correction and no blocker: run final gates and accept;
- defective contract: return to bead approval;
- genuine unresolved design or rescoping issue: block precisely.

Do not stop early because another review may follow. Do not continue after mandatory compliance is established merely for elegance, generality, optional abstraction, or stylistic preference.

## Lifecycle summary

### Material-revision pass

```text
Starting state:
- branch: main
- HEAD: accepted planning baseline
- bead: workflow:implementation-review
- implementation approval: Pending Approval
- working tree: complete uncommitted proposal

Review:
- complete adversarial audit
- direct material corrections
- focused verification

Ending state:
- HEAD: unchanged
- working tree: complete corrected implementation
- implementation approval: Pending Approval
- bead remains workflow:implementation-review
- no flush
- no commit
```

### Clean approving pass

```text
Starting state:
- complete implementation Pending Approval
- working tree contains the current material implementation version

Review:
- complete adversarial audit
- no material correction
- full mandatory acceptance gates

Acceptance:
- mark implementation Approved
- close bead
- br sync --flush-only
- stage and inspect complete accepted diff
- commit with bead ID
- leave working tree clean
```

### Bad-contract result

```text
Review exposes defective approved contract:
- stop
- do not rewrite contract in this role
- safely remove current implementation proposal
- record exact contract issue
- return bead to Pending Approval or precise blocked state
- no flush
- no commit
```

Consult `docs/specs/bead_review_implementation_commit_process.md` only for an exceptional condition this skill does not resolve.
