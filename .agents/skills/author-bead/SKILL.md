---
name: author-bead
description: Author a complete mossignal implementation bead from authoritative repository specifications and leave it Pending Approval. Use when the user assigns bead authorship, asks to turn an intended outcome into an implementation contract, or needs a bounded bead prepared for fresh independent approval.
---

# Author Bead

Produce the strongest complete implementation contract you can. Leave it `Pending Approval` because its current material version has not received fresh independent approval, not because it is knowingly incomplete or weak.

This file is the ordinary procedure. Do not read the repository-wide process specification unless this procedure encounters an unaddressed exceptional case or apparent conflict.

## 1. Establish the task and source of truth

Run:

```bash
git branch --show-current
git rev-parse HEAD
git status --short
br list --status=open --json
br search "<task terms>" --json
```

Keep the full `git rev-parse HEAD` value as the specification baseline.

Determine authority and precedence from `docs/specs/` and `AGENTS.md`. Use `rg` to find the directly governing requirements, then read their surrounding sections. Follow references into adjacent specifications when the task can affect initialization, failure semantics, persistence, reconfiguration, inspection, diagnostics, public API, or verification. Do not treat other material under `docs/` as normative unless `AGENTS.md` or a specification says it is.

Before drafting, answer:

- What single semantic or architectural outcome is required?
- Which requirements directly govern it?
- Which adjacent invariants could a locally convenient implementation violate?
- What must the implementation never do?
- What evidence could objectively distinguish compliance from noncompliance?
- Does any necessary decision remain absent, contradictory, or genuinely open?

Search temporary Draft, Pending Approval, Ready, implementing, blocked, and closed work. If equivalent work already exists, report or update that record as appropriate instead of creating another bead.

## 2. Inspect the current implementation

Use code to learn what exists; use specifications to determine what should exist.

Explore the files relevant to the task surface. Choose the important code and test files for deep inspection, then trace their functionality and execution flow through the modules they import and the callers that import them. Use `rg`, module declarations, public exports, call sites, and existing tests to determine:

- whether the requested behavior already exists wholly or partly;
- whether named APIs, types, modules, and test facilities actually exist;
- which component owns the current behavior;
- which downstream callers and adjacent invariants the work can affect;
- whether another bead overlaps the real implementation surface;
- which dependencies and implementation boundaries are genuine;
- what existing test infrastructure can supply meaningful evidence.

Do not derive desired semantics from current code when it conflicts with the specifications. Do not perform implementation edits in this role.

## 3. Choose a coherent boundary

Make the bead one reviewable semantic slice. It may span several modules when they jointly deliver one behavior. Split only when a boundary can be implemented and accepted independently or when dependency isolation is real.

Do not split by file, specification paragraph, or implementation step. Do not turn the bead into line-by-line pseudocode. Leave freedom for private helper structure, local naming, collections, and allocation choices that do not affect specified behavior.

## 4. Classify the authoring outcome

Choose one outcome before writing a bead.

### Authorable implementation bead

Use only when no unresolved semantic, architectural, or scoping decision remains. Create the ordinary complete `Pending Approval` bead described below.

### Design-blocked work

Use when authoritative specifications are missing, contradictory, or genuinely undecided. Record the exact conflicting sources or missing decision and why implementation cannot proceed. Do not present it as a routine implementation bead awaiting approval.

For a new design question, use:

```bash
br create --title "Design: <decision needed>" --type question --status blocked \
  --labels "workflow:blocked-design,<domain-labels>" \
  --description "<decision surface, sources, conflict, and required resolution>" \
  --no-auto-flush --json
```

If an existing bead owns the work, update that bead to status `blocked` with `workflow:blocked-design` instead of creating a duplicate.

### Rescoping-blocked work

Use when a material human decision about boundaries or priorities is required and no unambiguous decomposition follows from the specs. Record the exact choice rather than selecting an arbitrary split:

```bash
br create --title "Rescope: <task boundary>" --type task --status blocked \
  --labels "workflow:blocked-rescoping,<domain-labels>" \
  --description "<boundary problem, alternatives, and required decision>" \
  --no-auto-flush --json
```

## 5. Write an authorable implementation bead

Put the normative contract in `description` using this shape:

```text
Objective:
<one accepted outcome>

Specification baseline:
- Repository commit: <full git rev-parse HEAD value>

Authoritative sources:
- docs/specs/<file>, § "<section heading>": <brief governing requirement>

Obligations:
- O1: <executable normative requirement>
- O2: <closely related requirement>

Forbidden outcomes:
- <plausible but noncompliant result>

Scope:
- <included semantic work>

Non-goals:
- <explicitly excluded adjacent work>

Concern flags:
- <only applicable risks such as persistence, public API, failure atomicity>

Dependencies and blockers:
- <bead IDs or None known>

Expected change surface:
- <likely modules, files, or resources>

Bead approval:
Status: Pending Approval
Reason: Awaiting initial independent approval.
Known blockers: None
```

Group requirements into a manageable number of obligation IDs. Preserve material detail, but do not create one obligation per sentence. The expected change surface is a coordination clue, not a lock or permission boundary.

Put required proof in `acceptance_criteria`:

```text
Required evidence:
- O1: <test, inspection, artifact, or command>
- O2: <test, inspection, artifact, or command>
- Regression coverage: <when applicable>
- Additional mandatory verification: <checks beyond repository gates>

Completion criteria:
<conditions that make the semantic slice finished>
```

Use `design` only for helpful rationale, settled tradeoffs, examples, or edge cases. Do not hide requirements there. Reserve `notes` for later implementation evidence and review history.

The Beads storage status `draft` plus `workflow:pending-approval` represents the semantic state `Pending Approval`; `draft` is not the completed-state meaning. Preserve a priority assigned by the user; otherwise omit `--priority` and use the repository default rather than inventing urgency:

```bash
br create --title "<imperative outcome>" --type task \
  --status draft --labels "workflow:pending-approval,<domain-labels>" \
  --description "<contract>" --no-auto-flush --json
br update <id> --acceptance-criteria "<evidence contract>" \
  --no-auto-flush --json
```

Add real blocking dependencies with:

```bash
br dep add <id> <depends-on-id> --type blocks --no-auto-flush --json
```

## 6. Perform an author sanity check without approving it

Re-read the complete stored bead with `br show <id> --json`. Confirm:

- every material obligation has direct or adjacent authoritative support;
- the stored specification baseline is the full pre-authoring `HEAD` commit;
- every source reference names a stable section heading rather than a line number;
- no source requirement within scope disappeared during summarization;
- forbidden outcomes close realistic noncompliant routes;
- scope and non-goals do not contradict each other;
- evidence is observable and can fail;
- dependencies match the inspected code and existing beads;
- no unresolved design decision is disguised as implementation freedom;
- the approval section says `Pending Approval`, awaiting initial independent approval, with no known blocker.

Do not run `br sync --flush-only`, stage files, or commit in the authoring role. Use `--no-auto-flush` for mutations so the completed `Pending Approval` state remains uncommitted for the first independent reviewer. That reviewer will approve, materially revise, or block it and create the durable planning commit.

Report the bead ID, semantic state, objective, specification baseline, governing sections, dependencies, and any exact design or rescoping blocker. Never mark your own bead `Ready` and never start implementation in this role.

## Exceptional reference

Consult `docs/specs/bead_review_implementation_commit_process.md` only when the ordinary procedure does not determine an unusual lifecycle, recovery, or coordination case.
