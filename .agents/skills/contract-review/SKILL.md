---
name: review-contracts
description: Independently audit and correct selected draft specification contracts against authoritative repository specifications, promote bounded sound records as accepted reusable understanding, and then review any directly derived bead. Use after contract-task-prep, for explicitly selected committed drafts, or for draft contracts discovered during implementation. Do not implement code or provide findings-only commentary.
---

# Review specification contracts

Turn selected draft contracts into source-grounded reviewed shared understanding.
Contracts exist so later agents do not repeatedly reconcile the full
specification corpus. Treat drafts as fallible editorial material,
`docs/specs/` as authority, and unchanged reviewed contracts as settled. Correct
clear mistakes in place and stop before implementation.

## Establish the review set

* Read `AGENTS.md`, `docs/specs/contracts/README.md`, and `docs/specs/contracts/_template.yaml`.
* Run `git rev-parse HEAD`, `git status --short`, and `uv run --locked python scripts/contracts.py catalog`.
* Select explicitly requested draft contracts even when already committed. When
  none are named, select changed and untracked draft YAML files under
  `docs/specs/contracts/`. Exclude `_template.yaml` and unrelated contracts.
* Run `uv run --locked python scripts/contracts.py status <selected-contracts>`.
* Preserve unrelated working-tree changes.

Stop if there is no selected draft contract to review.

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
9. Retain `open_questions` only for unresolved observable product behavior or
   required public capability inside `scope.includes`. Move specific adjacent or
   future unrepresented facets to `known_uncovered`.
10. For each verification rule, confirm that it states a distinct verification obligation rather than restating product semantics, and that each cited source contributes authority appropriate to its role.

Do not promote a contract until the individual and cross-contract passes are
complete. Do not use review to pursue optional polish after the stopping
condition below is met.

## Review contract seams

After the individual passes, compare the selected contracts for duplicated
ownership, inconsistent terminology, contradictions, and boundary gaps. Inspect
only the applicable rules of adjacent reviewed contracts needed to resolve a
concrete seam; do not re-audit unchanged reviewed records. Revise until each
represented fact has one coherent owner.

Do not broaden this into a catalogue-wide audit or a formal completeness proof of the specification corpus.

## Promote sound contracts

Promote a contract when its represented boundary is clear, each stated rule is
supported at the reviewed evidence baseline, applicable represented rules are
coherent, no unresolved observable behavior remains inside that boundary, and
implementation freedom is preserved.

A contract may be promoted with `known_uncovered` facets. Do not withhold
promotion merely because adjacent, future, or out-of-scope behavior remains
unspecified. Withhold promotion only when a represented requirement is
unsupported or unresolved observable behavior remains inside the contract's
stated scope.

Promotion does not require exhaustive subject coverage, ideal decomposition,
every useful example or citation, future variants or contexts, private helper
design, or proof that no later task will extend the record. Optional improvement
is not a reason to keep a sound contract draft. Review exists to establish a
trustworthy reusable baseline; once established, reuse is the default.

For each promoted contract:

* update its source baseline to the reviewed working-tree state;
* run `uv run --locked python scripts/contracts.py fingerprint <contract>`;
* record every current source hash;
* set `status: reviewed`;
* run `uv run --locked python scripts/contracts.py status <contract>` and require every cited source to report `unchanged`.

Leave a contract as `draft` only for an unsupported represented requirement or
unresolved observable behavior inside its stated represented scope. A reviewed
hash records independently checked evidence; it is not a completeness claim.

Reopen a reviewed contract only for a changed cited source, a facet required by
the current task but outside represented scope, a concrete contradiction with
authority, a represented ambiguity materially affecting the current bead, or a
concrete contradiction with another applicable contract. Generalized suspicion,
possible additional citations, and editorial preference are not triggers.

## Review the derived bead last

Only after the contracts are settled, locate the open or deferred bead that
explicitly cites the selected contract IDs or paths. Do not require the user to
provide its ID.

Treat the bead as a bounded implementation slice derived from the contracts, not as contract authority.

Preserve the bead's identity, decomposition, and intended scope. A bead may deliberately implement only the contract facets needed for its stated objective. Do not broaden it to complete an entire reusable contract.

Correct unsupported requirements, omissions within the stated scope, contradictions with the contracts, false blockers, inaccurate exclusions, verification requirements, dependencies, and readiness.

Treat `known_uncovered` as non-blocking unless the bead explicitly depends on
that facet. Do not reopen settled contract rules merely to review the bead.

Do not create, split, or replace beads. Do not add new implementation responsibilities merely because they appear in a governing contract.

Check that every required cross-contract effect within the bead's scope has an owner. Do not approve a bead that requires behavior which all of its governing contracts or dependencies exclude.

Before leaving a bead or contract blocked, identify:

```text
represented contract requirement
current bead obligation
materially different observable outcomes
authoritative specification ambiguity
why implementation freedom cannot resolve it
```

If any item is missing, classify the matter as `known_uncovered`, implementation
freedom, adjacent future work, optional improvement, or outside the current slice
and do not withhold promotion or readiness.

When the existing task cannot be made coherent without changing its fundamental
scope or decomposition and the blocker burden is satisfied, leave it unready and
report the exact replanning need.

## Editorial authority

Be skeptical without being hostile. Do not presume correctness, but do not search for hypothetical defects without a concrete semantic reason. Apply clear corrections instead of returning a findings-only report.

Escalate only when:

* the choice affects observable product behavior or a required public capability;
* the authoritative specifications do not determine it;
* multiple materially different answers remain compatible with them; and
* choosing among them would introduce new normative policy.

A difficult implementation choice, an imprecise task artifact, or uncertainty created by a bad draft is not by itself a product question.

When implementation has already produced code and draft contract changes,
preserve the code as unrelated working-tree work. Settle the contracts and any
material bead correction first. The implementer may have recorded
specification-backed reusable facts, but may not independently approve them or
use them to expand product policy.

Never use a contract to prove itself, infer product truth from a bead, invent design policy, strengthen recommendations into requirements, weaken mandatory requirements, or approve an unresolved assumption.

Do not read, cite, edit, or discuss broader planning documents.

## Finish

* Run `uv run --locked python scripts/contracts.py catalog`.
* Run `uv run --locked python scripts/contracts.py status` for every changed contract.
* Run `git diff --check`.
* Follow `AGENTS.md` for review-role bead flushing and planning-pass commit behavior.
* Do not push or implement code.
* Report contracts corrected and promoted, any bead correction, genuine
  unresolved represented questions, and validation performed.
