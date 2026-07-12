---
name: review-bead
description: Independently audit and directly correct a complete mossignal bead in Pending Approval, then commit a clean planning result as Ready, Pending Approval after material revision, or Blocked. Use when the user assigns bead approval review or asks whether the current implementation contract is independently ready for implementation.
---

# Review Bead

Act as a fresh adversarial auditing editor. Complete the most thorough review you can, directly correct every issue authoritative material lets you resolve, reread the resulting bead, and make one terminal decision for this review pass.

Independence applies to the current material version. If you materially change the implementation contract, you become a partial author of that version and cannot mark it `Ready` in the same pass. Your completed result is `Pending Approval` for a fresh reviewer.

This file is the ordinary procedure. Consult the repository-wide process specification only for an exceptional case not handled here.

## 1. Establish the review target and clean baseline

Run:

```bash
git branch --show-current
git rev-parse HEAD
git status --short
br show <id> --json
br dep list <id> --direction both --json
br search "<task terms>" --json
```

Ordinary input is semantic `Pending Approval`, represented by Beads status `draft` plus `workflow:pending-approval`. Its approval section must say the current contract awaits fresh independent approval and has no known blocker.

A blocked bead may be reviewed only after its design or rescoping issue has been resolved and it has explicitly returned to `Pending Approval`. Do not silently process a `Ready`, implementing, or closed bead through ordinary approval review; require an explicit reason to reopen its approval state.

All work occurs on `main`. If the current branch is not `main`, stop without modifying the bead or repository.

Existing uncommitted changes belonging solely to the bead under review are expected and form part of the planning result. Stop only when the working tree contains unexplained changes or changes belonging to another task.

Begin the pass without flushing:

```bash
br update <id> --status in_progress \
  --remove-label workflow:pending-approval \
  --add-label workflow:bead-review --no-auto-flush --json
```

Keep the full starting `HEAD` as this review pass baseline.

## 2. Verify authority and repository reality independently

Determine authority and precedence from `docs/specs/` and `AGENTS.md`. Do not rely on the author’s source selection.

- Read every cited specification section with adequate surrounding context.
- Verify paths, stable section headings, governing subjects, and the recorded specification baseline.
- Search adjacent specifications for omitted requirements and negative constraints.
- Resolve apparent conflicts by documented precedence only; otherwise block for design.

Inspect relevant current code, tests, call sites, module ownership, dependencies, and overlapping beads. Trace important execution paths through imported and importing modules sufficiently to determine whether the contract matches repository reality, whether work already exists, and whether its boundary and evidence are feasible.

Current code proves what exists, not what should exist when it conflicts with specifications. Do not implement or modify production code in this role.

## 3. Perform a complete adversarial review

Review the whole contract, not only suspected gaps.

### Source fidelity

- Does every obligation have authoritative support?
- Are all material requirements within scope represented?
- Did summarization alter modality, identity, ordering, timing, compatibility, migration, persistence, or failure semantics?
- Does terminology match specifications and current public types?

### Completeness and negative space

Check applicable initialization, success, rejection, failure atomicity, cleanup, persistence, restoration, reconfiguration, inspection, diagnostics, public API, compatibility, and verification behavior. Add forbidden outcomes where a plausible shortcut would violate a governing invariant.

### Boundary and feasibility

- Is the objective one coherent semantic slice?
- Can implementation proceed without inventing architecture or semantics?
- Do scope and non-goals agree?
- Are dependencies real, complete, and correctly ordered?
- Does the expected change surface match current ownership without pretending to be a lock?
- Are the requested outcomes possible together?

### Evidence quality

- Is every obligation objectively checkable?
- Would each proposed test fail under a plausible violation?
- Is the test layer appropriate?
- Are mandatory additional commands finite and reproducible?
- Do completion criteria describe semantic completion rather than file churn?

Correct every issue you can resolve from authoritative material. Do not create a request-changes relay and do not knowingly leave correctable work for a later reviewer merely because another pass may occur.

Use `--no-auto-flush` for every intermediate bead mutation. Synchronize only after the pass reaches its clean terminal state.

## 4. Classify every edit as material or nonmaterial

A change is **material** when it changes the implementation contract, including any change to:

- objective;
- normative obligations;
- forbidden outcomes;
- scope or non-goals;
- authoritative interpretation;
- expected evidence or mandatory verification;
- dependencies;
- concern or risk flags;
- completion criteria;
- public, persistence, migration, failure, identity, or compatibility requirements.

A change is **nonmaterial** when meaning is unchanged, such as spelling, grammar, formatting, harmless wording cleanup, an obvious path or heading typo, layout normalization, removal of obsolete review-history text, or an unambiguous reference correction that does not change the cited requirement.

When uncertain, treat the change as material.

Reread the complete corrected bead. This self-check is required for correctness, but it is not independent approval of material edits you made.

## 5. Produce one clean terminal state

Remove obsolete approval text, prior blockers, resolved findings, review chronology, and advisory commentary. Git and bead history preserve earlier states. The bead should expose only the current contract and current terminal state.

Do not keep a finding ledger in ordinary bead review. Resolved issues disappear into the corrected contract. A blocked bead lists only active unresolved blockers. Nonblocking future ideas belong in separate beads only when worthwhile.

### Ready: no material correction and no blocker

Use `Ready` only when this fresh pass audited the complete current material version and did not need a material correction. Nonmaterial cleanup is allowed.

Update the bead’s specification baseline to the full review-pass baseline `HEAD` and set:

```text
Bead approval:
Status: Ready
Reviewed against repository commit: <full review-pass baseline HEAD>
```

Then:

```bash
br update <id> --status open \
  --remove-label workflow:bead-review --add-label workflow:ready \
  --no-auto-flush --json
```

### Pending Approval: material correction and no blocker

Use when you completed the review and corrected the contract materially. Do not mark the same version `Ready` in this pass.

Update its specification baseline to the full review-pass baseline `HEAD` and set:

```text
Bead approval:
Status: Pending Approval
Reason: The current implementation contract was materially revised during review and requires fresh independent approval.
Known blockers: None
```

Then:

```bash
br update <id> --status draft \
  --remove-label workflow:bead-review \
  --add-label workflow:pending-approval --no-auto-flush --json
```

`Pending Approval` means believed complete with no known blocker. It does not mean changes requested, partial review, or known deficiency.

### Blocked — Design Required

Use only when existing authoritative material cannot determine a required semantic or architectural answer. Set:

```text
Bead approval:
Status: Blocked — Design Required

Blocking issues:
- <exact unresolved decision>
```

Record the full review-pass baseline so later resolution work can identify specification changes.

Then:

```bash
br update <id> --status blocked \
  --remove-label workflow:bead-review \
  --add-label workflow:blocked-design --no-auto-flush --json
```

### Blocked — Rescoping Required

Use only when task boundaries or priorities require a material human judgment. Record the full review-pass baseline and only the exact active boundary decision, then:

```bash
br update <id> --status blocked \
  --remove-label workflow:bead-review \
  --add-label workflow:blocked-rescoping --no-auto-flush --json
```

## 6. Flush and commit the completed planning pass

Every terminal review result is a completed planning transition, including `Pending Approval` after material revision and either blocked state.

Run:

```bash
br sync --flush-only
git status --short
git diff -- .beads/issues.jsonl
```

Verify the diff contains only the intended bead planning changes and the terminal bead record is clean. Stage the synchronized bead JSONL explicitly, inspect the staged diff, and commit:

```bash
git add .beads/issues.jsonl
git diff --cached --stat
git diff --cached
git commit -m "<bead-id>: approve <task summary>"
```

Use the outcome-appropriate subject:

```text
<bead-id>: approve <task summary>
<bead-id>: revise <task summary> bead
<bead-id>: block <task summary> on <specific issue>
```

The bead ID must be the first token. Do not include unrelated changes. Verify the working tree is clean after the planning commit. Do not push unless separately authorized.

## Stopping rule

Finish all corrections available from authoritative material, reread the result, then classify it by materiality and blockers. Do not stop early because another pass is expected. Do not continue indefinitely for elegance, generality, or stylistic refinement.

Consult `docs/specs/bead_review_implementation_commit_process.md` only for an unusual lifecycle or coordination case this skill does not resolve.
