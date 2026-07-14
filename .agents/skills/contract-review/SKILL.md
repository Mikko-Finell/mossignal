---
name: review-contracts
description: Independently audit and correct changed draft specification contracts against the authoritative repository specifications, promote sound contracts, and then review any directly derived bead. Use after contract-task-prep or whenever uncommitted draft contracts under docs/specs/contracts need editorial review. Do not use for implementation or findings-only commentary.
---

# Review specification contracts

Turn changed draft contracts into a source-grounded reviewed package. Treat drafts as fallible editorial material and `docs/specs/` as authority. Correct clear mistakes in place. Stop before implementation.

## Establish the review set

* Read `AGENTS.md`, `docs/specs/contracts/README.md`, and `docs/specs/contracts/_template.yaml`.
* Run `git rev-parse HEAD`, `git status --short`, and `uv run --locked python scripts/contracts.py catalog`.
* Select changed and untracked contract YAML files under `docs/specs/contracts/`; exclude `_template.yaml` and unrelated unchanged contracts.
* Run `uv run --locked python scripts/contracts.py status <selected-contracts>`.
* Preserve unrelated working-tree changes.

Stop if there is no changed draft contract to review.

## Review contracts first

Review one contract completely before moving to the next.

For each contract:

1. Read the complete record and every cited heading-scoped specification section.
2. Verify each rule's source support, normative strength, reusable scope, qualifications, exactness, and uniqueness within the record.
3. Treat `basis: derived` as requiring a necessary inference, not a plausible interpretation.
4. Perform bounded independent discovery using the subject's title, ID, aliases, public symbols, and principal semantic terms. Check relevant cross-cutting facets and read the complete sections behind useful results. Stop when further searches only repeat accounted-for material.
5. Use deeper falsification only for derived, unusually broad, or high-consequence claims. Give particular scrutiny to claims of exclusivity, completeness, canonicality, uniqueness, determinism, independence, or exhaustiveness. Do not construct adversarial examples for every rule.
6. Edit immediately: remove unsupported claims, narrow overstatements, restore qualifications, add clearly omitted requirements, correct sources, and preserve unspecified implementation freedom.
7. Remove task sequencing, implementation staging, file scope, and bead-specific questions from contracts.
8. Rename, merge, split, or delete contracts when their reusable subject boundaries are wrong.
9. Retain `open_questions` only for unresolved product behavior or required public capability.
10. For each verification rule, confirm that it states a distinct verification obligation rather than restating product semantics, and that each cited source contributes authority appropriate to its role.

Do not promote a contract until the individual and cross-contract passes are complete.

## Review contract seams

After the individual passes, compare the changed contracts for duplicated ownership, inconsistent terminology, contradictions, and boundary gaps. Inspect only the adjacent existing contracts needed to resolve those seams. Revise until each fact has one coherent owner.

Do not broaden this into a catalogue-wide audit or a formal completeness proof of the specification corpus.

## Promote sound contracts

Promote contracts independently when they have no genuine unresolved product question and the defined review has passed.

For each promoted contract:

* update its source baseline to the reviewed working-tree state;
* run `uv run --locked python scripts/contracts.py fingerprint <contract>`;
* record every current source hash;
* set `status: reviewed`;
* run `uv run --locked python scripts/contracts.py status <contract>` and require every cited source to report `unchanged`.

Leave unresolved contracts as `draft`. A reviewed hash records independently checked evidence; it is not a completeness claim.

## Review the derived bead last

Only after the contracts are settled, locate the open bead that explicitly cites the changed contract IDs or paths. Do not require the user to provide its ID.

Treat the bead as a bounded implementation slice derived from the contracts, not as contract authority.

Preserve the bead's identity, decomposition, and intended scope. A bead may deliberately implement only the contract facets needed for its stated objective. Do not broaden it to complete an entire reusable contract.

Correct unsupported requirements, omissions within the stated scope, contradictions with the contracts, false blockers, inaccurate exclusions, verification requirements, dependencies, and readiness.

Do not create, split, or replace beads. Do not add new implementation responsibilities merely because they appear in a governing contract.

Check that every required cross-contract effect within the bead's scope has an owner. Do not approve a bead that requires behavior which all of its governing contracts or dependencies exclude.

When the existing task cannot be made coherent without changing its fundamental scope or decomposition, leave it unready and report the exact replanning need.

## Editorial authority

Be skeptical without being hostile. Do not presume correctness, but do not search for hypothetical defects without a concrete semantic reason. Apply clear corrections instead of returning a findings-only report.

Escalate only when:

* the choice affects observable product behavior or a required public capability;
* the authoritative specifications do not determine it;
* multiple materially different answers remain compatible with them; and
* choosing among them would introduce new normative policy.

A difficult implementation choice, an imprecise task artifact, or uncertainty created by a bad draft is not by itself a product question.

Never use a contract to prove itself, infer product truth from a bead, invent design policy, strengthen recommendations into requirements, weaken mandatory requirements, or approve an unresolved assumption.

Do not read, cite, edit, or discuss broader planning documents.

## Finish

* Run `uv run --locked python scripts/contracts.py catalog`.
* Run `uv run --locked python scripts/contracts.py status` for every changed contract.
* Run `git diff --check`.
* Follow `AGENTS.md` for review-role bead flushing and planning-pass commit behavior.
* Do not push or implement code.
* Report contracts corrected and promoted, beads corrected or split, unresolved product questions, and validation performed.
