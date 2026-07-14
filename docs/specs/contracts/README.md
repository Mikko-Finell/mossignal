# Specification contracts

This directory contains compact, reusable contract records for coherent mossignal subjects.

The records assemble requirements that may be distributed across several authoritative specifications. They exist so task preparation, implementation, and review can reuse the same source-grounded understanding instead of reconstructing it repeatedly.

## Current authority policy

The product specifications remain authoritative.

Contract status means:

- `draft`: newly created or changed and not independently reviewed;
- `reviewed`: independently checked against cited source evidence; every cited source has a reviewed hash.

If a contract conflicts with an authoritative specification at its recorded baseline, the specification governs and the conflict must be corrected.

Reviewed hashes are evidence fingerprints, not claims that a contract completely covers its coherent subject. A different repository `HEAD` alone does not invalidate a reviewed contract.

## Scope boundaries

Use explicit subject boundaries instead of an unfalsifiable completeness field:

- `scope.includes` identifies the reusable facets represented by the record;
- `scope.excludes` identifies adjacent facets owned elsewhere;
- `known_uncovered` names a specific facet already known to be absent, and is omitted when empty.

Task preparation must research a requested feature beyond these boundaries when needed. It can detect that a task needs an explicitly excluded or known-uncovered facet, but it must not claim that a contract is generally complete.

## Source evidence

Each source records a repository-relative document and an exact `heading_path`, from the outermost heading through the cited heading. The contract utility extracts that heading, its body, and nested subsections; it stops at the next heading of the same or higher level. It normalizes line endings, removes trailing horizontal whitespace, ensures one final newline, and hashes the resulting UTF-8 bytes with SHA-256.

- An `unchanged` source is byte-equivalent to the previously reviewed normalized evidence; rules citing it may be reused.
- A `changed` source requires renewed semantic review only for the rules that cite it.
- `missing`, `ambiguous`, and `not_fingerprinted` sources likewise require review of their citing rules.

Only independent review records `reviewed_hash` values. Task preparation never silently adds or refreshes them.

## Contract utility

Install the agent-tool dependencies listed in `scripts/requirements-agent-tools.txt`, then use:

```text
python3 scripts/contracts.py catalog
python3 scripts/contracts.py status docs/specs/contracts/<contract>.yaml
python3 scripts/contracts.py fingerprint docs/specs/contracts/<contract>.yaml
```

`catalog` lists compact metadata for contract selection and ignores `_template.yaml`. `status` is read-only and reports source status plus citing rule IDs. `fingerprint` prints current hashes without writing them. None of these commands decides semantic meaning, audits every contract, promotes a contract, or changes stored hashes.

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

New or changed records begin as `draft`. Task preparation derives the implementation bead from the contracts. Independent review and promotion are separate work and are not performed by the contract-task-preparation skill.
