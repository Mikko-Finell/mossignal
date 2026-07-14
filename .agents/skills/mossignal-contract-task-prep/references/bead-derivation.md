# Deriving the implementation bead

The bead is a bounded implementation plan. Contracts are reusable product truth.

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

## Bead content

Derive the bead from the contracts and include only what the implementation task needs:

- objective;
- contract basis;
- included scope;
- explicit exclusions and non-goals;
- implementation obligations;
- allowed implementation freedom where it prevents invention;
- expected change surface;
- required verification;
- blocking open questions;
- readiness state.

Use contract rule IDs when they improve traceability. Do not copy the entire contract into the bead.

## Consistency rules

- Every normative bead claim must be supported by a referenced contract.
- The bead may narrow task scope but must not weaken the underlying contract.
- The bead may choose among documented implementation freedoms but must not convert the choice into reusable product truth.
- Task-specific file paths, sequencing, and staging belong only in the bead.
- If a contract is partial, rely only on its researched facets.
- If an open question changes observable product behavior and blocks safe implementation, leave the bead unready.

## Handoff

Finish with a concise summary:

- bead created or refined;
- contracts used unchanged;
- contracts created or changed;
- unresolved decisions;
- important exclusions;
- no implementation performed.
