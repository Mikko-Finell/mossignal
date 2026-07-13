## docs/specs/bead_review_implementation_commit_process.md
- ``mossignal` Bead, Review, Implementation, and Commit Process` [1-1541]
  Preview: **Status:** Process specification, version 3 **Defines:** The repository’s bead lifecycle, four agent roles, review boundaries, implementation handoffs, working-tree discipline, quality gates, bead synchronization, and commit semantics **Does not define:** The detailed contents of individual agent skills, the complete contents of `AGENTS.md`, implementation architecture, product semantics, branch-based collaboration, pull-request policy, release management, or CI infrastructure details This specification defines how implementation work is selected, described, reviewed, executed, verified, accepted, and committed in the `mossignal` repository.
  Symbols: `Pending Approval`, `Ready`, `HEAD`, `main`, `Blocked — Design Required`, `Blocked — Rescoping Required`, `br sync --flush-only`, `AGENTS.md`, `workflow:implementation-review`, `Implementation Review`, `mossignal`, `docs/specs/`, `docs/`, `Draft`, `N/A`, `in_progress`, `workflow:pending-approval`, `Closed`
  Normative: MUST NOT 36, MUST 36, SHOULD NOT 4, SHOULD 23, MAY 7

- ``mossignal` Bead, Review, Implementation, and Commit Process > 1. Purpose` [9-57]
  Preview: This specification defines how implementation work is selected, described, reviewed, executed, verified, accepted, and committed in the `mossignal` repository.
  Symbols: `mossignal`, `main`

- ``mossignal` Bead, Review, Implementation, and Commit Process > 2. Normative language` [58-81]
  Preview: The terms **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.
  Symbols: `Pending Approval`, `Ready`
  Normative: MUST NOT 1, MUST 1, SHOULD NOT 1, SHOULD 1, MAY 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 3. Process philosophy` [82-149]
  Preview: The process MUST preserve the information needed across real handoff boundaries: It MUST NOT introduce additional identity systems or duplicated records without a demonstrated need.
  Symbols: `Pending Approval`, `Ready`
  Normative: MUST NOT 5, MUST 7, SHOULD 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 3. Process philosophy > 3.1 Rigor without ceremonial bookkeeping` [84-106]
  Preview: The process MUST preserve the information needed across real handoff boundaries: It MUST NOT introduce additional identity systems or duplicated records without a demonstrated need.
  Normative: MUST NOT 2, MUST 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 3. Process philosophy > 3.2 Direct correction instead of comment relays` [107-120]
  Preview: Reviewers are expected to be capable editors.
  Symbols: `Pending Approval`
  Normative: MUST NOT 1, MUST 4, SHOULD 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 3. Process philosophy > 3.3 Explicit stopping conditions` [121-141]
  Preview: Review MUST converge.
  Symbols: `Ready`, `Pending Approval`
  Normative: MUST NOT 1, MUST 2

- ``mossignal` Bead, Review, Implementation, and Commit Process > 3. Process philosophy > 3.4 Human design remains authoritative` [142-149]
  Preview: Agents MUST NOT resolve genuine architecture or semantic ambiguity by invention.
  Normative: MUST NOT 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 4. Repository operating model` [150-238]
  Preview: All ordinary work happens directly on `main`.
  Symbols: `HEAD`, `main`
  Normative: MUST NOT 3, MUST 1, SHOULD 1, MAY 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 4. Repository operating model > 4.1 Work happens directly on `main`` [152-166]
  Preview: All ordinary work happens directly on `main`.
  Symbols: `main`

- ``mossignal` Bead, Review, Implementation, and Commit Process > 4. Repository operating model > 4.2 Sequential code-changing agents` [167-181]
  Preview: Code-changing agents SHOULD operate sequentially.
  Normative: SHOULD 1, MAY 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 4. Repository operating model > 4.3 Baseline and working tree` [182-198]
  Preview: At the start of implementation: The complete implementation-review boundary is therefore the repository difference from `HEAD`, including: - staged changes; - unstaged changes; - relevant untracked files.
  Symbols: `HEAD`
  Normative: MUST NOT 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 4. Repository operating model > 4.4 Working-tree inspection` [199-221]
  Preview: Before modifying or reviewing code, an agent MUST inspect at least: An unexpected dirty tree MUST NOT be discarded, overwritten, or assumed to belong to the active bead.
  Normative: MUST NOT 1, MUST 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 4. Repository operating model > 4.5 Destructive commands` [222-238]
  Preview: Agents MUST NOT use destructive cleanup commands against unexplained work.
  Normative: MUST NOT 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 5. Governing artifacts and traceability` [239-321]
  Preview: Normative mossignal specifications live under `docs/specs/` unless `AGENTS.md` or a specification establishes a more specific rule.
  Symbols: `Ready`, `docs/specs/`, `AGENTS.md`, `docs/`, `Pending Approval`
  Normative: MUST NOT 1, MUST 2, SHOULD NOT 1, SHOULD 5, MAY 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 5. Governing artifacts and traceability > 5.1 Authoritative specifications` [241-255]
  Preview: Normative mossignal specifications live under `docs/specs/` unless `AGENTS.md` or a specification establishes a more specific rule.
  Symbols: `docs/specs/`, `AGENTS.md`, `docs/`
  Normative: MUST 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 5. Governing artifacts and traceability > 5.2 The bead as implementation contract` [256-277]
  Preview: A bead selects and operationalizes a bounded subset of the authoritative requirements.
  Symbols: `Ready`
  Normative: SHOULD NOT 1, SHOULD 3

- ``mossignal` Bead, Review, Implementation, and Commit Process > 5. Governing artifacts and traceability > 5.3 Local obligation identifiers` [278-293]
  Preview: A bead MAY assign local obligation identifiers such as: These identifiers are scoped to the bead.
  Normative: SHOULD 1, MAY 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 5. Governing artifacts and traceability > 5.4 Clean bead-approval record` [294-303]
  Preview: Ordinary bead review MUST NOT retain a finding ledger or review-pass chronology in the current bead contract.
  Symbols: `Ready`, `Pending Approval`
  Normative: MUST NOT 1, SHOULD 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 5. Governing artifacts and traceability > 5.5 One evolving evidence record` [304-321]
  Preview: The bead should maintain one evolving obligation and evidence record rather than separate duplicated compliance matrices for every agent role.
  Normative: MUST 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 6. Bead lifecycle` [322-448]
  Preview: The exact command-line state labels may vary, but the repository workflow MUST distinguish the following semantic states.
  Symbols: `Pending Approval`, `Ready`, `Draft`, `workflow:implementation-review`
  Normative: MUST NOT 3, MUST 2, SHOULD 1, MAY 3

- ``mossignal` Bead, Review, Implementation, and Commit Process > 6. Bead lifecycle > 6.1 `Pending Approval`` [349-361]
  Preview: The current bead is believed to be a complete implementation contract and has no known blocker, but its current material contents have not yet passed a fresh independent approval review.
  Symbols: `Pending Approval`
  Normative: MUST NOT 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 6. Bead lifecycle > 6.2 `Bead Review`` [362-369]
  Preview: A fresh bead reviewer is auditing and editing the bead.
  Normative: MUST NOT 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 6. Bead lifecycle > 6.3 `Ready`` [370-396]
  Preview: The complete current material version has passed a fresh independent review without requiring a material correction during that pass.
  Symbols: `Ready`
  Normative: MUST NOT 1, MUST 1, MAY 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 6. Bead lifecycle > 6.4 `Blocked — Design Required`` [397-402]
  Preview: The task cannot be implemented safely because authoritative design is missing, contradictory, or genuinely undecided.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 6. Bead lifecycle > 6.5 `Blocked — Rescoping Required`` [403-412]
  Preview: The intended work cannot be represented as one coherent implementation unit without a human decision about task boundaries or priorities.
  Symbols: `Pending Approval`, `Ready`
  Normative: SHOULD 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 6. Bead lifecycle > 6.6 `Implementing`` [413-418]
  Preview: An implementer is modifying the uncommitted working tree to satisfy the approved bead.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 6. Bead lifecycle > 6.7 `Implementation Review`` [419-438]
  Preview: The complete uncommitted implementation is pending fresh independent approval.
  Symbols: `Pending Approval`, `workflow:implementation-review`

- ``mossignal` Bead, Review, Implementation, and Commit Process > 6. Bead lifecycle > 6.8 `Closed`` [439-448]
  Preview: The bead has passed a fresh clean implementation-review pass, all mandatory gates, and final acceptance.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 7. Bead authoring` [449-520]
  Preview: The bead author MUST: - identify one bounded implementation objective; - record the repository commit used as the specification baseline; - cite directly governing specification material by path, stable section heading or anchor, and governing subject; - identify relevant adjacent constraints; - inspect relevant current code, tests, call sites, dependencies, and overlapping open, active, blocked, and closed beads; - derive a manageable set of implementation obligations; - state important forbidden outcomes; - define scope and non-goals; - specify the expected evidence; - identify dependencies and known blockers; - create the strongest complete implementation contract it can; - leave the completed material version in `Pending Approval` with no known blocker.
  Symbols: `Pending Approval`, `Blocked — Design Required`, `Blocked — Rescoping Required`
  Normative: MUST NOT 1, MUST 2, SHOULD 3

- ``mossignal` Bead, Review, Implementation, and Commit Process > 7. Bead authoring > 7.1 Author responsibilities` [451-471]
  Preview: The bead author MUST: - identify one bounded implementation objective; - record the repository commit used as the specification baseline; - cite directly governing specification material by path, stable section heading or anchor, and governing subject; - identify relevant adjacent constraints; - inspect relevant current code, tests, call sites, dependencies, and overlapping open, active, blocked, and closed beads; - derive a manageable set of implementation obligations; - state important forbidden outcomes; - define scope and non-goals; - specify the expected evidence; - identify dependencies and known blockers; - create the strongest complete implementation contract it can; - leave the completed material version in `Pending Approval` with no known blocker.
  Symbols: `Pending Approval`
  Normative: MUST NOT 1, MUST 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 7. Bead authoring > 7.2 Pending-approval record` [472-490]
  Preview: A completed authored bead should contain an explicit approval section broadly equivalent to: The exact phrasing may differ.
  Symbols: `Pending Approval`
  Normative: MUST 1, SHOULD 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 7. Bead authoring > 7.3 Architectural closure` [491-506]
  Preview: A bead is not authorable as an ordinary blocker-free `Pending Approval` implementation contract when implementation would require the implementer to decide matters such as: - whether a new stable identity category exists; - whether a value is semantic or derived; - whether persistence compatibility is required; - whether state should be preserved or reset; - whether a public API should be broadened; - how contradictory specifications should be reconciled; - what failure semantics should apply.
  Symbols: `Pending Approval`, `Blocked — Design Required`, `Blocked — Rescoping Required`
  Normative: SHOULD 2

- ``mossignal` Bead, Review, Implementation, and Commit Process > 7. Bead authoring > 7.4 Appropriate level of detail` [507-520]
  Preview: A bead should be precise enough that compliance can be determined objectively, but it should not become line-by-line pseudocode.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 8. Bead review` [521-659]
  Preview: The ordinary target is a bead in `Pending Approval`.
  Symbols: `Pending Approval`, `Ready`, `N/A`, `Blocked — Design Required`, `Blocked — Rescoping Required`, `br sync --flush-only`
  Normative: MUST NOT 4, MUST 7, SHOULD NOT 1, MAY 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 8. Bead review > 8.1 Independent auditing editor` [523-544]
  Preview: The ordinary target is a bead in `Pending Approval`.
  Symbols: `Pending Approval`, `Ready`
  Normative: MUST NOT 2, MUST 2

- ``mossignal` Bead, Review, Implementation, and Commit Process > 8. Bead review > 8.2 Review questions` [545-560]
  Preview: The reviewer should determine: - Does every material obligation have authoritative support?
  Symbols: `N/A`

- ``mossignal` Bead, Review, Implementation, and Commit Process > 8. Bead review > 8.3 Material and nonmaterial corrections` [561-581]
  Preview: A correction is material when it changes the implementation contract, including: - objective; - normative obligations; - forbidden outcomes; - scope or non-goals; - authoritative interpretation; - expected evidence or mandatory verification; - dependencies; - concern or risk flags; - completion criteria; - public, persistence, migration, failure, identity, or compatibility requirements.
  Normative: MUST 2

- ``mossignal` Bead, Review, Implementation, and Commit Process > 8. Bead review > 8.4 Blocking versus correctable issues` [582-597]
  Preview: A bead-review issue is blocking only when it identifies at least one of: - a missing or distorted governing requirement; - an unresolved architecture or semantic decision; - a contradiction; - an impossible acceptance criterion; - materially incomplete verification; - unsafe or incoherent scope; - a concrete route by which an implementation could violate the specifications.
  Normative: SHOULD NOT 1, MAY 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 8. Bead review > 8.5 Review outcomes` [598-617]
  Preview: After completing all available corrections, rereading the result, and removing obsolete approval or blocker text, the reviewer MUST choose one terminal outcome for the pass: - `Ready`: no unresolved blocker and no material correction was required during this fresh pass.
  Symbols: `Ready`, `Pending Approval`, `Blocked — Design Required`, `Blocked — Rescoping Required`
  Normative: MUST 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 8. Bead review > 8.6 Clean-review approval rule` [618-633]
  Preview: A bead becomes `Ready` only after a fresh independent reviewer audits the complete current material version, finds no unresolved blocker, and does not need to make a material correction.
  Symbols: `Ready`
  Normative: MUST NOT 2

- ``mossignal` Bead, Review, Implementation, and Commit Process > 8. Bead review > 8.7 Planning commit` [634-659]
  Preview: Every completed bead-review pass MUST create a durable planning commit, whether its terminal result is `Ready`, materially revised `Pending Approval`, or a blocked state.
  Symbols: `Pending Approval`, `Ready`, `br sync --flush-only`
  Normative: MUST 2

- ``mossignal` Bead, Review, Implementation, and Commit Process > 9. Approved-contract stability` [660-691]
  Preview: Once a bead enters `Ready`, the following portions are normative and SHOULD be treated as locked: Implementers and implementation reviewers may update: If implementation reveals that the approved contract is materially wrong or incomplete, the bead MUST return to `Pending Approval` or the appropriate blocked state before fresh bead review.
  Symbols: `Ready`, `Pending Approval`
  Normative: MUST NOT 1, MUST 1, SHOULD 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 10. Implementation` [692-751]
  Preview: An implementation agent may select only a bead whose `Ready` planning state has been flushed and committed.
  Symbols: `HEAD`, `Pending Approval`, `Ready`, `main`, `Implementation Review`
  Normative: MUST NOT 2, MUST 1, SHOULD 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 10. Implementation > 10.1 Selecting work` [694-707]
  Preview: An implementation agent may select only a bead whose `Ready` planning state has been flushed and committed.
  Symbols: `Ready`, `HEAD`
  Normative: MUST 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 10. Implementation > 10.2 Uncommitted implementation` [708-715]
  Preview: The implementer modifies the working tree directly on `main`.
  Symbols: `main`, `HEAD`
  Normative: MUST NOT 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 10. Implementation > 10.3 Implementation obligations` [716-731]
  Preview: Before handoff, the implementer SHOULD: - read the complete approved bead; - read the cited specification sections with sufficient surrounding context; - inspect relevant existing types and call sites; - derive the required tests; - implement the obligations at appropriate enforcement layers; - add or update tests; - update provisional implementation evidence; - run focused iteration checks; - leave the complete work uncommitted; - record implementation approval `Pending Approval`; - move the bead to `Implementation Review` without flushing or committing.
  Symbols: `Pending Approval`, `Implementation Review`
  Normative: SHOULD 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 10. Implementation > 10.4 No architectural invention` [732-737]
  Preview: If a required representation or behavior is not determined by the bead and specifications, the implementer must stop that portion and return the bead to `Pending Approval` or the appropriate blocked state.
  Symbols: `Pending Approval`
  Normative: MUST NOT 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 10. Implementation > 10.5 Handoff state` [738-751]
  Preview: At implementation handoff: The implementer’s reported command results are useful evidence but are not substitutes for the fresh approving implementation reviewer rerunning the required gates.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 11. Implementation review` [752-878]
  Preview: Implementation review examines: The reviewer MUST inspect more than the textual diff where the surrounding design or invariant requires it.
  Symbols: `Pending Approval`, `HEAD`, `in_progress`, `workflow:implementation-review`, `workflow:pending-approval`, `Blocked — Design Required`, `Blocked — Rescoping Required`
  Normative: MUST NOT 6, MUST 4, SHOULD 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 11. Implementation review > 11.1 Review object` [754-770]
  Preview: Implementation review examines: The reviewer MUST inspect more than the textual diff where the surrounding design or invariant requires it.
  Normative: MUST 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 11. Implementation review > 11.2 Direct correction` [771-785]
  Preview: The implementation reviewer SHOULD directly: - fix implementation defects; - strengthen tests; - correct incomplete evidence; - remove forbidden alternatives; - improve enforcement; - rerun relevant checks.
  Normative: MUST NOT 2, SHOULD 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 11. Implementation review > 11.3 Three-way comparison` [786-801]
  Preview: Implementation review is a three-way comparison: The reviewer must not assume that prior bead approval proves the bead perfect.
  Symbols: `Pending Approval`

- ``mossignal` Bead, Review, Implementation, and Commit Process > 11. Implementation review > 11.4 Review focus` [802-815]
  Preview: The reviewer should verify: - every bead obligation has real implementation coverage; - the implementation covers every relevant code path; - the enforcement layer is appropriate; - tests can fail when the requirement is violated; - negative constraints are actually enforced; - adjacent contracts remain valid; - Rust types, visibility, ownership, trait bounds, and conversions are coherent; - public and persisted shapes remain compatible where required; - no unapproved architecture has been introduced.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 11. Implementation review > 11.5 Material and nonmaterial implementation corrections` [816-825]
  Preview: A correction is material when it changes implementation behavior, enforcement, verification strength, architectural meaning, or the truth of the compliance claim.
  Normative: MUST 2

- ``mossignal` Bead, Review, Implementation, and Commit Process > 11. Implementation review > 11.6 Review outcomes` [826-863]
  Preview: Every implementation-review pass is complete and ends in one of four outcomes: If the reviewer makes any material correction and no blocker remains, it MUST leave the complete corrected implementation uncommitted with: The bead remains status `in_progress` with `workflow:implementation-review`.
  Symbols: `HEAD`, `in_progress`, `workflow:implementation-review`, `Pending Approval`, `workflow:pending-approval`, `Blocked — Design Required`, `Blocked — Rescoping Required`
  Normative: MUST NOT 3, MUST 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 11. Implementation review > 11.6 Review outcomes > Implementation Pending Approval` [837-849]
  Preview: If the reviewer makes any material correction and no blocker remains, it MUST leave the complete corrected implementation uncommitted with: The bead remains status `in_progress` with `workflow:implementation-review`.
  Symbols: `in_progress`, `workflow:implementation-review`
  Normative: MUST NOT 1, MUST 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 11. Implementation review > 11.6 Review outcomes > Approved and committed` [850-853]
  Preview: If a fresh reviewer completes the full adversarial pass without a material correction and finds no blocker, it may perform final acceptance directly under Section 12.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 11. Implementation review > 11.6 Review outcomes > Returned to bead approval` [854-857]
  Preview: If the approved normative contract is materially wrong or incomplete but existing authoritative material appears sufficient to correct it, the reviewer MUST NOT revise that contract.
  Symbols: `HEAD`, `Pending Approval`, `workflow:pending-approval`
  Normative: MUST NOT 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 11. Implementation review > 11.6 Review outcomes > Blocked` [858-863]
  Preview: If a required semantic or architectural answer is genuinely missing or contradictory, the reviewer safely restores accepted `HEAD` and records `Blocked — Design Required`.
  Symbols: `HEAD`, `Blocked — Design Required`, `Blocked — Rescoping Required`
  Normative: MUST NOT 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 11. Implementation review > 11.7 Independence and stopping rule` [864-878]
  Preview: A reviewer directly fixes every correctable in-scope defect.
  Symbols: `Pending Approval`
  Normative: MUST NOT 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 12. Clean implementation approval and acceptance` [879-933]
  Preview: The approving implementation reviewer MUST be fresh with respect to the current material implementation version.
  Symbols: `HEAD`, `Pending Approval`, `br sync --flush-only`
  Normative: MUST NOT 3, MUST 3, SHOULD 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 12. Clean implementation approval and acceptance > 12.1 Clean-pass requirement` [881-886]
  Preview: The approving implementation reviewer MUST be fresh with respect to the current material implementation version.
  Symbols: `HEAD`, `Pending Approval`
  Normative: MUST NOT 1, MUST 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 12. Clean implementation approval and acceptance > 12.2 Mandatory verification` [887-897]
  Preview: The approving reviewer MUST independently rerun against the exact tree being accepted: - the canonical full repository quality gate; - every mandatory bead-specific command; - relevant regression tests; - required compatibility, differential, persistence, migration, or failure-atomicity checks activated by the bead.
  Normative: MUST 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 12. Clean implementation approval and acceptance > 12.3 Approval record` [898-913]
  Preview: Before closure, the bead SHOULD contain a clean record broadly equivalent to: It MUST NOT retain resolved findings, correction chronology, or advisories.
  Normative: MUST NOT 1, SHOULD 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 12. Clean implementation approval and acceptance > 12.4 Acceptance responsibilities` [914-933]
  Preview: The approving implementation reviewer MUST: 1.
  Symbols: `HEAD`, `br sync --flush-only`
  Normative: MUST NOT 1, MUST 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 13. Bead closure and synchronization` [934-973]
  Preview: The bead MUST be closed before the accepted implementation commit is created.
  Symbols: `br sync --flush-only`
  Normative: MUST NOT 2, MUST 3

- ``mossignal` Bead, Review, Implementation, and Commit Process > 13. Bead closure and synchronization > 13.1 Closure occurs before commit` [936-947]
  Preview: The bead MUST be closed before the accepted implementation commit is created.
  Normative: MUST 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 13. Bead closure and synchronization > 13.2 Bead flush` [948-959]
  Preview: After closing the bead, the approving implementation reviewer MUST run: This exports the current bead changes to the repository’s JSONL representation.
  Normative: MUST 2

- ``mossignal` Bead, Review, Implementation, and Commit Process > 13. Bead closure and synchronization > 13.3 Flush failure` [960-965]
  Preview: If bead closure or `br sync --flush-only` fails, the accepted commit MUST NOT be created.
  Symbols: `br sync --flush-only`
  Normative: MUST NOT 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 13. Bead closure and synchronization > 13.4 No post-commit closure commit` [966-973]
  Preview: The normal process MUST NOT create one implementation commit followed by another commit solely to close or flush the bead.
  Normative: MUST NOT 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 14. Commit semantics` [974-1102]
  Preview: A commit records one completed and accepted repository-state transition.
  Normative: MUST NOT 2, MUST 1, SHOULD NOT 1, SHOULD 2

- ``mossignal` Bead, Review, Implementation, and Commit Process > 14. Commit semantics > 14.1 Meaning of a commit` [976-1008]
  Preview: A commit records one completed and accepted repository-state transition.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 14. Commit semantics > 14.1 Meaning of a commit > Planning commit` [980-992]
  Preview: A planning commit records one completed independent bead-review pass whose result is: It contains the clean synchronized planning record and references the bead ID.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 14. Commit semantics > 14.1 Meaning of a commit > Accepted implementation commit` [993-1008]
  Preview: An accepted implementation commit means: Both classes are accepted repository checkpoints, not ordinary intermediate agent saves.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 14. Commit semantics > 14.2 One bead per commit` [1009-1018]
  Preview: An accepted planning or implementation commit SHOULD correspond to one bead.
  Normative: MUST NOT 1, SHOULD 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 14. Commit semantics > 14.3 Commit-to-bead linkage` [1019-1041]
  Preview: Every planning, implementation, test, implementation-facing documentation, review-fix, or corrective commit MUST reference the relevant bead ID.
  Normative: MUST 1, SHOULD 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 14. Commit semantics > 14.4 Subject quality` [1042-1059]
  Preview: The subject should describe the accepted semantic or architectural outcome.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 14. Commit semantics > 14.5 Commit body` [1060-1088]
  Preview: A commit body is RECOMMENDED for nontrivial beads.
  Normative: SHOULD NOT 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 14. Commit semantics > 14.6 No self-referential commit hash in the bead` [1089-1102]
  Preview: The bead MUST NOT require its final commit hash as part of its own pre-commit closed state.
  Normative: MUST NOT 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 15. Automated quality gates` [1103-1255]
  Preview: Automated checks establish mechanical and executable quality.
  Symbols: `main`, `HEAD`
  Normative: MUST NOT 1, MUST 1, SHOULD 4

- ``mossignal` Bead, Review, Implementation, and Commit Process > 15. Automated quality gates > 15.1 Role of automation` [1105-1122]
  Preview: Automated checks establish mechanical and executable quality.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 15. Automated quality gates > 15.2 Canonical repository gate` [1123-1154]
  Preview: The repository SHOULD define one version-controlled canonical command for ordinary final verification, for example: or: The exact name is a repository choice.
  Normative: SHOULD 2

- ``mossignal` Bead, Review, Implementation, and Commit Process > 15. Automated quality gates > 15.3 Bead-specific verification` [1155-1174]
  Preview: A bead may require additional mandatory checks based on its obligations and concern flags.
  Normative: SHOULD 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 15. Automated quality gates > 15.4 Verification levels` [1175-1217]
  Preview: The repository should distinguish three levels of automated checking.
  Normative: MUST 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 15. Automated quality gates > 15.4 Verification levels > Iteration checks` [1179-1190]
  Preview: Fast checks used during implementation: Iteration checks provide feedback but do not constitute final acceptance.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 15. Automated quality gates > 15.4 Verification levels > Mandatory commit gate` [1191-1200]
  Preview: Checks that MUST pass against the exact final working tree before commitment:
  Normative: MUST 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 15. Automated quality gates > 15.4 Verification levels > Extended verification` [1201-1217]
  Preview: Expensive checks that may run periodically, in CI, or for specifically affected beads: An extended check becomes a mandatory bead gate when the bead materially changes the subsystem or risk that the check protects.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 15. Automated quality gates > 15.5 Hard pass/fail semantics` [1218-1230]
  Preview: For a mandatory gate: - failure blocks commitment; - inability to run the command is not a pass; - designated warnings are failures; - a timeout or hang is a failure unless the gate explicitly defines otherwise; - repeatedly rerunning a flaky test until it happens to pass is forbidden; - weakening, deleting, or bypassing a test merely to clear the gate is forbidden; - changes to test oracles receive the same scrutiny as production changes; - environment-dependent behavior must be explained and resolved, not ignored.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 15. Automated quality gates > 15.6 Baseline failures` [1231-1245]
  Preview: `main` should remain green.
  Symbols: `main`, `HEAD`
  Normative: SHOULD 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 15. Automated quality gates > 15.7 Gate changes` [1246-1255]
  Preview: A bead that modifies the quality gate, lint policy, test harness, or reference oracle must treat that infrastructure as part of the reviewed implementation.
  Normative: MUST NOT 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 16. Review-mode distinctions` [1256-1307]
  Preview: The repository process distinguishes two review activities.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 16. Review-mode distinctions > 16.1 Bead review` [1260-1282]
  Preview: **Primary object:** **Purpose:** Complete an adversarial review, directly correct the contract, and determine whether the current material version qualifies for clean independent approval.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 16. Review-mode distinctions > 16.2 Implementation review` [1283-1307]
  Preview: **Primary object:** **Purpose:** Correct and verify work that has not yet passed acceptance controls.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 17. Later-discovered defects` [1308-1333]
  Preview: After the accepted commit, the original bead remains closed as the historical record of what was accepted at that time.
  Normative: MUST NOT 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 17. Later-discovered defects > 17.1 Closed work remains historical` [1310-1315]
  Preview: After the accepted commit, the original bead remains closed as the historical record of what was accepted at that time.
  Normative: MUST NOT 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 17. Later-discovered defects > 17.2 Corrective bead` [1316-1327]
  Preview: When a substantive defect is discovered after acceptance, the ordinary bead-authoring role should create a corrective bead that references: - the original bead; - the relevant accepted commit; - the violated specification clauses; - the observed implementation defect; - the required corrective evidence.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 17. Later-discovered defects > 17.3 Design escalation` [1328-1333]
  Preview: If later investigation exposes a genuine specification ambiguity or contradiction, the bead author must record the exact design blocker rather than inventing a corrective implementation answer.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 18. Role separation` [1334-1393]
  Preview: Agent identities are not permanently assigned.
  Symbols: `Pending Approval`, `Ready`

- ``mossignal` Bead, Review, Implementation, and Commit Process > 18. Role separation > 18.1 Bead author` [1340-1352]
  Preview: May: - author the strongest complete bead it can; - revise it before independent review; - later implement it after approval.
  Symbols: `Pending Approval`

- ``mossignal` Bead, Review, Implementation, and Commit Process > 18. Role separation > 18.2 Bead reviewer` [1353-1364]
  Preview: May: - directly edit the bead; - approve an unchanged or only nonmaterially corrected version as `Ready`; - leave a materially corrected version in `Pending Approval`; - block it for design or rescoping; - flush and commit the completed planning pass.
  Symbols: `Ready`, `Pending Approval`

- ``mossignal` Bead, Review, Implementation, and Commit Process > 18. Role separation > 18.3 Implementer` [1365-1377]
  Preview: May: - implement an approved bead; - add tests and evidence; - run iteration checks.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 18. Role separation > 18.4 Implementation reviewer` [1378-1393]
  Preview: May: - inspect the complete working tree; - directly fix code and tests; - update evidence and implementation approval state; - leave materially corrected work pending fresh approval; - run mandatory gates and complete final acceptance after a fresh clean pass; - close and flush the bead; - create the accepted implementation commit.
  Symbols: `Pending Approval`

- ``mossignal` Bead, Review, Implementation, and Commit Process > 19. Exceptional conditions` [1394-1432]
  Preview: When a task requires a decision not contained in authoritative material: Move the bead to the appropriate blocked state and report the exact missing decision.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 19. Exceptional conditions > 19.1 Unresolved design` [1396-1408]
  Preview: When a task requires a decision not contained in authoritative material: Move the bead to the appropriate blocked state and report the exact missing decision.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 19. Exceptional conditions > 19.2 Unexpected unrelated changes` [1409-1412]
  Preview: If the working tree contains unrelated changes whose ownership cannot be established, implementation or review must stop before modifying or committing them.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 19. Exceptional conditions > 19.3 Partially completed agent work` [1413-1418]
  Preview: An interrupted agent may leave uncommitted work.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 19. Exceptional conditions > 19.4 Mandatory command unavailable` [1419-1424]
  Preview: If a required command cannot run because of missing infrastructure, environment, credentials, platform support, or tool failure, the approving implementation reviewer must not report it as passed.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 19. Exceptional conditions > 19.5 Emergency repository recovery` [1425-1432]
  Preview: Emergency reverts or repository recovery may require exceptions to the ordinary sequence.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 20. Mechanically enforceable rules` [1433-1458]
  Preview: The repository SHOULD automate simple process invariants where practical.
  Symbols: `Pending Approval`, `Ready`, `Closed`
  Normative: SHOULD 1

- ``mossignal` Bead, Review, Implementation, and Commit Process > 21. End-to-end normal procedure` [1459-1512]
  Preview: The ordinary workflow is: 1.
  Symbols: `Pending Approval`, `Ready`, `Blocked — Design Required`, `Blocked — Rescoping Required`, `main`, `Implementation Review`, `HEAD`

- ``mossignal` Bead, Review, Implementation, and Commit Process > 21. End-to-end normal procedure > Phase A — Bead preparation` [1463-1478]
  Preview: 1.
  Symbols: `Pending Approval`, `Ready`, `Blocked — Design Required`, `Blocked — Rescoping Required`

- ``mossignal` Bead, Review, Implementation, and Commit Process > 21. End-to-end normal procedure > Phase B — Implementation` [1479-1487]
  Preview: 10.
  Symbols: `Ready`, `main`, `Implementation Review`, `Pending Approval`

- ``mossignal` Bead, Review, Implementation, and Commit Process > 21. End-to-end normal procedure > Phase C — Implementation review and acceptance` [1488-1512]
  Preview: 16.
  Symbols: `Pending Approval`, `HEAD`

- ``mossignal` Bead, Review, Implementation, and Commit Process > 22. Completion criteria` [1513-1536]
  Preview: An implementation bead is fully complete only when: Implementation completion does not require pursuing optional improvements beyond mandatory compliance.

- ``mossignal` Bead, Review, Implementation, and Commit Process > 23. Summary rule` [1537-1541]
  Preview: The repository’s working rule is: > Work is authored as a complete bead in `Pending Approval`.
  Symbols: `Pending Approval`, `Ready`, `main`
