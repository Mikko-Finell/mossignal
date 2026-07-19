---
name: review-implementation
description: Perform the corrective acceptance review of a completed mossignal implementation bead. Use when asked to review, inspect, verify, or accept an implemented or closed bead. Audit the implementation against its bounded obligations, correct in-scope defects immediately, run acceptance checks, preserve the bead's closed state, and finish with accepted repository state. Do not use for a user-requested read-only review.
---

# Review an implementation

Treat implementation review as the final corrective pass over a completed bead.
Find, fix, verify, and accept in one workflow. Do not merely produce a defect
list for another agent.

## Establish the review boundary

1. Read `AGENTS.md`.
2. Inspect `git status --short --branch`, recent commits, the complete proposal
   diff, and unexplained working-tree changes.
3. Read the completed bead with `br show <id> --json` and confirm its current
   state with `br sync --status`.
4. Read the applicable reviewed contracts named by the bead. Reuse unchanged
   represented rules without re-auditing them. Read authoritative specification
   sections only for an applicable unrepresented facet, changed contract source,
   concrete contradiction, or material ambiguity.
5. Derive a bounded review checklist from the bead's included scope, exclusions,
   implementation freedom, and required verification.

Preserve unrelated work. Do not broaden review into adjacent roadmap work,
contract polish, speculative architecture, or implementation of known-uncovered
facets.

## Review and correct

Inspect the implementation, public surface, tests, documentation, and repository
integration against the bounded checklist. Look for concrete correctness,
determinism, error-handling, API, verification, and scope defects.

Correct every in-scope defect immediately. For behavioral defects, follow the
repository regression-test-first rule: add or adjust the focused test, observe
the relevant failure, then correct the implementation. Make the smallest
coherent correction and continue reviewing the corrected result.

Do not return a findings-only report, ask the implementer to fix ordinary
defects, create follow-up beads for obligations already in scope, or treat
optional improvements as acceptance failures.

## Preserve bead state

Keep a completed bead closed throughout corrective review. Do not reopen it to
signal that corrections are underway or because its first implementation
attempt was imperfect. Do not rewrite its description merely to match incidental
implementation details.

If acceptance genuinely requires a material scope change, unresolved observable
product policy, new external authority, or modification of unrelated work, stop
and report the exact blocker. Leave bead-state changes to renewed planning or
explicit user direction; do not independently reopen it.

## Handle contracts discovered during implementation

If the implementation produced draft contracts, require their independent
contract review before final acceptance. Do not use implementation review to
self-promote an implementer's contract claims. An unchanged reviewed contract
is done at its recorded baseline and is not a general review target.

## Verify and accept

1. Run the narrowest relevant tests while correcting defects.
2. Run every bead-specific verification obligation.
3. Run `make check-dev`, then `make check-final`.
4. Confirm the bead remains closed and its dependencies are coherent.
5. Run `br sync --flush-only` and confirm synchronization.
6. Inspect the final diff and commit composition. Ensure only the bead's accepted
   implementation, tests, authorized contract records, and synchronized bead
   export are included.
7. Follow `AGENTS.md` for the accepted implementation commit. Do not push.

Report the corrections made, final acceptance status, and checks run. Mention a
remaining issue only when it is a genuine blocker under `AGENTS.md`. Never
include line-of-code citations in the close-out report.
