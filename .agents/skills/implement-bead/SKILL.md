---
name: implement-bead
description: Implement a committed Ready mossignal bead from a clean accepted HEAD as the strongest complete uncommitted working-tree proposal, then leave implementation approval Pending Approval. Use when the user assigns implementation of a specific approved bead and expects code, tests, compact evidence, focused verification, and handoff for fresh independent implementation review.
---

# Implement Bead

Implement the approved contract completely. Do not intentionally leave correctable work for reviewers. The completed result remains uncommitted and `Pending Approval` because an implementer cannot independently approve its own working-tree implementation.

The normal flow is:

```text
committed Ready bead
→ clean accepted HEAD
→ complete uncommitted implementation
→ implementation approval: Pending Approval
→ independent implementation review
```

This file is the ordinary procedure. Consult the exceptional interrupted-work or recovery procedure when uncommitted work already exists or safe restoration becomes uncertain.

## 1. Require the ordinary clean starting state

Run:

```bash
git branch --show-current
git rev-parse HEAD
git status --porcelain
br show <id> --json
br dep list <id> --direction both --json
br coordination status --json
```

If the current branch is not `main`, stop without modifying the bead or repository.

Proceed only when:

- the bead is semantically `Ready`, represented by status `open` and the sole lifecycle label `workflow:ready`;
- its approval section explicitly records fresh independent readiness and the reviewed repository baseline;
- the synchronized `Ready` bead record is committed in `HEAD`;
- all blocking dependencies are closed;
- `git status --porcelain` is empty;
- no active coordination or reservation conflict exists;
- `HEAD` is the accepted implementation baseline.

The ordinary invariant is:

```text
HEAD = last accepted repository state, including the committed Ready bead
working tree relative to HEAD = exactly this bead's proposed implementation
```

Proceed only from a clean working tree. If uncommitted work already exists, do not combine it with a new ordinary implementation pass. Use the interrupted-work recovery procedure or stop for coordination.

Never discard, overwrite, stage, or commit unexplained work.

Begin implementation without flushing:

```bash
br update <id> --status in_progress \
  --remove-label workflow:ready --add-label workflow:implementing \
  --no-auto-flush --json
```

Keep the starting `HEAD` and clean status as the boundary for every later diff and restoration decision.

## 2. Treat the approved contract as locked

Read the complete approved bead. The locked normative contract includes:

```text
objective
obligations
forbidden outcomes
scope and non-goals
required evidence
mandatory verification
concern flags
dependencies
completion criteria
```

The implementer may update only implementation-facing information such as implementation evidence, verification results, implementation approval state, and implementation-discovered blockers. Do not silently revise the normative contract.

Determine authority and precedence from `docs/specs/` and `AGENTS.md`. Read every cited specification section with adequate surrounding context. Inspect relevant current modules, types, tests, call sites, public exports, ownership boundaries, and execution flows. Use `docs/specs/testing_and_verification_policy.md` selectively for the affected verification classes.

Before editing, map every obligation to:

- the correct enforcement layer;
- likely implementation location;
- test type and location;
- focused and bead-specific verification commands;
- adjacent invariants that must remain true.

## 3. Produce the strongest complete implementation

- Implement every obligation, negative constraint, boundary case, and specified failure behavior.
- Prefer modules and concrete domain types. Keep visibility as narrow as possible.
- Add or change public API only when the approved contract requires it; document it and add a runnable example when useful.
- Do not invent semantic identities, fallback variants, generic frameworks, public abstractions, persistence rules, migration behavior, or compatibility policy absent from the contract.
- Surface real failures with `Result`; do not add production `unwrap` or `expect`.
- Choose unit, integration, property, differential, exhaustive, fuzz, fault-injection, or compatibility testing according to the confidence required.

For a reproducible defect that can be expressed through the project's test infrastructure, add the regression test first, run it, and observe the expected failure before fixing the implementation. When the defect concerns compilation, documentation generation, fuzzing, static validation, infrastructure, or another boundary not suited to an ordinary regression test, add the strongest appropriate reproducible evidence instead.

Run the narrowest exact checks repeatedly. Use quiet commands where possible. Run `make check-dev` once the implementation is coherent, plus every bead-specific check required during implementation.

Do not weaken a test, oracle, warning policy, architecture check, or required evidence merely to pass.

## 4. Maintain compact implementation evidence

Update `notes` rather than duplicating the contract:

```text
Implementation evidence:
- O1 [complete]: <implementation location>; <test location>
- O2 [complete]: <implementation location>; <test or inspection evidence>

Verification:
- <focused command>: passed
- make check-dev: passed
- <bead-specific command>: passed

Implementation approval:
Status: Pending Approval
Reason: Awaiting initial independent review of the current working-tree implementation.
```

Do not create a findings field. The implementer is not an implementation reviewer. Do not paste long logs or enumerate every changed file.

## 5. Handle a material contract defect without self-repair

Stop implementation immediately when work proves that the approved objective, obligations, forbidden outcomes, scope, non-goals, evidence, mandatory verification, concern flags, dependencies, or completion criteria are materially wrong or incomplete.

Do not revise the normative contract yourself.

### Existing authority can determine the correction

When authoritative material appears sufficient for renewed bead review:

1. record the exact implementation-discovered contract issue;
2. restore the accepted planning baseline safely as described below;
3. return the bead to semantic `Pending Approval`, not transient `workflow:bead-review`;
4. leave the planning state unflushed and uncommitted for a fresh bead-review pass.

Use a clean approval record broadly equivalent to:

```text
Bead approval:
Status: Pending Approval
Reason: Implementation exposed a material defect in the approved contract requiring renewed bead review.

Implementation-discovered issue:
- <exact omitted, contradictory, or infeasible contract requirement>
```

Transition with non-flushing mutation:

```bash
br update <id> --status draft \
  --remove-label workflow:implementing \
  --add-label workflow:pending-approval --no-auto-flush --json
```

### Genuine design or architecture gap

When the required answer is absent, contradictory, semantic, or architectural, restore the accepted baseline and record only the exact active unresolved question. Then:

```bash
br update <id> --status blocked \
  --remove-label workflow:implementing \
  --add-label workflow:blocked-design --no-auto-flush --json
```

When task boundaries or priorities require a material human judgment, restore the accepted baseline, record only the exact boundary decision, and use:

```bash
br update <id> --status blocked \
  --remove-label workflow:implementing \
  --add-label workflow:blocked-rescoping --no-auto-flush --json
```

Do not invent the answer.

## 6. Restore safely after contract failure

Revert only code, tests, and implementation evidence positively identified as belonging to the failed implementation attempt, restoring the repository to the accepted `HEAD` before handing the bead back to planning.

Use targeted edits or targeted restoration of known paths. Do not use blanket destructive commands and do not alter unexplained work. Reinspect `git status --porcelain` and the diff from the captured starting `HEAD` before changing the bead's durable state.

If you cannot confidently distinguish your own changes or safely restore the accepted baseline:

- stop;
- do not flush;
- do not commit;
- do not claim a clean planning handoff;
- invoke the exceptional interrupted-work or recovery procedure;
- report the exact repository and bead state.

The ordinary successful contract-failure handoff is:

```text
HEAD = accepted Ready planning state
working tree = bead planning change only
bead = Pending Approval or the precise Blocked state
no flush
no commit
```

Production code from the failed attempt must not remain mixed into a bead returned for approval review.

## 7. Hand off a successful implementation uncommitted

Inspect the complete difference from `HEAD`, including staged, unstaged, and relevant untracked files. Confirm every change belongs to the bead and the proposal completely satisfies the locked contract.

Ensure the bead notes end with:

```text
Implementation approval:
Status: Pending Approval
Reason: Awaiting initial independent review of the current working-tree implementation.
```

Then:

```bash
br update <id> --status in_progress \
  --remove-label workflow:implementing \
  --add-label workflow:implementation-review --no-auto-flush --json
```

The implementation-review label identifies the review queue; the approval text carries the semantic `Pending Approval` state.

Leave `HEAD` unchanged and the working tree as the complete proposed implementation. Do not stage merely for review, close the bead, run `br sync --flush-only`, commit, or push. Use `--no-auto-flush` for every bead mutation.

Do not run `br sync --flush-only` in the implementation role.

Report the completed implementation, tests, focused and bead-specific verification, compact evidence, and exact handoff state. Do not specify how implementation reviewers iterate or approve; that belongs to the `review-implementation` skill.

## State summary

### Ordinary successful implementation

```text
Starting state:
- branch: main
- HEAD: accepted Ready planning commit
- working tree: clean
- bead: Ready

Implementation:
- complete code and tests
- focused verification
- compact implementation evidence

Ending state:
- HEAD: unchanged
- working tree: complete proposed implementation
- bead: Implementation approval Pending Approval
- lifecycle label: workflow:implementation-review
- no flush
- no commit
```

### Contract failure during implementation

```text
Starting state:
- HEAD: accepted Ready planning commit
- bead: Ready

Implementation exposes a material contract defect:
- stop implementation
- do not edit the normative contract
- safely remove only this failed implementation attempt
- record the exact implementation-discovered issue
- return the bead to Pending Approval or the precise Blocked state
- no flush
- no commit
```

Consult `docs/specs/bead_review_implementation_commit_process.md` only for interrupted-work recovery or another exceptional case this skill does not cover.
