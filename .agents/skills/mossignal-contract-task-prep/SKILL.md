---
name: mossignal-contract-task-prep
description: Prepare a mossignal feature or implementation task by identifying its applicable specification contracts, creating or updating compact draft contract records under docs/specs/contracts, and deriving or refining the implementation bead. Use before implementation. Do not use to implement code or independently approve the resulting contracts.
---

# Mossignal contract task preparation

Prepare one implementation task from the authoritative repository specifications while preserving the reusable research as compact contract records.

Stop before implementation.

## Inputs

Accept any of these as the task starting point:

- a feature request or roadmap item;
- a provisional implementation bead;
- an existing bead that needs specification grounding;
- one or more existing contract records that may need expansion.

Use the current repository working tree, not an assumed remote revision.

## Required outputs

Produce all applicable outputs:

1. New or updated `draft` contract records under `docs/specs/contracts/`.
2. A finalized or refined implementation bead using the repository's existing bead conventions.
3. A concise handoff naming:
   - contracts used;
   - contracts created or changed;
   - unresolved product questions;
   - important task exclusions;
   - files or bead fields changed.

Do not create an implementation patch.

## Workflow

### 1. Establish the baseline and route the task

- Run `git rev-parse HEAD` and `git status --short`.
- Record whether governing specifications or existing contracts have working-tree changes.
- Read `docs/specs/contracts/README.md` and inspect existing contract records.
- Read `references/concept-boundaries.md` before deciding contract boundaries.
- Determine provisionally whether the task:
  - uses existing contracts unchanged;
  - expands existing contracts;
  - requires new coherent-subject contracts;
  - combines several contracts.
- Treat any existing bead as provisional evidence, not specification authority.

### 2. Research and author the contracts

- Read `references/contract-authoring.md` and `docs/specs/contracts/_template.yaml`.
- Search every authoritative specification that may define, constrain, encode, migrate, diagnose, or verify the subject.
- Reconcile terminology, ownership, normative strength, scope, exclusions, and implementation freedom.
- Create or update the smallest coherent set of reusable contract records.
- State every unique contract fact once.
- Attach exact document-and-heading source references.
- Mark every changed contract `draft`.
- Record the source baseline and any relevant dirty specification paths.
- Preserve genuine uncertainty in `open_questions`; do not invent a resolution.
- If the complete subject was not researched, mark coverage `partial` and identify the uncovered facets.

### 3. Derive or refine the bead

- Read `references/bead-derivation.md`.
- Use the researched contracts to finalize the implementation task.
- Reference every governing contract by ID and path.
- Include only task-relevant requirements, exclusions, freedoms, and verification expectations.
- Keep reusable product truth in contracts and task-specific sequencing or file scope in the bead.
- If an unresolved contract question blocks implementation, leave the bead unready and surface the decision.
- Stop after the contracts and bead are internally consistent.

## Guardrails

- Do not create one contract per rule, method, heading, or task.
- Do not copy entire specification sections into YAML.
- Do not treat silence about representation, helper names, or conveniences as a defect.
- Do not turn examples, recommendations, or representative Rust into exact requirements.
- Do not weaken `MUST` or strengthen `SHOULD`/`MAY`.
- Do not duplicate one fact as a rule, prohibition, relation, and verification item.
- Do not add tests that merely restate every rule unless the testing specification imposes a distinct obligation.
- Do not promote a changed contract to `reviewed`.
- Do not make the bead an independent source of product truth.
- Do not implement code.

## Completion check

Before stopping, verify:

- each contract represents a coherent reusable subject;
- every bead requirement is supported by a referenced contract;
- every changed contract identifies its exact source baseline;
- implementation freedom and open questions are not conflated;
- no task-specific detail polluted the reusable contract;
- the result is compact enough to be useful to a future agent.
