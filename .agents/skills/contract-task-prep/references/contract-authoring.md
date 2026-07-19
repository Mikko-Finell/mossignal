# Authoring compact mossignal contracts

The current specifications remain authoritative. A contract record is a
reusable, source-grounded view of their combined requirements, preserved so
future agents do not repeat the same reconciliation across the specification
corpus.

## Research order

1. Capture the exact repository baseline and working-tree state.
2. Run `uv run --locked python scripts/contracts.py catalog` and select likely contracts by metadata and scope; do not inspect every record.
3. Run `uv run --locked python scripts/contracts.py status` for only those selected records.
4. Reuse reviewed rules backed only by unchanged sources as settled shared
   understanding. Recheck only rules citing changed, missing, ambiguous, or
   unfingerprinted sources, or rules implicated by a concrete contradiction.
5. Search the task's applicable product specifications for its symbols, aliases, adjacent concepts, and standard contract facets.
6. Read complete governing sections, not isolated search snippets.
7. Map each applicable discovered requirement to an unchanged rule, a new or corrected rule, a new coherent contract, or an explicit outside-task determination.
8. Reconcile the sources before writing the record.

Search beyond the obvious API section when applicable:

- semantics and invariants;
- public Rust responsibilities;
- construction and validation;
- lifecycle and runtime behavior;
- identity, equality, hashing, or ordering;
- diagnostics and rendering;
- persistence and compatibility;
- reconfiguration and migration;
- distinct verification obligations;
- explicit prohibitions and freedoms.

## Baseline

Every changed contract records:

- `source_baseline.commit` from `git rev-parse HEAD`;
- whether the working tree is clean;
- relevant modified specification paths when it is dirty.

Do not describe a dirty working-tree contract as if the commit alone contained its sources.

## Status, scope, and reviewed evidence

Use one status field:

- `status`: `draft` or `reviewed`;

Any contract created or changed by this skill is `draft`.

Use `scope.includes` and `scope.excludes` to state explicit reusable subject
boundaries. Use `known_uncovered` for a specific adjacent or future facet not
represented by the record, and omit it when empty. A reviewed contract may have
`known_uncovered`; never claim that a contract is globally complete.

Reviewed hashes are evidence fingerprints, not completeness claims. Only
independent review records them. An unchanged reviewed contract is done and
accepted for reuse at that baseline. A changed, missing, ambiguous, or
unfingerprinted source requires semantic rechecking only for rules that cite it.
A different repository `HEAD`, optional refinement, or generalized suspicion
does not make a contract stale. Task preparation must never silently refresh
reviewed hashes.

## Rule organization

State each unique fact once.

Use:

- `requirements` for mandatory semantics, exact public responsibilities, prohibitions, and required cross-system effects;
- `recommendations` for normative `SHOULD`/`SHOULD NOT` guidance;
- `implementation_freedom` for a small number of freedoms worth making explicit;
- `open_questions` only for unresolved observable product behavior or required
  capabilities inside `scope.includes`.

A missing constructor name, accessor, private field, derive, module path, storage type, or convenience is not an open question unless the specifications require a stable observable commitment.

## Good enough

A draft is ready for independent promotion when its represented boundary is
clear, each stated rule is supported, applicable rules are coherent, no
unresolved observable behavior remains inside the boundary, and implementation
freedom is preserved. It need not exhaust the eventual subject, cite every
possibly relevant paragraph, define future variants, or choose private helper
design. Further optional polish is not a reason to keep it draft.

Preserve source strength:

- do not strengthen examples or `SHOULD` language into requirements;
- do not weaken `MUST` or exact public responsibilities;
- state an exact API name or shape only when the concrete API authority makes it exact;
- mark a claim `basis: derived` only when it is a necessary consequence rather than a direct source statement; omit `basis` for direct rules.

## Source references

Define compact source aliases once under `sources` and cite those aliases from rules.

Each source entry should include:

- repository-relative document path;
- exact heading path from outermost parent to cited heading;
- a `reviewed_hash` only when independent review has recorded it;
- optional authority note when the relationship is not obvious.

The utility parses ATX Markdown headings, includes a matched heading and nested subsections through the next same-or-higher heading, normalizes only line endings and trailing horizontal whitespace, ensures one final newline, then hashes UTF-8 bytes with SHA-256. Duplicate full heading paths are ambiguous. Prefer this deterministic evidence over fragile line numbers.

## Scope

The contract scope describes the reusable subject, not the current implementation task.

Use `scope.includes` and `scope.excludes` only when they clarify the subject boundary. Task file lists, sequencing, milestone exclusions, and implementation staging belong in the bead.

### Task artifacts are provisional

Roadmaps and beads define task scope, not reusable product truth.

* Keep task sequencing, implementation staging, file scope, and bead-specific questions out of contracts.
* Treat a question as a contract open question only when the authoritative
  specifications genuinely leave represented product behavior unresolved.
* When roadmap wording conflicts with the specifications, record the mismatch in the handoff and refine the bead. Treat the roadmap as a temporary bootstrapping artifact and do not edit it based on contracts being authored in the same run.

## Compactness

Soft expectations:

- small subject: about 30–100 lines;
- medium subject or family: about 80–180 lines;
- over roughly 200 lines: inspect for duplication or independent subcontracts.

Do not add:

- migration rationale;
- adoption assessments;
- repeated authority summaries when rules already cite sources;
- positive and negative restatements of the same fact;
- verification entries that merely paraphrase requirements;
- empty or `not applicable` sections;
- a generated prose packet inside the canonical record.

Use `docs/specs/contracts/_template.yaml` as the starting shape. Delete unused optional fields rather than preserving empty scaffolding.
