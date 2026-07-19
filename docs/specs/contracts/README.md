# Specification contracts

This directory contains compact, reusable contract records for coherent mossignal subjects.

The records assemble requirements that may be distributed across several authoritative specifications. They exist so task preparation, implementation, and review can reuse the same source-grounded understanding instead of reconstructing it repeatedly.

## Current authority policy

The product specifications remain authoritative.

Contract status means:

- `draft`: newly created or changed and not independently reviewed;
- `reviewed`: independently checked against cited source evidence, done, and
  accepted for reuse at that evidence baseline; every cited source has a
  reviewed hash.

If a contract conflicts with an authoritative specification at its recorded baseline, the specification governs and the conflict must be corrected.

Reviewed hashes are evidence fingerprints, not claims that a contract completely covers its coherent subject. A different repository `HEAD` alone does not invalidate a reviewed contract.

Treat an unchanged reviewed contract as settled shared understanding. Task
preparation, implementation, and review reuse its represented rules without
reconstructing their source support from the specifications. Reopen it only for
a changed cited source, a facet required by the current task but outside its
represented scope, a concrete contradiction with authority, a represented
ambiguity that materially affects the current bead, or a concrete contradiction
with another applicable contract. Optional refinements, additional possible
citations, and generalized caution are not reopening triggers.

## Scope boundaries

Use explicit subject boundaries instead of an unfalsifiable completeness field:

- `scope.includes` identifies the reusable facets represented by the record;
- `scope.excludes` identifies adjacent facets owned elsewhere;
- `known_uncovered` names a specific facet already known to be absent, and is omitted when empty.

Task preparation must research a requested feature beyond these boundaries when needed. It can detect that a task needs an explicitly excluded or known-uncovered facet, but it must not claim that a contract is generally complete.

A reviewed contract may contain `known_uncovered`. Adjacent, future, and
out-of-scope behavior does not prevent promotion and does not block a bead that
does not depend on it. `open_questions` is reserved for unresolved observable
behavior or required public capability inside `scope.includes`.

## Good-enough review

A draft is ready for promotion when its represented boundary is clear, every
stated rule is supported at the cited evidence baseline, applicable rules are
internally coherent, no unresolved observable behavior remains inside that
boundary, and unspecified implementation choices remain free.

Promotion does not require exhaustive subject coverage, ideal decomposition,
every useful example or citation, future variants and contexts, private helper
design, or proof that no later task will extend the record. Review establishes a
trustworthy reusable baseline; once established, reuse is the default and
reopening requires concrete evidence.

## Source evidence

Each source records a repository-relative document and an exact `heading_path`, from the outermost heading through the cited heading. The contract utility extracts that heading, its body, and nested subsections; it stops at the next heading of the same or higher level. It normalizes line endings, removes trailing horizontal whitespace, ensures one final newline, and hashes the resulting UTF-8 bytes with SHA-256.

- An `unchanged` source is byte-equivalent to the previously reviewed normalized evidence; rules citing it may be reused.
- A `changed` source requires renewed semantic review only for the rules that cite it.
- `missing`, `ambiguous`, and `not_fingerprinted` sources likewise require review of their citing rules.

Only independent review records `reviewed_hash` values. Task preparation never silently adds or refreshes them.

## Contract utility

Use the repository's locked `uv` environment:

```text
uv run --locked python scripts/contracts.py catalog
uv run --locked python scripts/contracts.py status docs/specs/contracts/<contract>.yaml
uv run --locked python scripts/contracts.py fingerprint docs/specs/contracts/<contract>.yaml
uv run --locked python scripts/contracts.py coverage
```

`catalog` lists compact metadata for contract selection and ignores `_template.yaml`. `status` is read-only and reports source status plus citing rule IDs. `fingerprint` prints current hashes without writing them. None of these commands decides semantic meaning, audits every contract, promotes a contract, or changes stored hashes.

`coverage` reports the approximate specification evidence footprint of rule-backed
contract citations. It separates all references, draft references, and unchanged
reviewed references, and counts headings, normalized bytes, and paragraphs using
normative keywords. This is a research-gap heuristic, not a semantic completeness
claim. Use `--format json` for machine-readable output or repeat `--document` to
limit the report to selected specification files. Use `--format dump` for detailed
per-document counts instead of the default percentage tables. The same default
report is available as `make contract-coverage`.

## What belongs here

Create one record per coherent reusable subject, such as:

- `DiagnosticPath`;
- stable structural keys;
- `NetworkFingerprint`;
- machine lifecycle;
- one built-in node or a tightly regular node family.

Do not create one record per task, rule, method, heading, or test.

## Authoring rules

- Record the exact source commit and relevant dirty specification paths.
- State every unique fact once.
- Preserve normative strength and exactness.
- Cite repository-relative document paths and exact heading paths.
- Keep task-specific scope in the bead.
- Keep representation and conveniences open unless the specifications constrain them.
- Use `open_questions` only for unresolved observable behavior or required public capability.
- Delete unused optional sections from the template.
- Prefer compact records; inspect records over roughly 200 lines for duplication or poor boundaries.

## File naming

Use lowercase kebab-case filenames, for example:

```text
diagnostic-path.yaml
stable-structural-keys.yaml
machine-lifecycle.yaml
```

Use stable dotted IDs inside records, for example:

```text
mossignal.metadata.diagnostic_path
mossignal.identity.stable_structural_keys
```

## Workflow

Contracts accumulate incrementally. Task preparation reuses reviewed records,
researches only changed, uncovered, and task-specific facets, creates or extends
draft records for reusable discoveries, and derives the implementation bead.
Independent review promotes sound drafts and reviews the bounded bead.

Early implementation may expose additional reusable specification-backed facts.
The implementer may preserve those facts in new or changed draft contracts when
the specifications and approved bead already determine the behavior. Such drafts
do not authorize new product policy or scope expansion and must be independently
reviewed before final implementation acceptance. Later tasks then reuse the
larger reviewed contract set instead of repeating the same specification
research.
