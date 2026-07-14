# Authoring compact mossignal contracts

The current specifications remain authoritative. A contract record is a reusable, source-grounded view of their combined requirements.

## Research order

1. Capture the exact repository baseline and working-tree state.
2. Inspect existing contracts and aliases before creating a new record.
3. Search all product specifications for the subject, its symbols, aliases, adjacent concepts, and standard contract facets.
4. Read complete governing sections, not isolated search snippets.
5. Reconcile the sources before writing the record.

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

## Status and coverage

Use separate fields:

- `status`: `draft` or `reviewed`;
- `coverage.state`: `complete` or `partial`.

Any contract created or changed by this skill is `draft`.

Use `complete` only after researching the full coherent subject across all applicable specifications. Otherwise use `partial` and name the uncovered facets. A bead must not rely on a partial contract outside its recorded coverage.

## Rule organization

State each unique fact once.

Use:

- `requirements` for mandatory semantics, exact public responsibilities, prohibitions, and required cross-system effects;
- `recommendations` for normative `SHOULD`/`SHOULD NOT` guidance;
- `implementation_freedom` for a small number of freedoms worth making explicit;
- `open_questions` only for unresolved observable product behavior or required capabilities.

A missing constructor name, accessor, private field, derive, module path, storage type, or convenience is not an open question unless the specifications require a stable observable commitment.

Preserve source strength:

- do not strengthen examples or `SHOULD` language into requirements;
- do not weaken `MUST` or exact public responsibilities;
- state an exact API name or shape only when the concrete API authority makes it exact;
- mark a claim `basis: derived` only when it is a necessary consequence rather than a direct source statement; omit `basis` for direct rules.

## Source references

Define compact source aliases once under `sources` and cite those aliases from rules.

Each source entry should include:

- repository-relative document path;
- exact heading or heading path;
- optional authority note when the relationship is not obvious.

Prefer headings and a baseline commit over fragile line numbers.

## Scope

The contract scope describes the reusable subject, not the current implementation task.

Use `scope.includes` and `scope.excludes` only when they clarify the subject boundary. Task file lists, sequencing, milestone exclusions, and implementation staging belong in the bead.

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
