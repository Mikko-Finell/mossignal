# Deriving the implementation bead

The bead is a bounded implementation plan. Contracts are reusable, reviewed
views of authoritative product truth.

## Existing bead

When a bead already exists:

- treat it as provisional;
- compare every claimed requirement and exclusion with the researched contracts;
- correct unsupported or incomplete claims;
- preserve useful task organization that does not conflict with the contracts.

## New bead

If no bead exists, follow the repository's current bead tooling and format. Do not invent a parallel task-storage system.

Create the bead only after the relevant contract research is sufficiently complete.

## Required contract basis

Include a concise contract-basis section naming every governing record, for example:

```text
## Contract basis

- `mossignal.identity.stable_structural_keys`
  - `docs/specs/contracts/stable-structural-keys.yaml`
  - status: draft
  - role: governs public key values, allocation, category safety, and conversions
```

Distinguish:

- contracts changed by the task-preparation work;
- existing contracts used unchanged;
- contextual contracts that constrain scope but are not modified.

Also identify represented contract facets required by the bead and any
`known_uncovered` facets intentionally outside it. Do not make unrelated future
coverage a task blocker.

## Bead content

Derive the bead from the contracts and include only what the implementation task needs:

When a bead derives from a roadmap, identify the roadmap document, item number,
and item title in the bead. Treat this as task provenance and sequencing context,
not product authority. At the same time, mark that item `[IN PROGRESS]` in the
roadmap document by changing its heading from `## <number>. <title>` to
`## <number>. [IN PROGRESS] <title>`.

- objective;
- contract basis;
- included scope;
- explicit exclusions and non-goals;
- implementation obligations;
- allowed implementation freedom where it prevents invention;
- expected change surface;
- required verification;
- genuine blocking questions;
- readiness state.

Use contract rule IDs when they improve traceability. Do not copy the entire contract into the bead.

## Consistency rules

- Every normative bead claim must be supported by a referenced contract.
- The bead may narrow task scope but must not weaken the underlying contract.
- The bead may choose among documented implementation freedoms but must not
  convert the choice into reusable product truth. When several reasonable
  implementations conform, direct implementation to choose the simplest
  deterministic option, test it, and record the concrete decision.
- Task-specific file paths, sequencing, and staging belong only in the bead.
- If a contract is partial, rely only on its represented facets.
- Treat unchanged reviewed facets as settled; do not independently re-audit
  their source support while deriving the bead.
- Leave the bead unready only for conflicting requirements, missing fundamental
  semantics required for correct behavior, or an already-frozen compatibility
  promise that prevents a safe local choice, and only when the blocker burden
  below is satisfied.
- Do not require specification amendments before ordinary implementation
  choices. Review the concrete result first; update specifications afterward
  only if the accepted choice should become a permanent promise.

## Blocker burden

A blocking question must identify:

```text
represented contract requirement
current bead obligation
blocking category: conflicting requirements, missing fundamental semantics,
or an already-frozen compatibility promise
the concrete conflict, missing semantic rule, or frozen promise
why the simplest deterministic conforming choice cannot safely resolve it
```

If any item is absent, classify the matter as `known_uncovered`, implementation
freedom, adjacent future work, optional improvement, or outside the bead and keep
the bounded task moving. Multiple reasonable observable outcomes are not by
themselves blocking.

## Implementation-discovered knowledge

Early foundational implementation may discover reusable specification-backed
facts or settle ordinary open implementation choices. The implementer may
preserve specification-backed facts in new or changed draft contracts. Record
ordinary choices in the implementation, tests, or bead close-out instead; they
do not become contract rules unless an authoritative specification is later
amended. Draft contract changes require independent review before final
implementation acceptance. A material change to the bead returns to planning
review.

## Handoff

Finish with a concise summary:

- bead created or refined;
- contracts used unchanged;
- contracts created or changed;
- unresolved decisions;
- important exclusions;
- no implementation performed.
