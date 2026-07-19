# AGENTS.md

## Authority

Normative mossignal specifications live under `docs/specs/`. Other material
under `docs/` is not authoritative unless this file or a specification says it
is.

Search `docs/specs/` directly by symbol or concept to locate relevant
authoritative sections quickly. Read the governing sections before making
semantic, API, architecture, validation, or testing decisions.

Example:
```bash
rg 'NetworkBuilder<D>|topology patch' docs/specs/
```

Then read the cited section in the authoritative spec file.

Do not invent architecture or semantics when the documents are missing,
contradictory, or genuinely undecided. Surface the exact design blocker.

The bead and commit rules in this file govern ordinary product work.
`docs/testing_and_verification_policy.md` governs verification depth.

## Specification contracts and bounded progress

Specification contracts under `docs/specs/contracts/` exist so agents do not
repeatedly rediscover and reconcile the same requirements across the full
specification corpus. Each contract is a compact, source-linked, independently
reviewed record of one coherent subject. Specifications remain authoritative;
contracts preserve durable shared understanding for task preparation,
implementation, and review.

The working model is:

```text
authoritative specifications
        ↓
reusable reviewed contracts
        ↓
one bounded implementation bead
        ↓
implementation and verification
```

`reviewed` means done and accepted for reuse at the recorded evidence baseline,
not globally complete. Reuse represented rules backed by unchanged cited sources
without re-auditing their source support, completeness, or editorial quality.
Reopen a reviewed contract only when a cited source changed, the current task
needs an unrepresented facet, a concrete source contradiction is found, a
represented rule permits materially different observable outcomes for the
current bead, or applicable contracts concretely contradict each other.

`known_uncovered` records specific adjacent or future coverage and is compatible
with `reviewed`. It does not block work unless the current bead actually depends
on that facet. Missing private representation details, optional refinements, and
theoretical broader coverage are not blockers.

Early foundational implementation is expected to uncover reusable
specification-backed knowledge. An implementer may create or extend draft
contracts to preserve it and may continue when the specifications and approved
bead already determine the behavior. The implementer must not promote its own
draft, invent product policy, or materially expand the bead without renewed
planning review. All implementation-discovered contract changes require
independent review before final implementation acceptance.

A claimed design blocker must identify all of:

```text
represented contract requirement
current bead obligation
materially different observable outcomes
authoritative specification ambiguity
why implementation freedom cannot resolve it
```

If any item is missing, classify the matter as `known_uncovered`, implementation
freedom, adjacent future work, an optional improvement, or irrelevant to the
current slice, and continue.

## Repository model

- Ordinary work happens directly on `main`.
- `HEAD` is the last accepted state; the complete working-tree difference is
  the current proposal.
- Code-changing agents operate sequentially unless the user explicitly
  authorizes independent parallel work.
- Inspect the branch, `HEAD`, working tree, active bead, dependencies, and
  expected change surface before editing or reviewing.
- Treat unexplained changes as user or other-agent work. Do not overwrite,
  discard, stage, or commit them.
- Never use destructive cleanup commands against unexplained work.
- Do not create intermediate implementation commits. The normal accepted
  product commit contains implementation, tests, closed bead state, and the
  synchronized `.beads` record together.

## Architecture and failure discipline

- Start with modules. Add crates only for durable independent boundaries.
- Default to private items and `pub(crate)` visibility. Treat every public item
  as deliberate architecture requiring documentation and appropriate examples.
- Prefer concrete domain types and enums. Add traits only at durable replacement
  points or true I/O boundaries.
- Do not add compatibility shims, deprecated aliases, speculative abstractions,
  or `build.rs` without a real requirement.
- Production code must not use `unwrap` or `expect`.
- Return `Option` for ordinary absence and `Result` for real failure.
- Surface errors with enough context to identify what failed and where.
- Panic only for a proven internal invariant violation where continuing could
  corrupt state, and state that invariant in the message.

## Testing and checks

For a bug fix, write the failing regression test first, run it, and observe the
failure before correcting the defect.

Use the narrowest relevant tests while iterating. Then use:

```bash
make check-dev
```

This is the quiet, fast implementation gate. Success prints one `OK` line;
failure prints the failed command and captured diagnostics.

Final acceptance uses:

```bash
make check-final
```

This is the comprehensive finite repository gate. Add new check categories to
the appropriate Make target when their underlying facilities become real.
Long fuzz campaigns and other scheduled verification remain separate unless a
bead explicitly makes one mandatory.

A warning, unavailable command, timeout, flaky result, or failing mandatory
check is not a pass. Do not weaken a test, oracle, lint, or gate merely to clear
the current task.

## Python tooling

Repository Python scripts use the locked `uv` environment. Run them with `uv run --locked python`; Make targets already do this. Do not install their dependencies with `pip` or system Python.

## Specification trace comments

Use `// SPEC:` comments where a non-obvious implementation choice directly enforces a normative specification requirement or invariant that a future maintainer might otherwise weaken, remove, or “simplify” incorrectly.

Keep comments brief. Reference the specification path and section heading, then state the implementation consequence. NEVER add `SPEC:` comments for obvious code, repeat rustdoc, quote long passages, or use line numbers.

Example:

```rust
// SPEC: docs/specs/concrete_rust_api_surface.md §16 "Stable keys"
// Structural category remains part of identity even when opaque payloads are equal.
```

## Beads and commits

Use `br` for issue mutation and `br ... --json` for machine-readable inspection.
Use `br ready --json` for approved claimable implementation work. Bead authors
use `--no-auto-flush` and do not commit. Bead reviewers flush and commit each
completed planning pass. Implementation and implementation-review roles use
`--no-auto-flush`; during final acceptance, after closing the bead, run:

```bash
br sync --flush-only
```

before staging the accepted commit.

Accepted implementation commit subjects use:

```text
<bead-id>: <imperative semantic summary>
```

Do not push unless the user separately authorizes it.

## Close-out reports

Never include line-of-code citations in close-out reports.

<!-- br-agent-instructions-v1 -->

---

## Beads Workflow Integration

This project uses [beads_rust](https://github.com/Dicklesworthstone/beads_rust) (`br`/`bd`) for issue tracking. Issues are stored in `.beads/` and tracked in git.

### Essential Commands

```bash
# View ready issues (open, unblocked, not deferred)
br ready              # or: bd ready

# List and search
br list --status=open # All open issues
br show <id>          # Full issue details with dependencies
br search "keyword"   # Full-text search

# Create and update
br create --title="..." --description="..." --type=task --priority=2
br update <id> --status=in_progress
br close <id> --reason="Completed"
br close <id1> <id2>  # Close multiple issues at once

# Sync with git
br sync --flush-only  # Export DB to JSONL
br sync --status      # Check sync status
```

### Workflow Pattern

1. **Start**: Run `br ready` to find actionable work
2. **Claim**: Use `br update <id> --status=in_progress`
3. **Work**: Implement the task
4. **Complete**: Use `br close <id>`
5. **Sync**: Always run `br sync --flush-only` at session end

### Key Concepts

- **Dependencies**: Issues can block other issues. `br ready` shows only open, unblocked work.
- **Priority**: P0=critical, P1=high, P2=medium, P3=low, P4=backlog (use numbers 0-4, not words)
- **Types**: task, bug, feature, epic, chore, docs, question
- **Blocking**: `br dep add <issue> <depends-on>` to add dependencies

<!-- end-br-agent-instructions -->

<!-- bv-agent-instructions-v2 -->

---

## Beads Workflow Integration

This project uses [beads_rust](https://github.com/Dicklesworthstone/beads_rust) (`br`) for issue tracking and [beads_viewer](https://github.com/Dicklesworthstone/beads_viewer) (`bv`) for graph-aware triage. Issues are stored in `.beads/` and tracked in git.

### Using bv as an AI sidecar

bv is a graph-aware triage engine for Beads projects (.beads/beads.jsonl). Instead of parsing JSONL or hallucinating graph traversal, use robot flags for deterministic, dependency-aware outputs with precomputed metrics (PageRank, betweenness, critical path, cycles, HITS, eigenvector, k-core).

**Scope boundary:** bv handles *what to work on* (triage, priority, planning). `br` handles creating, modifying, and closing beads.

**CRITICAL: Use ONLY --robot-* flags. Bare bv launches an interactive TUI that blocks your session.**

#### The Workflow: Start With Triage

**`bv --robot-triage` is your single entry point.** It returns everything you need in one call:
- `quick_ref`: at-a-glance counts + top 3 picks
- `recommendations`: ranked actionable items with scores, reasons, unblock info
- `quick_wins`: low-effort high-impact items
- `blockers_to_clear`: items that unblock the most downstream work
- `project_health`: status/type/priority distributions, graph metrics
- `commands`: copy-paste shell commands for next steps

```bash
bv --robot-triage        # THE MEGA-COMMAND: start here
bv --robot-next          # Minimal: just the single top pick + claim command

# Token-optimized output (TOON) for lower LLM context usage:
bv --robot-triage --format toon
```

Before claiming, verify current state with `br show <id> --json` or `br ready --json`. `recommendations` can include graph-important blocked or assigned work; only `quick_ref.top_picks` and non-empty `claim_command` fields represent claimable work.

#### Other bv Commands

| Command | Returns |
|---------|---------|
| `--robot-plan` | Parallel execution tracks with unblocks lists |
| `--robot-priority` | Priority misalignment detection with confidence |
| `--robot-insights` | Full metrics: PageRank, betweenness, HITS, eigenvector, critical path, cycles, k-core |
| `--robot-alerts` | Stale issues, blocking cascades, priority mismatches |
| `--robot-suggest` | Hygiene: duplicates, missing deps, label suggestions, cycle breaks |
| `--robot-diff --diff-since <ref>` | Changes since ref: new/closed/modified issues |
| `--robot-graph [--graph-format=json\|dot\|mermaid]` | Dependency graph export |

#### Scoping & Filtering

```bash
bv --robot-plan --label backend              # Scope to label's subgraph
bv --robot-insights --as-of HEAD~30          # Historical point-in-time
bv --recipe actionable --robot-plan          # Pre-filter: ready to work (no blockers)
bv --recipe high-impact --robot-triage       # Pre-filter: top PageRank scores
```

### br Commands for Issue Management

```bash
br ready              # Show issues ready to work (no blockers)
br list --status=open # All open issues
br show <id>          # Full issue details with dependencies
br create --title="..." --type=task --priority=2
br update <id> --status=in_progress
br close <id> --reason="Completed"
br close <id1> <id2>  # Close multiple issues at once
br sync --flush-only  # Export DB to JSONL
```

### Workflow Pattern

1. **Triage**: Run `bv --robot-triage` to find the highest-impact actionable work
2. **Claim**: Use `br update <id> --status=in_progress`
3. **Work**: Implement the task
4. **Complete**: Use `br close <id>`
5. **Sync**: Always run `br sync --flush-only` at session end

### Key Concepts

- **Dependencies**: Issues can block other issues. `br ready` shows only unblocked work.
- **Priority**: P0=critical, P1=high, P2=medium, P3=low, P4=backlog (use numbers 0-4, not words)
- **Types**: task, bug, feature, epic, chore, docs, question
- **Blocking**: `br dep add <issue> <depends-on>` to add dependencies

<!-- end-bv-agent-instructions -->
