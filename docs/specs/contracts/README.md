# Specification contracts

This directory contains compact, reusable contract records for coherent mossignal subjects.

The records assemble requirements that may be distributed across several authoritative specifications. They exist so task preparation, implementation, and review can reuse the same source-grounded understanding instead of reconstructing it repeatedly.

## Current authority policy

The product specifications remain authoritative.

Contract status means:

- `draft`: newly created or changed and not independently reviewed;
- `reviewed`: independently checked against the cited specification baseline.

If a contract conflicts with an authoritative specification at its recorded baseline, the specification governs and the conflict must be corrected.

## Coverage

Status and completeness are separate:

- `coverage.state: complete` means the full coherent subject was researched across all applicable specifications;
- `coverage.state: partial` means only named facets were researched and uncovered facets are listed.

Do not treat a partial contract as complete authority outside its included coverage.

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
- Cite repository-relative document paths and headings.
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
