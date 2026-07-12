# `mossignal` Bead, Review, Implementation, and Commit Process

**Status:** Process specification, version 3  
**Defines:** The repository’s bead lifecycle, four agent roles, review boundaries, implementation handoffs, working-tree discipline, quality gates, bead synchronization, and commit semantics  
**Does not define:** The detailed contents of individual agent skills, the complete contents of `AGENTS.md`, implementation architecture, product semantics, branch-based collaboration, pull-request policy, release management, or CI infrastructure details

---

## 1. Purpose

This specification defines how implementation work is selected, described, reviewed, executed, verified, accepted, and committed in the `mossignal` repository.

The process is designed for coding agents that may implement explicit requirements competently but must not be trusted to:

- infer missing architecture;
- resolve underspecified semantic questions;
- notice every distant invariant affected by a local change;
- distinguish a locally convenient solution from one permitted by the authoritative specifications;
- maintain review continuity without explicit procedural support.

The process therefore converts implementation work into a bounded sequence of independently reviewed obligations while deliberately avoiding unnecessary project-management ceremony.

The central model is:

```text
authoritative specifications
        ↓
complete bead in Pending Approval
        ↓
complete independent bead-review pass
        ↓
planning commit: Ready, materially revised Pending Approval, or Blocked
        ↓
fresh review repeats when Pending Approval
        ↓
uncommitted implementation on main
        ↓
complete independent implementation-review passes
        ↓
fresh clean implementation approval and mechanical verification
        ↓
bead closure and bead-store flush
        ↓
one accepted implementation commit
```

The process is based on four principles:

1. **Specifications define truth.**
2. **A bead defines one approved, bounded implementation contract derived from that truth.**
3. **The uncommitted working tree represents proposed work that has not yet passed repository quality controls.**
4. **A commit represents one completed and accepted repository-state transition.**

A commit is not merely a convenient checkpoint in an agent’s editing process. A planning commit records a completed bead-review conclusion. An accepted implementation commit means that the implementation is complete, independently reviewed, mechanically verified, closed, synchronized to the repository’s bead record, and accepted into `main`.

---

## 2. Normative language

The terms **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.

A **bead** is the repository’s durable task and compliance record for one bounded unit of work.

A **bead author** creates the strongest complete implementation contract it can and leaves its current material version pending independent approval.

A **bead reviewer** independently audits and directly edits a bead in `Pending Approval`, then completes the pass by committing a clean planning result as `Ready`, materially revised `Pending Approval`, or a blocked state.

A **material change** alters the meaning of the implementation contract, including its objective, normative obligations, forbidden outcomes, scope, non-goals, authoritative interpretation, expected evidence, mandatory verification, dependencies, concern flags, completion criteria, or public, persistence, migration, failure, identity, or compatibility requirements.

A **nonmaterial change** preserves the meaning of the implementation contract, such as spelling, grammar, formatting, harmless wording cleanup, correction of an obvious path or heading typo, layout normalization, removal of obsolete review-history text, or an unambiguous reference correction that does not change the cited requirement.

An **implementer** modifies the repository to satisfy an approved bead.

An **implementation reviewer** independently audits and directly corrects the uncommitted working tree. A reviewer that materially changes the implementation leaves it pending fresh approval. A fresh reviewer that needs no material correction runs the required quality gates, closes and flushes the bead, and creates the accepted implementation commit.

A **blocking issue** is a defect that prevents bead approval or implementation acceptance.

An **advisory** is a possible improvement that does not prevent approval or acceptance.

---

## 3. Process philosophy

### 3.1 Rigor without ceremonial bookkeeping

The process MUST preserve the information needed across real handoff boundaries:

```text
specification → bead
bead author → bead reviewer
bead reviewer → planning commit or fresh bead reviewer
approved bead → implementation
implementer → implementation reviewer
implementation reviewer → fresh implementation reviewer when materially revised
clean implementation reviewer → accepted implementation commit
```

It MUST NOT introduce additional identity systems or duplicated records without a demonstrated need.

In particular:

- the bead ID is the global identity of the work;
- bead-local obligation IDs may be used for traceability;
- code locations, tests, commands, and commits use their existing native identities;
- the bead MUST NOT attempt to predict or contain the hash of the commit that will contain its own final state.

### 3.2 Direct correction instead of comment relays

Reviewers are expected to be capable editors.

When an issue is unambiguously resolvable from authoritative specifications and the approved scope, a reviewer SHOULD correct it directly rather than write a comment for another agent to interpret and apply.

The process MUST NOT imitate a pull-request comment loop merely for the sake of role separation.

Independence is achieved by requiring a fresh audit from an agent that did not originate the current material version. It does not require that corrections be handed back to the original author.

A bead reviewer MUST complete every correction available from authoritative material. If it materially changes the implementation contract, it becomes a partial author of that material version and MUST leave the bead `Pending Approval` for a fresh independent reviewer. Rereading its own corrected result is necessary for completeness but is not independent approval.

An implementation reviewer follows the same independence rule. It MUST directly complete every in-scope correction available under the approved contract. If it materially changes behavior, enforcement, verification strength, architecture, or the compliance claim, it becomes a partial author of the current implementation version and MUST leave implementation approval `Pending Approval`. A fresh reviewer that needs no material correction may complete final acceptance directly.

### 3.3 Explicit stopping conditions

Review MUST converge.

A reviewer MUST distinguish between:

- a requirement necessary for compliance;
- evidence necessary to verify compliance;
- a concrete implementation risk;
- an optional improvement;
- a stylistic preference;
- a speculative future concern.

A bead-review pass is complete when the reviewer has corrected every issue it can resolve from authoritative material, reread the complete result, and classified it according to material changes and external blockers. A bead becomes `Ready` only when a fresh independent reviewer completes that pass without needing a material correction.

An implementation is good enough when every mandatory obligation is satisfied, every blocking issue is resolved, the required evidence exists, and the applicable quality gates pass.

An implementation-review pass is complete when the reviewer has made every available in-scope correction, rerun affected verification, reviewed the complete result, and classified the pass. Material correction leads to `Pending Approval`; a fresh pass requiring no material correction may run final gates and accept; a defective contract returns to bead approval; a genuine external issue blocks precisely.

Review MUST NOT remain open because the artifact could theoretically be made more detailed, more elegant, more generalized, or more comprehensively documented.

### 3.4 Human design remains authoritative

Agents MUST NOT resolve genuine architecture or semantic ambiguity by invention.

When authoritative material does not determine the answer, the correct outcome is a design or specification blocker, not a guessed implementation and not an endlessly elaborated bead.

---

## 4. Repository operating model

### 4.1 Work happens directly on `main`

All ordinary work happens directly on `main`.

The standard process uses no:

- feature branches;
- agent-created branches;
- worktrees;
- pull-request merge workflow;
- cherry-pick coordination;
- parallel branch reconciliation.

This is intentional. The repository favors a single visible working state over the coordination complexity produced by multiple agents editing divergent trees.

### 4.2 Sequential code-changing agents

Code-changing agents SHOULD operate sequentially.

Only one agent may normally modify the repository at a time.

Parallel modification MAY occur only when:

- the user explicitly authorizes it;
- relevant files or resources are reserved;
- the work is demonstrably independent;
- the resulting changes cannot interfere semantically or mechanically.

File reservation is a coordination aid, not a replacement for semantic review. The default remains sequential execution.

### 4.3 Baseline and working tree

At the start of implementation:

```text
HEAD = last accepted repository state
working tree relative to HEAD = current proposed implementation
```

The complete implementation-review boundary is therefore the repository difference from `HEAD`, including:

- staged changes;
- unstaged changes;
- relevant untracked files.

The ordinary implementation process MUST NOT create intermediate commits merely to provide a review baseline. `HEAD` already provides that baseline.

### 4.4 Working-tree inspection

Before modifying or reviewing code, an agent MUST inspect at least:

```text
current branch
current HEAD
working-tree status
active bead
known file reservations
```

An unexpected dirty tree MUST NOT be discarded, overwritten, or assumed to belong to the active bead.

The agent must determine whether the changes:

- belong to the active bead;
- belong to another task;
- are unexplained;
- make safe continuation impossible.

Unexplained conflicting changes are a coordination blocker.

### 4.5 Destructive commands

Agents MUST NOT use destructive cleanup commands against unexplained work.

Commands equivalent to the following are forbidden unless the user explicitly authorizes them for a known purpose:

```bash
git reset --hard
git clean -fd
git checkout -- .
git restore --source=HEAD --worktree --staged .
```

An agent may revert or replace changes it has positively identified as belonging to its own active work, but it must not erase unknown repository state.

---

## 5. Governing artifacts and traceability

### 5.1 Authoritative specifications

Normative mossignal specifications live under `docs/specs/` unless `AGENTS.md` or a specification establishes a more specific rule. Other material under `docs/` is not automatically authoritative.

The authoritative specifications define the required semantics and architecture.

A bead does not replace them.

The repository process documentation and agent skills MUST establish:

- which documents are authoritative;
- precedence rules where documents overlap;
- how specification versions or repository commits are locked for a bead;
- how unresolved conflicts are escalated.

### 5.2 The bead as implementation contract

A bead selects and operationalizes a bounded subset of the authoritative requirements.

A bead SHOULD identify its specification baseline by repository commit and cite authoritative sources by document path, stable section heading or anchor, and brief governing subject. A `Ready` bead SHOULD normally contain:

```text
objective
authoritative sources
local obligations
forbidden outcomes
scope and non-goals
expected evidence
concern or risk flags
dependencies and blockers
approval state and approval baseline
```

The bead SHOULD summarize requirements precisely, but it SHOULD NOT copy large portions of the specifications.

The implementation reviewer remains responsible for consulting the governing specifications where necessary to interpret the bead, verify adjacent constraints, or detect an error in the bead itself.

### 5.3 Local obligation identifiers

A bead MAY assign local obligation identifiers such as:

```text
O1
O2
O3
```

These identifiers are scoped to the bead.

An obligation should combine closely related normative requirements into one executable and reviewable statement where doing so loses no material detail.

The process SHOULD avoid creating one obligation per sentence merely for mechanical completeness.

### 5.4 Clean bead-approval record

Ordinary bead review MUST NOT retain a finding ledger or review-pass chronology in the current bead contract.

Resolved issues disappear into the corrected contract. A `Ready` bead contains no resolved findings, earlier blockers, planning-review chronology, or advisory commentary. A blocked bead lists only currently active unresolved blockers. When a blocker is resolved, its obsolete text is removed before the bead returns to `Pending Approval`.

Non-blocking future ideas SHOULD become separate beads only when they are worth tracking. Git and bead history preserve prior planning states without burdening implementers with review archaeology.

Implementation review follows the same cleanliness rule. Resolved implementation defects disappear into corrected code, tests, evidence, and Git history. The bead retains only its current implementation approval state or an exact active blocker.

### 5.5 One evolving evidence record

The bead should maintain one evolving obligation and evidence record rather than separate duplicated compliance matrices for every agent role.

Implementation and review may enrich the same obligation with:

```text
implementation location
test location
verification command
status
implementation approval
```

The record MUST remain compact enough to be useful.

---

## 6. Bead lifecycle

The exact command-line state labels may vary, but the repository workflow MUST distinguish the following semantic states.

```text
Pending Approval
Bead Review
Ready
Blocked — Design Required
Blocked — Rescoping Required
Implementing
Implementation Review
Closed
```

Beads CLI storage states and workflow labels MAY encode these semantic states. In the current repository convention:

```text
Pending Approval             = status draft + workflow:pending-approval
Bead Review                  = status in_progress + workflow:bead-review
Ready                        = status open + workflow:ready
Blocked — Design Required    = status blocked + workflow:blocked-design
Blocked — Rescoping Required = status blocked + workflow:blocked-rescoping
```

A temporary internal `Draft` state MAY exist while an author is actively constructing a bead, but it is not the completed output handed to a reviewer.

### 6.1 `Pending Approval`

The current bead is believed to be a complete implementation contract and has no known blocker, but its current material contents have not yet passed a fresh independent approval review.

`Pending Approval` does not mean known incomplete, changes requested, blocked, partially reviewed, awaiting the same reviewer to continue, or unsuitable for review.

There are two ordinary entries:

- an author completes the strongest contract it can;
- a reviewer completes a full pass that materially revises the contract.

A bead in `Pending Approval` MUST NOT be selected for implementation.

### 6.2 `Bead Review`

A fresh bead reviewer is auditing and editing the bead.

The reviewer determines whether the bead is complete, correctly sourced, bounded, executable, and objectively verifiable.

Every pass is complete. The reviewer MUST NOT knowingly defer correctable issues merely because material corrections will require another fresh approval pass.

### 6.3 `Ready`

The complete current material version has passed a fresh independent review without requiring a material correction during that pass.

`Ready` means:

- its governing sources are identified;
- its obligations adequately represent the relevant requirements;
- its forbidden outcomes are explicit where needed;
- its scope is coherent;
- expected evidence is objectively checkable;
- no unresolved architecture or semantic decision remains;
- the approval record names the repository baseline reviewed;
- the reviewer made no material correction during the approving pass.

A reviewer MAY make nonmaterial corrections and still mark the bead `Ready`. When uncertain whether a correction is material, the reviewer MUST treat it as material.

A `Ready` bead contains only the approved implementation contract, expected evidence, approval baseline, and explicit `Ready` state. It MUST NOT retain resolved findings, previous blockers, review-pass chronology, or advisory commentary unrelated to implementation.

Its approval record should be broadly equivalent to:

```text
Bead approval:
Status: Ready
Reviewed against repository commit: <baseline HEAD>
```

### 6.4 `Blocked — Design Required`

The task cannot be implemented safely because authoritative design is missing, contradictory, or genuinely undecided.

The process returns to specification or human architectural work.

### 6.5 `Blocked — Rescoping Required`

The intended work cannot be represented as one coherent implementation unit without a human decision about task boundaries or priorities.

A reviewer SHOULD rescope directly when the correct decomposition follows unambiguously from the specifications. This blocked state is reserved for material judgment calls.

A blocked bead records only currently active unresolved blockers. After resolution, obsolete blocker text is removed and the bead returns to `Pending Approval`; it does not become `Ready` without a fresh clean review.

Its approval record should identify the semantic blocked state and list only exact active blocking issues.

### 6.6 `Implementing`

An implementer is modifying the uncommitted working tree to satisfy the approved bead.

The approved normative contract is considered locked during this stage.

### 6.7 `Implementation Review`

The complete uncommitted implementation is pending fresh independent approval. A fresh reviewer audits and directly corrects it against:

```text
approved bead
governing specifications
current HEAD
complete working-tree difference from HEAD
surrounding repository context
```

The semantic implementation approval state is recorded in the bead while the lifecycle label remains `workflow:implementation-review`.

- If the reviewer makes a material implementation correction, the current version remains `Pending Approval` for another fresh reviewer. It is not closed, flushed, staged, or committed.
- If a fresh reviewer makes no material correction and finds no blocker, that reviewer runs the full mandatory gates and completes final acceptance directly.
- If the approved contract is materially defective, the implementation is safely removed and the bead returns to `Pending Approval` or an exact blocked state.

A fresh clean implementation-review pass completes final acceptance directly.

### 6.8 `Closed`

The bead has passed a fresh clean implementation-review pass, all mandatory gates, and final acceptance.

For implementation beads, closure occurs **before** the accepted commit is created.

Closure alone is not the final repository operation. The bead store must then be flushed so that the closed state is included in the same accepted commit as the implementation.

---

## 7. Bead authoring

### 7.1 Author responsibilities

The bead author MUST:

- identify one bounded implementation objective;
- record the repository commit used as the specification baseline;
- cite directly governing specification material by path, stable section heading or anchor, and governing subject;
- identify relevant adjacent constraints;
- inspect relevant current code, tests, call sites, dependencies, and overlapping open, active, blocked, and closed beads;
- derive a manageable set of implementation obligations;
- state important forbidden outcomes;
- define scope and non-goals;
- specify the expected evidence;
- identify dependencies and known blockers;
- create the strongest complete implementation contract it can;
- leave the completed material version in `Pending Approval` with no known blocker.

The author MUST NOT approve its own bead, flush the bead store, stage planning changes, or create a planning commit.

Current code is evidence about repository state. It is not semantic authority when it conflicts with the specifications.

### 7.2 Pending-approval record

A completed authored bead should contain an explicit approval section broadly equivalent to:

```text
Bead approval:
Status: Pending Approval
Reason: Awaiting initial independent approval.
Known blockers: None
```

The exact phrasing may differ.

The important semantic facts are that the bead is believed complete, is visibly not independently approved, and cannot be mistaken for implementation-ready work.

The author SHOULD perform a final sanity check by rereading the stored bead and ensuring it is the strongest complete contract it can produce. This self-check is not an independent audit.

The author MUST use non-flushing bead mutations where supported. It leaves the `Pending Approval` bead uncommitted for the first independent reviewer, which will approve, materially revise, or block it and create the durable planning commit.

### 7.3 Architectural closure

A bead is not authorable as an ordinary blocker-free `Pending Approval` implementation contract when implementation would require the implementer to decide matters such as:

- whether a new stable identity category exists;
- whether a value is semantic or derived;
- whether persistence compatibility is required;
- whether state should be preserved or reset;
- whether a public API should be broadened;
- how contradictory specifications should be reconciled;
- what failure semantics should apply.

These questions belong to specification or architecture work.

When an author identifies such an unresolved external issue, it SHOULD record `Blocked — Design Required` rather than create an ordinary blocker-free implementation contract. When a material human decision about task boundaries or priorities is required, it SHOULD record `Blocked — Rescoping Required`. The author records the exact active issue, does not guess, and does not flush or commit that authoring result.

### 7.4 Appropriate level of detail

A bead should be precise enough that compliance can be determined objectively, but it should not become line-by-line pseudocode.

It may intentionally leave implementation freedom for matters such as:

- private helper structure;
- internal collection selection where behavior is unaffected;
- module-local naming;
- allocation strategy where not observable;
- code organization within established boundaries.

---

## 8. Bead review

### 8.1 Independent auditing editor

The ordinary target is a bead in `Pending Approval`. A blocked bead may enter approval review only after its external issue is resolved and it explicitly returns to `Pending Approval`. A `Ready`, implementing, or closed bead MUST NOT silently pass through ordinary bead review without an explicit reason to reopen approval.

The bead reviewer MUST be fresh with respect to authorship of the current material version.

The reviewer acts as an auditing editor, not merely a commenter.

The reviewer MUST independently verify source authority, reread cited specification sections with adequate context, search omitted adjacent requirements, inspect relevant current code and tests, inspect dependencies and overlapping beads, and directly amend every defect resolvable from authoritative material.

Examples include:

- adding a missing obligation;
- correcting a distorted requirement;
- tightening an acceptance criterion;
- adding a forbidden fallback;
- correcting source references;
- narrowing accidental scope;
- identifying mandatory verification from the testing policy.

The reviewer MUST NOT implement production code during bead review.

### 8.2 Review questions

The reviewer should determine:

- Does every material obligation have authoritative support?
- Are relevant source requirements within scope represented?
- Does the bead omit an important negative constraint?
- Could the task be implemented without inventing architecture?
- Are initialization, failure, persistence, reconfiguration, inspection, diagnostics, or verification relevant?
- Are acceptance criteria objectively checkable?
- Is the task one coherent semantic slice?
- Does the bead contradict existing public types or terminology?
- Are the requested outcomes actually possible?

These questions guide the review. They do not require a long visible checklist of obvious `N/A` entries.

### 8.3 Material and nonmaterial corrections

A correction is material when it changes the implementation contract, including:

- objective;
- normative obligations;
- forbidden outcomes;
- scope or non-goals;
- authoritative interpretation;
- expected evidence or mandatory verification;
- dependencies;
- concern or risk flags;
- completion criteria;
- public, persistence, migration, failure, identity, or compatibility requirements.

A correction is nonmaterial when it preserves meaning, including spelling, grammar, formatting, harmless wording cleanup, correction of an obvious path or heading typo, layout normalization, removal of obsolete review-history text, or correction of an unambiguous reference without changing the cited requirement.

When uncertain whether a correction is material, the reviewer MUST treat it as material.

The reviewer MUST reread its complete corrected result. That establishes completeness, but it does not provide independent approval for material changes made by that reviewer.

### 8.4 Blocking versus correctable issues

A bead-review issue is blocking only when it identifies at least one of:

- a missing or distorted governing requirement;
- an unresolved architecture or semantic decision;
- a contradiction;
- an impossible acceptance criterion;
- materially incomplete verification;
- unsafe or incoherent scope;
- a concrete route by which an implementation could violate the specifications.

Stylistic preferences, optional examples, speculative future concerns, and possible refinements are not blockers.

Non-blocking future ideas SHOULD NOT clutter the current bead. They MAY become separate beads when they are actually worth tracking.

### 8.5 Review outcomes

After completing all available corrections, rereading the result, and removing obsolete approval or blocker text, the reviewer MUST choose one terminal outcome for the pass:

```text
Ready
Pending Approval
Blocked — Design Required
Blocked — Rescoping Required
```

- `Ready`: no unresolved blocker and no material correction was required during this fresh pass. Nonmaterial corrections are permitted.
- `Pending Approval`: no unresolved blocker, but the reviewer materially revised the contract. The reviewer completed its work, but a fresh agent must independently approve the current version.
- `Blocked — Design Required`: existing authoritative material cannot determine a required semantic or architectural answer.
- `Blocked — Rescoping Required`: task boundaries or priorities require a material human judgment.

Every terminal result records the full repository baseline reviewed. `Ready` and materially revised `Pending Approval` update the bead’s specification baseline to that review baseline.

A generic “changes requested” handoff is unnecessary. Every review pass is complete.

### 8.6 Clean-review approval rule

A bead becomes `Ready` only after a fresh independent reviewer audits the complete current material version, finds no unresolved blocker, and does not need to make a material correction.

A reviewer that materially changes the bead MUST NOT mark that same material version `Ready` during the same pass. It has become a partial author of that version. Rereading its edits is required but is not independent approval.

The ordinary iterative loop is:

```text
Author → Pending Approval
Reviewer A completes review and materially revises → Pending Approval
Reviewer B completes review without material correction → Ready
```

Reviewers MUST NOT intentionally leave known correctable issues for the next pass. Iteration exists to preserve independence after complete material revision, not to encourage partial reviews.

### 8.7 Planning commit

Every completed bead-review pass MUST create a durable planning commit, whether its terminal result is `Ready`, materially revised `Pending Approval`, or a blocked state.

The reviewer MUST:

1. leave a clean current bead record containing only the current contract and active approval or blocker state;
2. run `br sync --flush-only`;
3. inspect the resulting diff and ensure it contains only intended planning changes;
4. stage the synchronized bead record;
5. inspect the staged diff;
6. create a commit whose subject begins with the bead ID;
7. leave no planning-related working-tree residue.

Appropriate subjects include:

```text
<bead-id>: approve <task summary>
<bead-id>: revise <task summary> bead
<bead-id>: block <task summary> on <specific issue>
```

A `Pending Approval` planning commit is complete work: it records a completed material revision awaiting fresh independent approval. A blocked planning commit likewise records a completed conclusion about an external blocker.

---

## 9. Approved-contract stability

Once a bead enters `Ready`, the following portions are normative and SHOULD be treated as locked:

```text
objective
obligations
forbidden outcomes
scope and non-goals
required evidence
concern flags
```

Implementers and implementation reviewers may update:

```text
implementation evidence
verification results
implementation approval state
implementation-discovered blockers
status
closure information
```

If implementation reveals that the approved contract is materially wrong or incomplete, the bead MUST return to `Pending Approval` or the appropriate blocked state before fresh bead review.

An implementation reviewer MUST NOT silently change the approved semantic scope and then approve code against the revised scope.

Nonmaterial wording corrections do not require renewed approval.

---

## 10. Implementation

### 10.1 Selecting work

An implementation agent may select only a bead whose `Ready` planning state has been flushed and committed.

The agent MUST confirm:

- the bead is approved;
- dependencies are complete;
- the working tree is clean;
- `HEAD` is the accepted baseline;
- no conflicting active work exists.

Existing uncommitted implementation work belongs to interrupted-work recovery, not a new ordinary implementation pass.

### 10.2 Uncommitted implementation

The implementer modifies the working tree directly on `main`.

The implementer MUST NOT create an implementation commit before review merely to preserve progress or provide a diff boundary.

The repository difference from `HEAD` is the proposed implementation.

### 10.3 Implementation obligations

Before handoff, the implementer SHOULD:

- read the complete approved bead;
- read the cited specification sections with sufficient surrounding context;
- inspect relevant existing types and call sites;
- derive the required tests;
- implement the obligations at appropriate enforcement layers;
- add or update tests;
- update provisional implementation evidence;
- run focused iteration checks;
- leave the complete work uncommitted;
- record implementation approval `Pending Approval`;
- move the bead to `Implementation Review` without flushing or committing.

### 10.4 No architectural invention

If a required representation or behavior is not determined by the bead and specifications, the implementer must stop that portion and return the bead to `Pending Approval` or the appropriate blocked state.

The implementer MUST NOT invent semantic identities, fallback variants, compatibility rules, public abstractions, or migration policies merely to make the task compile.

### 10.5 Handoff state

At implementation handoff:

```text
HEAD = unchanged accepted baseline
working tree = complete proposed bead implementation
bead = Implementation Review with approval Pending Approval
```

The implementer’s reported command results are useful evidence but are not substitutes for the fresh approving implementation reviewer rerunning the required gates.

---

## 11. Implementation review

### 11.1 Review object

Implementation review examines:

```text
approved bead
applicable authoritative specifications
HEAD
all staged changes
all unstaged changes
all relevant untracked files
surrounding unchanged code
tests and verification evidence
```

The reviewer MUST inspect more than the textual diff where the surrounding design or invariant requires it.

### 11.2 Direct correction

The implementation reviewer SHOULD directly:

- fix implementation defects;
- strengthen tests;
- correct incomplete evidence;
- remove forbidden alternatives;
- improve enforcement;
- rerun relevant checks.

The reviewer MUST NOT retain resolved defects, correction chronology, or advisories in the bead. Resolved issues disappear into corrected implementation, tests, evidence, and Git history. Only exact active blockers remain recorded.

The reviewer MUST NOT silently broaden the approved task.

### 11.3 Three-way comparison

Implementation review is a three-way comparison:

```text
specifications
        ↕
approved bead
        ↕
implementation and tests
```

The reviewer must not assume that prior bead approval proves the bead perfect.

If the bead itself contains a material error, the proper response is to return it to `Pending Approval` or the appropriate blocked state for fresh bead review.

### 11.4 Review focus

The reviewer should verify:

- every bead obligation has real implementation coverage;
- the implementation covers every relevant code path;
- the enforcement layer is appropriate;
- tests can fail when the requirement is violated;
- negative constraints are actually enforced;
- adjacent contracts remain valid;
- Rust types, visibility, ownership, trait bounds, and conversions are coherent;
- public and persisted shapes remain compatible where required;
- no unapproved architecture has been introduced.

### 11.5 Material and nonmaterial implementation corrections

A correction is material when it changes implementation behavior, enforcement, verification strength, architectural meaning, or the truth of the compliance claim. Material corrections include changes to production logic, validation, failure behavior, state transitions, ordering, persistence, migration, diagnostics, API or visibility boundaries, ownership, required enforcement, panic paths, test assertions or oracles, regression or advanced verification coverage, compliance evidence, or mandatory verification commands.

A correction is nonmaterial only when it cannot affect behavior, architecture, verification strength, or the compliance claim, such as formatting, import ordering, spelling, harmless comments, equivalent mechanical cleanup, an obvious evidence typo that does not change the claim, or obsolete review-state removal.

When uncertain, the reviewer MUST treat the correction as material.

The reviewer MUST exercise affected paths and rerun relevant checks after correction. This completes and stabilizes the work but is not independent approval of material edits made by that reviewer.

### 11.6 Review outcomes

Every implementation-review pass is complete and ends in one of four outcomes:

```text
Implementation Pending Approval
Approved and committed
Returned to bead approval
Blocked
```

#### Implementation Pending Approval

If the reviewer makes any material correction and no blocker remains, it MUST leave the complete corrected implementation uncommitted with:

```text
Implementation approval:
Status: Pending Approval
Reason: The current implementation was materially revised during review and requires fresh independent approval.
Known blockers: None
```

The bead remains status `in_progress` with `workflow:implementation-review`. The reviewer MUST NOT close, flush, stage, or commit. A fresh implementation reviewer audits the current material version.

#### Approved and committed

If a fresh reviewer completes the full adversarial pass without a material correction and finds no blocker, it may perform final acceptance directly under Section 12. Nonmaterial cleanup is permitted.

#### Returned to bead approval

If the approved normative contract is materially wrong or incomplete but existing authoritative material appears sufficient to correct it, the reviewer MUST NOT revise that contract. It safely removes only the current bead implementation and evidence, restores accepted `HEAD`, records the exact implementation-discovered issue, and returns the bead unflushed and uncommitted to semantic `Pending Approval` with `workflow:pending-approval`.

#### Blocked

If a required semantic or architectural answer is genuinely missing or contradictory, the reviewer safely restores accepted `HEAD` and records `Blocked — Design Required`. If task boundaries require a material human judgment, it records `Blocked — Rescoping Required`. Only the exact active blocker remains.

If safe targeted restoration cannot distinguish the current bead’s changes from unknown work, the reviewer stops without flushing or committing and invokes exceptional recovery. A bead returned to planning MUST NOT remain mixed with uncommitted production implementation changes.

### 11.7 Independence and stopping rule

A reviewer directly fixes every correctable in-scope defect. If those fixes are material, the current implementation requires a fresh independent approval pass.

The reviewer MUST NOT stop early because another pass may occur, issue a mechanical comment relay, approve its own material edits, or continue indefinitely for elegance, generality, optional abstraction, or stylistic preference.

Finish every correction available within the approved contract, rerun affected verification, and re-review the complete result. Then classify the pass:

- material correction made: `Pending Approval`;
- no material correction and no blocker: run final gates and accept;
- defective contract: return to bead approval;
- genuine unresolved design or rescoping issue: block precisely.

---

## 12. Clean implementation approval and acceptance

### 12.1 Clean-pass requirement

The approving implementation reviewer MUST be fresh with respect to the current material implementation version. It must audit the complete working tree relative to `HEAD`, find no unresolved blocker, and need no material correction during that pass.

The reviewer MUST NOT trust implementer or prior-review reports as proof. Nonmaterial cleanup is permitted. Any material correction returns the implementation to `Pending Approval` and prevents acceptance in that pass.

### 12.2 Mandatory verification

The approving reviewer MUST independently rerun against the exact tree being accepted:

- the canonical full repository quality gate;
- every mandatory bead-specific command;
- relevant regression tests;
- required compatibility, differential, persistence, migration, or failure-atomicity checks activated by the bead.

Command unavailability, designated warnings, timeout, hang, flakiness, or failure blocks acceptance.

### 12.3 Approval record

Before closure, the bead SHOULD contain a clean record broadly equivalent to:

```text
Implementation approval:
Status: Approved
Reviewed against accepted HEAD: <baseline HEAD>

Verification:
- <canonical repository acceptance gate>: passed
- <mandatory bead-specific command>: passed
```

It MUST NOT retain resolved findings, correction chronology, or advisories.

### 12.4 Acceptance responsibilities

The approving implementation reviewer MUST:

1. inspect repository status and the complete diff from `HEAD`;
2. verify that every change belongs to the active bead;
3. verify each mandatory obligation and the accuracy of implementation evidence;
4. record explicit implementation approval and verification;
5. close the bead;
6. run `br sync --flush-only`;
7. inspect the synchronized bead JSONL change;
8. stage implementation, tests, required documentation, evidence, and bead state;
9. inspect the complete staged diff;
10. create one accepted implementation commit whose subject begins with the bead ID;
11. verify the working tree is clean.

The reviewer MUST NOT push unless separately authorized.

---

## 13. Bead closure and synchronization

### 13.1 Closure occurs before commit

The bead MUST be closed before the accepted implementation commit is created.

This ordering ensures that the commit contains:

- the completed implementation;
- tests and documentation;
- final implementation approval state;
- the closed bead state;
- the synchronized bead JSONL record.

### 13.2 Bead flush

After closing the bead, the approving implementation reviewer MUST run:

```bash
br sync --flush-only
```

This exports the current bead changes to the repository’s JSONL representation.

The reviewer MUST inspect the resulting repository changes and confirm that they correspond to the active bead and expected bead-store updates.

### 13.3 Flush failure

If bead closure or `br sync --flush-only` fails, the accepted commit MUST NOT be created.

The bead and repository state must be reconciled first.

### 13.4 No post-commit closure commit

The normal process MUST NOT create one implementation commit followed by another commit solely to close or flush the bead.

The closed and flushed bead state belongs in the same accepted commit as the implementation.

---

## 14. Commit semantics

### 14.1 Meaning of a commit

A commit records one completed and accepted repository-state transition. The process distinguishes two ordinary commit classes.

#### Planning commit

A planning commit records one completed independent bead-review pass whose result is:

```text
Ready
materially revised Pending Approval
Blocked — Design Required
Blocked — Rescoping Required
```

It contains the clean synchronized planning record and references the bead ID. It does not claim implementation is complete.

#### Accepted implementation commit

An accepted implementation commit means:

```text
the bead was approved before implementation;
the implementation satisfies the approved contract;
independent implementation review occurred;
mandatory automated checks passed;
the bead was closed;
the bead store was flushed;
the complete accepted change was committed together.
```

Both classes are accepted repository checkpoints, not ordinary intermediate agent saves.

### 14.2 One bead per commit

An accepted planning or implementation commit SHOULD correspond to one bead.

Unrelated beads MUST NOT normally be combined into one commit.

A bead ordinarily has one planning commit per completed approval pass and one accepted implementation commit when implementation finishes. Multiple planning commits are expected when material revisions require fresh approval.

Implementation may exceptionally require more than one accepted implementation commit when work must be partitioned for a concrete repository reason, but each such commit must independently satisfy the applicable acceptance semantics and reference the bead ID.

### 14.3 Commit-to-bead linkage

Every planning, implementation, test, implementation-facing documentation, review-fix, or corrective commit MUST reference the relevant bead ID.

The required subject form is:

```text
<bead-id>: <imperative semantic summary>
```

Examples:

```text
ms-101: approve canonical persistence bead
ms-102: revise topology migration bead
ms-103: block diagnostic lifecycle on episode identity
ms-142: enforce canonical diagnostic ordering
ms-207: preserve toggle state across compatible replacement
ms-311: reject incomplete initialization snapshots
```

The bead ID SHOULD be the first token so that linkage is mechanically searchable.

### 14.4 Subject quality

The subject should describe the accepted semantic or architectural outcome.

Good:

```text
ms-142: enforce canonical diagnostic ordering
```

Poor:

```text
ms-142: update files
ms-142: implement bead
ms-142: fixes
```

### 14.5 Commit body

A commit body is RECOMMENDED for nontrivial beads.

It should normally summarize:

```text
Outcome:
- the important semantic or architectural changes

Verification:
- the canonical quality-gate command
- important additional bead-specific checks

Notes:
- material compatibility, migration, or limitation information
```

The body should remain concise.

It SHOULD NOT:

- duplicate the full bead;
- copy specification paragraphs;
- list every changed file;
- reproduce long command output;
- narrate every review edit;
- contain the final commit hash.

### 14.6 No self-referential commit hash in the bead

The bead MUST NOT require its final commit hash as part of its own pre-commit closed state.

The durable primary linkage is:

```text
commit message → bead ID
```

Repository tools may later derive the matching commit from Git history.

---

## 15. Automated quality gates

### 15.1 Role of automation

Automated checks establish mechanical and executable quality.

They do not independently prove specification compliance.

The process therefore requires both:

```text
semantic review against specifications and bead
+
mandatory automated quality gates
```

A passing test suite cannot legitimize an implementation that violates the approved architecture.

A strong semantic argument cannot excuse code that fails required compilation, linting, formatting, or tests.

### 15.2 Canonical repository gate

The repository SHOULD define one version-controlled canonical command for ordinary final verification, for example:

```bash
./scripts/quality-gate
```

or:

```bash
just quality-gate
```

The exact name is a repository choice.

Agents SHOULD invoke the canonical command rather than reconstruct the standard command list from memory.

For a Rust workspace, the gate will normally include equivalents of:

```text
format verification
workspace compilation or checking
linting with repository-approved warnings treated as failures
unit tests
integration tests
documentation tests where applicable
repository-specific structural validation
```

The exact Cargo flags and feature combinations must be defined centrally.

### 15.3 Bead-specific verification

A bead may require additional mandatory checks based on its obligations and concern flags.

Examples include:

```text
incremental-versus-reference differential tests
canonical persistence round trips
historical compatibility vectors
failure-atomicity fault injection
exact-deadline boundary tests
all-topological-order bounded checks
public API compile tests
diagnostic episode lifecycle tests
migration accounting tests
```

The bead SHOULD list only the additional checks. It should not duplicate the standard gate.

### 15.4 Verification levels

The repository should distinguish three levels of automated checking.

#### Iteration checks

Fast checks used during implementation:

```text
format affected code
compile the relevant crate
run focused tests
```

Iteration checks provide feedback but do not constitute final acceptance.

#### Mandatory commit gate

Checks that MUST pass against the exact final working tree before commitment:

```text
canonical repository quality gate
all mandatory bead-specific checks
all relevant regression tests
```

#### Extended verification

Expensive checks that may run periodically, in CI, or for specifically affected beads:

```text
long property-test campaigns
bounded exhaustive exploration
fuzzing
Miri
large historical compatibility corpora
cross-platform matrices
performance regression suites
very long differential histories
```

An extended check becomes a mandatory bead gate when the bead materially changes the subsystem or risk that the check protects.

### 15.5 Hard pass/fail semantics

For a mandatory gate:

- failure blocks commitment;
- inability to run the command is not a pass;
- designated warnings are failures;
- a timeout or hang is a failure unless the gate explicitly defines otherwise;
- repeatedly rerunning a flaky test until it happens to pass is forbidden;
- weakening, deleting, or bypassing a test merely to clear the gate is forbidden;
- changes to test oracles receive the same scrutiny as production changes;
- environment-dependent behavior must be explained and resolved, not ignored.

### 15.6 Baseline failures

`main` should remain green.

A bead should normally begin from a known passing standard gate.

If the gate already fails at `HEAD`, the agent must establish whether the failure is:

- a known accepted baseline issue;
- an environment problem;
- unrelated repository breakage;
- caused by the active work.

An unexplained pre-existing mandatory failure prevents reliable acceptance and SHOULD be resolved before continuing ordinary implementation.

### 15.7 Gate changes

A bead that modifies the quality gate, lint policy, test harness, or reference oracle must treat that infrastructure as part of the reviewed implementation.

Agents MUST NOT weaken a gate merely to make the current bead pass.

Any intended reduction in coverage or strictness requires explicit bead obligations and independent review.

---

## 16. Review-mode distinctions

The repository process distinguishes two review activities.

### 16.1 Bead review

**Primary object:**

```text
bead in Pending Approval
authoritative specifications
relevant current code, tests, dependencies, and overlapping beads
```

**Purpose:** Complete an adversarial review, directly correct the contract, and determine whether the current material version qualifies for clean independent approval.

**Normal result:**

```text
Ready
Pending Approval after material revision
Blocked — Design Required
Blocked — Rescoping Required
```

The reviewer edits the bead directly, creates a clean terminal record, flushes it, and commits the completed planning pass.

### 16.2 Implementation review

**Primary object:**

```text
approved bead
uncommitted working tree relative to HEAD
applicable specifications
```

**Purpose:** Correct and verify work that has not yet passed acceptance controls.

**Normal result:**

```text
Implementation Pending Approval after material revision
Approved, closed, flushed, and committed after a clean pass
Returned to bead approval
Blocked precisely
```

The reviewer edits code and tests directly. A material correction requires another fresh pass; a clean pass performs final acceptance without a separate reviewer role.

---

## 17. Later-discovered defects

### 17.1 Closed work remains historical

After the accepted commit, the original bead remains closed as the historical record of what was accepted at that time.

A later task MUST NOT silently rewrite the original bead or edit committed code without a new bead.

### 17.2 Corrective bead

When a substantive defect is discovered after acceptance, the ordinary bead-authoring role should create a corrective bead that references:

- the original bead;
- the relevant accepted commit;
- the violated specification clauses;
- the observed implementation defect;
- the required corrective evidence.

The corrective bead then follows the ordinary review, implementation, quality-gate, closure, flush, and commit process.

### 17.3 Design escalation

If later investigation exposes a genuine specification ambiguity or contradiction, the bead author must record the exact design blocker rather than inventing a corrective implementation answer.

---

## 18. Role separation

Agent identities are not permanently assigned. Roles are per bead.

One agent may serve different roles at different times, subject to the following restrictions.

### 18.1 Bead author

May:

- author the strongest complete bead it can;
- revise it before independent review;
- later implement it after approval.

May not:

- independently approve its own bead;
- flush or commit its completed `Pending Approval` authoring result.

### 18.2 Bead reviewer

May:

- directly edit the bead;
- approve an unchanged or only nonmaterially corrected version as `Ready`;
- leave a materially corrected version in `Pending Approval`;
- block it for design or rescoping;
- flush and commit the completed planning pass.

Must be independent from authorship of the current material version for approval. A reviewer that materially edits the contract cannot mark that same version `Ready` in the same pass.

### 18.3 Implementer

May:

- implement an approved bead;
- add tests and evidence;
- run iteration checks.

May not:

- commit the ordinary implementation before independent acceptance;
- close the bead as a substitute for independent acceptance.

### 18.4 Implementation reviewer

May:

- inspect the complete working tree;
- directly fix code and tests;
- update evidence and implementation approval state;
- leave materially corrected work pending fresh approval;
- run mandatory gates and complete final acceptance after a fresh clean pass;
- close and flush the bead;
- create the accepted implementation commit.

Must not silently change the approved semantic contract or approve its own material implementation corrections. Must return a defective contract to `Pending Approval` or the appropriate blocked state.

---

## 19. Exceptional conditions

### 19.1 Unresolved design

When a task requires a decision not contained in authoritative material:

```text
do not guess
do not generalize speculatively
do not add an arbitrary fallback
do not keep reviewing indefinitely
```

Move the bead to the appropriate blocked state and report the exact missing decision.

### 19.2 Unexpected unrelated changes

If the working tree contains unrelated changes whose ownership cannot be established, implementation or review must stop before modifying or committing them.

### 19.3 Partially completed agent work

An interrupted agent may leave uncommitted work.

A recovery agent reviews the working tree as in-progress implementation. It does not assume the work is correct, complete, or safe merely because another agent created it.

### 19.4 Mandatory command unavailable

If a required command cannot run because of missing infrastructure, environment, credentials, platform support, or tool failure, the approving implementation reviewer must not report it as passed.

The issue must be resolved, the gate formally changed through an approved bead, or the task explicitly blocked.

### 19.5 Emergency repository recovery

Emergency reverts or repository recovery may require exceptions to the ordinary sequence.

Such exceptions require explicit user authorization and should be followed by a bead-backed reconciliation or corrective record.

---

## 20. Mechanically enforceable rules

The repository SHOULD automate simple process invariants where practical.

Potential checks include:

- planning and implementation commit subjects begin with a valid bead ID;
- one accepted planning or implementation commit references one bead;
- `Pending Approval` beads use the unambiguous workflow label and are not returned by implementation-ready selection;
- a `Ready` bead records a fresh approval baseline;
- a reviewer that records material contract changes leaves the bead `Pending Approval`;
- a bead is closed before its implementation commit;
- bead JSONL is synchronized in the accepted commit;
- no mandatory gate is omitted from the final implementation approval record;
- `Ready` beads contain no resolved findings, previous blockers, review chronology, advisories, or unresolved blockers;
- blocked beads contain only active unresolved blockers;
- `Closed` implementation beads contain final verification evidence;
- the canonical quality-gate command is version-controlled;
- files reserved by another active task are not modified.

Automation should validate structure and presence.

It should not attempt to replace semantic review with keyword counting or simplistic completeness scoring.

---

## 21. End-to-end normal procedure

The ordinary workflow is:

### Phase A — Bead preparation

1. An author derives the strongest complete implementation contract it can from authoritative specifications and current repository reality.
2. The author records the specification baseline and leaves the bead `Pending Approval` with no known blocker.
3. The author does not flush or commit.
4. A fresh bead reviewer performs a complete adversarial review against specifications, current code and tests, dependencies, and overlapping work.
5. The reviewer directly corrects every issue it can resolve and rereads the complete result.
6. The reviewer chooses exactly one terminal result:
   - `Ready` when no material correction was required;
   - `Pending Approval` when the contract was materially revised;
   - `Blocked — Design Required` for an unresolved external design issue;
   - `Blocked — Rescoping Required` for an unresolved human boundary decision.
7. The reviewer removes obsolete review and blocker history, flushes the bead record, and commits the completed planning pass with a bead-prefixed subject.
8. If the result is `Pending Approval`, a fresh reviewer repeats steps 4–7 against the current material version.
9. Implementation may begin only from a committed `Ready` planning state.

### Phase B — Implementation

10. An implementer selects a `Ready` bead.
11. The implementer confirms a clean accepted baseline on `main`.
12. The implementer modifies the uncommitted working tree.
13. The implementer adds tests and provisional evidence.
14. The implementer runs focused checks.
15. The bead moves to `Implementation Review` with implementation approval `Pending Approval`.

### Phase C — Implementation review and acceptance

16. A fresh implementation reviewer compares the working tree with the bead and specifications.
17. The reviewer directly fixes every in-scope defect it can resolve, strengthens tests, corrects evidence, reruns affected checks, and re-reviews the complete result.
18. If any correction was material, the implementation remains `Pending Approval`, unflushed and uncommitted, and a fresh reviewer repeats steps 16–18.
19. If the approved contract is defective, the reviewer safely restores accepted `HEAD` and returns the bead to approval or a precise blocked state without flushing or committing.
20. A fresh reviewer that needs no material correction runs:
    - the canonical repository quality gate;
    - every mandatory bead-specific check.
21. The reviewer verifies obligation evidence and records clean implementation approval.
22. The reviewer closes the bead.
23. The reviewer runs:

```bash
br sync --flush-only
```

24. The reviewer inspects the complete diff, including bead JSONL changes.
25. The reviewer stages the accepted change.
26. The reviewer inspects the staged diff.
27. The reviewer creates one bead-prefixed accepted implementation commit.
28. The reviewer confirms the working tree is clean.

---

## 22. Completion criteria

An implementation bead is fully complete only when:

```text
it was independently approved before implementation;
its mandatory obligations are implemented;
its tests and evidence are adequate;
its implementation received independent review;
all active blockers are resolved;
all mandatory gates pass against the final working tree;
no unapproved architectural decision was introduced;
the bead is closed;
the bead store is flushed;
the closed bead and accepted implementation are committed together;
the commit references the bead ID.
```

Implementation completion does not require pursuing optional improvements beyond mandatory compliance.

The process values defensible semantic correctness, explicit evidence, and independent review. It does not require unlimited refinement or maximal procedural detail.

---

## 23. Summary rule

The repository’s working rule is:

> Work is authored as a complete bead in `Pending Approval`. Fresh bead reviewers perform complete adversarial passes and commit each clean planning result: `Ready` only after a pass requiring no material correction, materially revised `Pending Approval` for another fresh pass, or an exact blocked state. Implementation begins only from committed `Ready` and remains uncommitted on `main`. Fresh implementation reviewers directly correct complete proposals; material corrections remain `Pending Approval`, while a clean pass runs all final gates, records approval, closes and flushes the bead, and creates the single accepted implementation commit.
