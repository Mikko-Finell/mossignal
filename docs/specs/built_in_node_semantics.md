# `mossignal` Built-in Node Semantics

**Status:** Consolidated design specification, version 2  
**Defines:** Built-in nodes, boundary endpoints, shared laws, inspection, explanation, diagnostics, initialization, and reconfiguration  
**Excluded:** Processor implementation, testing policy, model checking, contracts, serialized wire formats, and application-specific modules

---

## 1. Purpose

This specification defines the built-in semantic language of `mossignal`.

A built-in node is a domain-neutral operation that transforms signals, retains signal-related state, or schedules future signal behavior. The catalogue must remain small, precise, orthogonal, deterministic, fully inspectable, causally explainable, and compatible with state-preserving reconfiguration.

More elaborate behavior should ordinarily be expressed through named standard modules composed from built-in nodes.

A node deserves primitive status when first-class treatment materially improves atomic semantics, simultaneous-event handling, state preservation, pending-event migration, inspection, explanation, persistence, or diagnostics.

---

# Part I — Shared semantic rules

## 2. Signal kinds

The core kinds are:

```rust
pub enum Level {}
pub enum Pulse {}
```

### 2.1 Level

```rust
pub enum LogicLevel {
    Low,
    High,
}
```

A level is persistent binary signal state.

`Low` is a real signal value. It is not an absence marker, an uninitialized value, or a default for missing external input.

The core does not add an `Unknown`, `Unset`, or third logic value. Absence of authoritative current signal state is represented by machine lifecycle, not by extending `LogicLevel`.

### 2.2 Pulse

A pulse is a discrete occurrence at one logical time.

```rust
pub struct PulseCount(/* non-negative integer */);
```

`PulseCount` MUST support at least `ZERO` and `ONE`.

Pulse multiplicity is semantically significant. A pulse is not a temporary `High`, a third level value, a self-resetting level, or an individually ordered token within a simultaneous batch.

## 3. Reactions and transactions

A **reaction** is one complete settlement at one logical time.

A caller transaction may contain several reactions when advancing across earlier pending deadlines before settling the caller-supplied transaction time.

Node semantics are defined per reaction. Transaction atomicity determines publication of the complete outer operation but does not collapse reactions at distinct logical times into one semantic batch.

## 4. Node categories

A node is:

- **Combinational:** stateless and non-temporal; output depends only on settled current inputs.
- **Stateful:** retains semantic state across reactions.
- **Temporal:** owns one or more future obligations at caller-supplied logical times.

Every stateful or temporal node must define initialization, simultaneous-input behavior, current output, successor state, inspection, causality, persistence, and reconfiguration compatibility.

## 5. Synchronous transducer model

Every built-in node is interpreted as a deterministic synchronous transducer.

During one reaction at time `T`, a node may read:

```text
previous committed state
settled current inputs
due temporal obligations at T
```

and produce:

```text
settled current outputs
at most one proposed successor value per state cell
future temporal additions or cancellations
diagnostic-condition updates
causal facts
```

Formally:

```text
(current outputs, proposed successor state, future effects)
    = F(previous state, current inputs, due obligations)
```

Current outputs are available to downstream nodes in the same reaction.

Proposed successor state is not visible as stored state during that reaction. All successor state commits once after complete settlement.

There are no hidden same-time microsteps and no repeated same-reaction transition of one state cell.

## 6. Glitch-free semantics

Only settled pre-reaction and post-reaction values are semantically observable.

Internal evaluation order MUST NOT create observable intermediate transitions.

Example:

```text
Before reaction:
  A = High
  B = Low
  Any(A, B) = High

At one logical time:
  A becomes Low
  B becomes High

After settlement:
  Any(A, B) = High
```

No intermediate `Low` exists semantically, and a downstream edge detector emits nothing.

This rule applies to outputs, downstream nodes, state changes, explanations, and inspection subscriptions.

## 7. Simultaneous batches

All stimuli at one logical time form one unordered batch.

Behavior MUST NOT depend on insertion order, connection order, collection iteration order, arbitrary ordering between simultaneous pulses, pending-event container order, or evaluator traversal order.

A conflict must have an explicit order-independent result, explicit priority, retain-and-diagnose behavior, or reaction rejection through failure of the containing transaction.

Stimuli at different logical times remain distinct reactions and MUST NOT be coalesced merely because one caller transaction crossed them.

## 8. Current-time controls

A node controlled by a level uses that level’s fully settled value for the current reaction.

Previous-level behavior requires explicit previous state.

## 9. Current-reaction dependency signatures

Every node kind defines a conservative static signature describing which current inputs may affect which current outputs within one reaction.

A dependency is included when influence is possible under any valid state or parameter configuration. It does not depend on current runtime values.

Previous-state facts and due-event facts may also affect current outputs, but they are reaction roots rather than current input ports.

The built-in signatures are:

| Node | Current-reaction input-to-output dependencies |
|---|---|
| `Constant` | none |
| `Not` | input → output |
| `All`, `Any`, `Parity`, `AtLeast` | every level input → output |
| `Select` | selector and both branches → output |
| `Merge`, `Coalesce`, `Zip` | every pulse input → output |
| `PulseGate` | pulses and enable → output |
| `PulseSelect` | selector and both branches → output |
| `PulseRoute` | selector and pulses → both outputs |
| `RisingEdge`, `FallingEdge`, `AnyEdge` | level input → pulse output |
| `Toggle` | toggle input → level output |
| `PulseSetResetLatch` | set and reset → level output |
| `LevelSetResetLatch` | set and reset → level output |
| `SampleHold` | value and sample → level output |
| `PulseDelay` | no current input → current output; due pulse group → output |
| `TransportDelay` | no current input → current output; due transition batch → output |
| `InertialDelay` | no current input → current output; matured candidate → output |
| `Periodic` | enable and due boundary → pulse output |

A whole stateful node is not automatically a causality barrier. A path breaks current-reaction causality only where it crosses previous committed state or strictly later logical time.

The complete current-reaction dependency graph MUST be acyclic. Validation therefore rejects current-reaction cycles, including cycles that pass through stateful nodes whose current inputs affect their current outputs.

No unspecified fixed-point or same-time microstep semantics are used.

## 10. Pulse fan-out

Pulses are information, not consumable tokens.

One pulse output connected to several inputs sends the complete count to every input.

Fan-out is intrinsic. There is no splitter primitive.

## 11. Pulse multiplicity

Every pulse-consuming node must explicitly preserve, sum, coalesce, group, suppress, bound, or otherwise transform multiplicity.

Multiplicity MUST NOT be silently discarded.

## 12. Distinct state concepts

Specifications must distinguish:

```text
current port values
internal semantic state
pending future obligations
active diagnostic episodes
optional diagnostic or causal history
```

The latest pulse time is history, not persistent pulse state. A delay queue is pending work, not a signal value. An active diagnostic episode is semantic state because it affects future diagnostic publication.

## 13. Machine and node initialization

A newly spawned machine is uninitialized. It has declared node state but no authoritative external level valuation, settled current signal valuation, output baseline, pending schedule, or current runtime explanation.

The first successful transaction supplies a complete input snapshot and establishes the first logical time. Its first reaction uses:

```text
declared initial node state
+ complete settled external level input
+ initial pulse batch
+ no previously pending due obligations
```

Declared initial state acts as previous committed state for that reaction. A stateful node may therefore produce a different current output and successor state immediately during initialization.

A node newly added by reconfiguration follows the same first-reaction rule under the new topology.

Before machine initialization, structural inspection may expose node definitions and declared initial state. Current runtime inspection must fail structurally as not initialized.

## 14. Initial state declarations

Every stateful node and level-producing temporal node has an explicit initial condition.

Implicit defaults SHOULD be avoided.

Changing an initial-state parameter does not silently overwrite preserved runtime state. It affects newly created state, explicitly reset state, or state whose migration rule selects reinitialization.

Edge detectors have an explicit initialization policy.

## 15. Variadic inputs

Every variadic input slot has stable port identity independent from display order.

For commutative nodes, reordering ports does not change behavior.

## 16. Duplicate sources

Connecting one upstream output to multiple ports of one node is valid and counted once per port.

```text
Merge(source, source), source count 3 -> 6
Parity(source, source), source High -> Low
AtLeast(2, source, source), source High -> High
```

This SHOULD produce a non-blocking duplicate-source diagnostic. Ordinary fan-out to separate nodes does not.

## 17. Total semantics

Where a natural mathematical result exists, well-typed arities and parameters SHOULD have total semantics.

```text
All([])          = High
Any([])          = Low
Parity([])       = Low
AtLeast(0, ...)  = High
AtLeast(5 of 3)  = Low
All([x])         = x
```

Suspicious forms normally produce warnings rather than errors.

Blocking validation is reserved for incoherent structures such as kind mismatch, missing fixed ports, invalid direction, unsupported multiple drivers, invalid temporal parameters, and current-reaction dependency cycles.

## 18. Explanations and causes

Current justification is distinct from the most recent transition cause.

A level may remain unchanged while its supporting inputs change.

Every node defines:

- **supporters:** facts currently justifying the result;
- **blockers:** facts preventing a requested alternative.

Explanations SHOULD report all current supporters or blockers rather than an arbitrary minimal proof.

Stateful explanations distinguish previous state, current control facts, the rule producing current output and successor state, and the cause that most recently established retained state.

Temporal explanations distinguish due obligations, current input, future scheduling, cancellation, and migration.

## 19. Diagnostic occurrences and episodes

A transient condition produces a diagnostic occurrence for the reaction in which it exists.

A condition that may remain continuously true across reactions is represented as an active diagnostic episode.

An episode has stable semantic identity based on at least:

```text
diagnostic code
owning structural subject
condition discriminator
```

An episode emits a diagnostic event when it begins, materially changes, and optionally when it resolves. It MUST NOT emit the same unchanged warning on every unrelated transaction.

The active condition remains visible through inspection and participates in snapshots, restoration, replay, digests, and migration.

A rejected transaction does not commit a new episode.

## 20. Shared reconfiguration rules

Reconfiguration is evaluated against the actual state reached at the patch’s effective time.

For every surviving stateful or temporal node, planning and finalization must classify state as:

```text
Preserve
Migrate
Reset
Reject
```

Every pending event must receive one explicit outcome:

```text
PreserveDeadline
RecomputeDeadline
TransformPayload
Cancel
Reject
```

No state or pending event may disappear silently.

Changing connections or topology seeds ordinary reevaluation under the new reaction graph. Topology changes are causes; they are not synthetic levels or pulses.

A preserved node’s initial-state parameter is not reapplied unless the selected migration outcome is `Reset`.

---

# Part II — Level combinational nodes

## 21. Catalogue

```text
Constant
Not
All
Any
Parity
AtLeast
Select
```

## 22. `Constant`

```text
Inputs: none
Output: Level
Parameter: LogicLevel
Law: output equals configured level
```

`net.low()` and `net.high()` may be typed conveniences.

An unconnected required input never receives an implicit default. There is no pulse constant.

Changing the configured value changes the current output through ordinary topology-induced reevaluation.

## 23. `Not`

```text
Inputs: one Level
Output: Level
Law: logical complement
```

Explanation identifies the input and its opposite output.

## 24. `All`

```text
Inputs: zero or more Levels
Output: Level
Law: High iff every input is High
```

```text
All([])  = High
All([x]) = x
```

Zero and unary forms are valid with non-blocking diagnostics.

When Low, every Low input is a blocker. When High, every input is a supporter.

## 25. `Any`

```text
Inputs: zero or more Levels
Output: Level
Law: High iff at least one input is High
```

```text
Any([])  = Low
Any([x]) = x
```

When High, all High inputs are supporters. When Low, all inputs being Low collectively block High.

## 26. `Parity`

```text
Inputs: zero or more Levels
Output: Level
Law: High iff an odd number of inputs are High
```

```text
Parity([])     = Low
Parity([x])    = x
Parity([a,b])  = binary XOR
```

`Parity` is not “exactly one.”

A binary `xor(a, b)` convenience maps directly to `Parity` with two inputs.

## 27. `AtLeast`

```text
Inputs: zero or more Levels
Output: Level
Parameter: non-negative threshold k
Law: High iff at least k ports are High
```

```text
AtLeast(0, ...) = High
AtLeast(k, n)   = Low when k > n
```

Constant-result configurations are valid with diagnostics.

`Exactly`, `AtMost`, and `Majority` are standard modules or conveniences.

## 28. `Select`

```text
Inputs:
  selector: Level
  when_low: Level
  when_high: Level
Output: Level
```

```text
selector Low  -> when_low
selector High -> when_high
```

Both branches are part of the static current-reaction dependency signature. The unselected branch is structurally influential but absent from current causal support.

`Select` is primitive because it preserves direct branch-selection semantics for explanations, inspection, and graph views.

---

# Part III — Pulse combinational nodes

## 29. Catalogue

```text
Merge
Coalesce
Zip
```

## 30. `Merge`

```text
Inputs: zero or more Pulse
Output: Pulse
Law: output count is the sum of all input counts
```

```text
Merge([])        = 0
Merge([3])       = 3
Merge([2,4,1])   = 7
```

Zero and unary forms are valid with diagnostics.

Causality records grouped contribution counts without inventing pulse order.

## 31. `Coalesce`

```text
Inputs: one Pulse
Output: Pulse
```

```text
0 -> 0
positive count -> 1
```

All contributing causes remain available even though output multiplicity becomes one.

A variadic `any_pulse` convenience composes `Merge` and `Coalesce`.

## 32. `Zip`

```text
Inputs: one or more Pulse
Output: Pulse
Law: output count is the minimum input count
```

Each output occurrence represents one simultaneous group containing one occurrence from every input.

```text
Zip([2,3])    = 2
Zip([4,4,4])  = 4
Zip([5,0,2])  = 0
```

Unary `Zip` is valid with a redundancy warning.

Zero-input `Zip` is invalid because no finite natural multiplicity exists.

`Zip` does not retain unmatched pulses across reactions.

---

# Part IV — Level-controlled pulse nodes

## 33. Catalogue

```text
PulseGate
PulseSelect
PulseRoute
```

A persistent level may control current-reaction pulse flow. A pulse cannot combinationally create persistent level state.

## 34. `PulseGate`

```text
Inputs:
  pulses: Pulse
  enable: Level
Output: Pulse
```

```text
enable High -> pass complete count
enable Low  -> output zero
```

The fully settled current enable level controls the current pulse batch.

High is the sole canonical enabled polarity. Low-enabled behavior uses explicit inversion.

Inspection exposes enable level, input count, passed count, suppressed count, and recent pass or suppression.

## 35. `PulseSelect`

```text
Inputs:
  selector: Level
  when_low: Pulse
  when_high: Pulse
Output: Pulse
```

The output receives the selected branch’s complete count. The other branch is suppressed and not remembered.

Both pulse branches are part of the static current-reaction dependency signature. Explanations identify selected and suppressed contributions.

## 36. `PulseRoute`

```text
Inputs:
  selector: Level
  pulses: Pulse
Outputs:
  when_low: Pulse
  when_high: Pulse
```

The complete input count goes to exactly one output according to the settled selector.

---

# Part V — Transition-sensitive and stateful nodes

## 37. Catalogue

```text
RisingEdge
FallingEdge
AnyEdge
Toggle
PulseSetResetLatch
LevelSetResetLatch
SampleHold
```

Counters and general finite-state machines are excluded from the initial primitive core.

## 38. Edge initialization

Edge detectors use one explicit policy:

```text
Baseline
  Previous observation begins unestablished.
  The first settled observation establishes it without emitting.

Assume(initial_level)
  The configured level acts as the previous observation.
  The first settled observation is compared normally and may emit.
```

There is no hidden default.

On every reaction, an edge detector compares the settled current input against the previous observation, produces its current pulse output, and proposes the current input as the next remembered observation.

Changing detector kind may preserve an established remembered level. Changing initialization policy does not retroactively alter an already established observation.

## 39. `RisingEdge`

```text
Input: Level
Output: Pulse
State: previous observed level or unestablished baseline
Current output law: Low -> High emits one
Successor law: remember settled current input
```

All other observations emit zero.

## 40. `FallingEdge`

```text
Input: Level
Output: Pulse
State: previous observed level or unestablished baseline
Current output law: High -> Low emits one
Successor law: remember settled current input
```

All other observations emit zero.

## 41. `AnyEdge`

```text
Input: Level
Output: Pulse
State: previous observed level or unestablished baseline
Current output law: either transition emits one
Successor law: remember settled current input
```

Repeating the same level emits zero.

## 42. `Toggle`

```text
Input:
  toggle: Pulse
Output: Level
State: stored level
Parameter: explicit initial level
```

Given previous state `s` and pulse count `n`:

```text
n even -> current output = s; successor state = s
n odd  -> current output = not s; successor state = not s
```

The resulting current output is available downstream in the same reaction. State commits once after settlement.

The node retains the cause of the latest inversion. Even-count batches may contribute to current explanation without establishing a new inversion cause.

Changing the initial-level parameter preserves current stored state unless migration explicitly resets it.

The primitive has no optional reset port. Resettable toggle behavior is a named standard module or separately specified future fixed-shape node.

## 43. `PulseSetResetLatch`

```text
Inputs:
  set: Pulse
  reset: Pulse
Output: Level
State: stored level
Parameters:
  explicit initial level
  conflict policy
```

Positive count means the corresponding control is active. Multiplicity beyond presence is irrelevant.

For previous state `s`:

```text
set present, reset absent -> current output High; successor High
set absent, reset present -> current output Low; successor Low
both absent               -> current output s; successor s
both present              -> apply conflict policy
```

Policies:

```text
SetDominant
  current output High; successor High

ResetDominant
  current output Low; successor Low

RetainAndDiagnose
  current output previous state; successor previous state;
  emit one transient conflict diagnostic for this reaction

RejectTransaction
  reject the containing transaction atomically
```

A pulse conflict is reaction-scoped. Because pulses do not remain asserted, separate later conflicts are separate diagnostic occurrences rather than continuation of one persistent episode.

A persistent fault state is not part of this primitive.

Changing conflict policy preserves stored level unless an explicit migration rule says otherwise.

## 44. `LevelSetResetLatch`

```text
Inputs:
  set: Level
  reset: Level
Output: Level
State: stored level
Parameters:
  explicit initial level
  conflict policy
```

For previous state `s`:

```text
set High, reset Low -> current output High; successor High
set Low, reset High -> current output Low; successor Low
both Low            -> current output s; successor s
both High           -> apply policy
```

Policies are:

```text
SetDominant
ResetDominant
RetainAndDiagnose
RejectTransaction
```

They have the same state outcomes as for `PulseSetResetLatch`.

Controls use current settled levels and remain asserted across reactions while `High`.

Under `RetainAndDiagnose`, continuous `set = High` and `reset = High` is a persistent diagnostic episode. It begins when the conflict first settles, remains inspectable without repeated unchanged warnings, materially changes if its evidence changes, and resolves when the conflict ends or the owning semantics are removed.

Changing conflict policy preserves stored level but may begin, resolve, or reclassify the active conflict episode according to the new policy.

## 45. `SampleHold`

```text
Inputs:
  value: Level
  sample: Pulse
Output: Level
State: stored level
Parameter: explicit initial level
```

For previous state `s`:

```text
sample count = 0 -> current output s; successor s
sample count > 0 -> current output settled value; successor settled value
```

Multiple simultaneous sample pulses are equivalent to one. Their contributing causes remain available.

The sampled value is visible downstream in the same reaction. State commits once after settlement.

Changing the initial-level parameter preserves stored state unless migration explicitly resets it.

The primitive has no optional reset port. Resettable behavior is a named standard module or separately specified future fixed-shape node.

## 46. Counters

Counters are deferred because integer state introduces a new value domain and unresolved threshold, overflow, saturation, wrapping, reset, and signal-output semantics.

They may later become a standard module, a carefully specified primitive family, or part of a constrained state-machine extension.

---

# Part VI — Temporal nodes

## 47. Catalogue

```text
PulseDelay
TransportDelay
InertialDelay
Periodic
```

Ordinary durations use positive spans and schedule strictly future work.

Immediate behavior is produced directly in the current reaction rather than by scheduling an event at the current time.

## 48. Pending events

Every pending event exposes:

- stable event identity;
- owner node;
- deadline;
- event kind;
- semantic payload;
- multiplicity where applicable;
- originating logical time;
- originating cause;
- revision context;
- cancellation and migration policy.

Equivalent events may be aggregated internally only if an associative and commutative law preserves complete observable behavior and inspection and explanation preserve their causal facts.

Events due at one time join one simultaneous unordered due-event batch.

Cancellation is explicit and inspectable.

## 49. Exact-deadline principle

A pending obligation due at time `T` is evaluated from temporal state entering the reaction at `T`.

Same-time current input modifies or suppresses that due obligation only where the node’s semantics explicitly declare a current-reaction dependency.

Consequently:

```text
PulseDelay       due pulses emit regardless of new pulses at T
TransportDelay   due transitions mature regardless of new input at T
InertialDelay    a candidate surviving to T matures at T
Periodic         settled enable at T may suppress or permit a due boundary
```

## 50. Temporal initialization

`PulseDelay` begins with no pending pulse groups.

`TransportDelay` and `InertialDelay` each accept one explicit initial level. That level initializes both remembered input and output before their first reaction.

The primitive forms do not permit an initial input/output mismatch.

If the first settled input differs from the declared initial level:

- `TransportDelay` keeps the initial output and schedules the differing transition for `T₀ + delay`;
- `InertialDelay` keeps the initial output and creates a candidate for `T₀ + delay`.

Neither node changes current output immediately from its current input.

A fresh `Periodic` begins with no phase anchor, no prior enabled state, and no next deadline.

## 51. `PulseDelay`

```text
Input: Pulse
Output: Pulse
Parameter: NonZeroSpan<D>
State: pending pulse groups
```

Every occurrence received at `T` is reproduced at `T + delay`.

A pulse group due at `T` emits its complete multiplicity regardless of new pulses arriving at `T`. New pulses schedule strictly future groups and have no instantaneous path to current output.

Groups sharing one deadline may sum multiplicities while preserving grouped causal contributors.

On duration change, the standard compatibility rule is:

```text
existing pending deadlines are preserved
new duration applies only to future input pulses
```

A patch may request another explicit migration policy. Every affected group must be classified as preserved, recomputed, transformed, canceled, or rejected.

## 52. `TransportDelay`

```text
Input: Level
Output: Level
Parameter: NonZeroSpan<D>
State:
  remembered input
  output level
  queued transitions
Parameter:
  explicit initial level
```

Every settled input transition at originating time `S` queues its target level for deadline `S + delay`. Short-lived input transitions are preserved.

On every reaction, remembered input proposes the settled current input as its successor value. A difference from the previous remembered input is the transition that creates new queued work.

Current input affects remembered input and future scheduling but has no instantaneous dependency on current output.

A transition due at `T` matures regardless of a new input transition at `T`. The new input transition may queue a later transition.

### 52.1 Same-deadline transition batches

Duration changes or explicit migration may cause several queued transitions to mature at one deadline.

The settled output for that reaction is the target of the matured transition with the greatest originating logical time.

This is semantic chronological precedence, not pending-event iteration order. Only the final settled output is observable; no same-time intermediate output transitions exist.

All matured transitions remain available to inspection and explanation. Earlier matured transitions are superseded at that deadline by later-originating ones.

Ordinary node operation produces at most one queued input transition per originating reaction. A migration that would create conflicting transitions with indistinguishable originating time must explicitly resolve them or reject the patch.

### 52.2 Reconfiguration

The standard duration-change compatibility rule preserves existing queued deadlines and applies the new duration only to future input transitions.

Alternative rescheduling requires an explicit migration policy and complete event classification.

Changing input connectivity preserves compatible queued transitions. Ordinary topology-induced reevaluation compares the newly settled input with remembered input and may schedule a new future transition.

## 53. `InertialDelay`

```text
Input: Level
Output: Level
Parameter: NonZeroSpan<D>
State:
  remembered input
  output level
  optional candidate transition
Parameter:
  explicit initial level
```

On every reaction, remembered input proposes the settled current input as its successor value.

A settled input transition at time `S` creates a candidate with target equal to the new input and deadline `D = S + delay` when that target differs from the output that will remain after due obligations at `S` are applied. If the settled input equals that output, no candidate is required.

The candidate succeeds if the target remains the input throughout:

```text
[S, D)
```

A contradictory input change strictly before `D` cancels the candidate.

A contradictory input change exactly at `D`:

- does not cancel the matured transition;
- allows the matured target to determine current output at `D`;
- may establish a new opposite candidate beginning at `D`.

Current input therefore has no instantaneous dependency on current output.

If input changes again before a candidate matures, the existing candidate is canceled. A new candidate is established from that reaction time exactly when the newly settled input differs from the output that remains after current due obligations are applied.

Inspection exposes target, originating time, deadline, elapsed span, remaining span, originating cause, and recent cancellation or maturation.

Changing duration while a candidate exists requires one explicit migration choice, for example:

```text
PreserveDeadline
RecomputeFromOrigin
RestartFromPatchTime
CancelCandidate
RejectPatch
```

Each choice must define the candidate’s resulting origin, deadline, causal ancestry, and state-loss consequence.

`Debounce` is a primitive alias for `InertialDelay`, not a distinct semantic node.

## 54. `Periodic`

```text
Input:
  enable: Level
Output: Pulse
Parameters:
  NonZeroSpan<D> period
  first-emission policy
  re-enable phase policy
State:
  optional phase anchor
  previous enabled state
  optional next eligible boundary
```

### 54.1 Initial enable and phase anchor

A fresh instance has no phase anchor.

The first settled `High` enable establishes the initial anchor at the current reaction time.

- `Immediate` may emit once in that reaction.
- `AfterFirstPeriod` schedules the first boundary one period later.
- `PreservePhase` has no earlier phase to preserve until an anchor has first been established.

A fresh instance that remains disabled stays anchorless. Every reaction proposes the settled current enable value as the next remembered enabled state.

### 54.2 Periodic boundaries

Once an anchor exists, periodic boundaries are:

```text
anchor + n * period
```

for non-negative integer `n` as admitted by the active first-emission and re-enable policies.

Each eligible boundary is a distinct logical reaction and emits one pulse at that boundary. Boundaries at different logical times MUST NOT be collapsed into one later pulse count.

If other pulse causes reach the same output at the same boundary time, ordinary same-reaction pulse algebra determines the combined multiplicity.

### 54.3 First-emission policy

```text
Immediate
AfterFirstPeriod
```

Under `RestartPhase`:

- `Immediate` emits once in the enabling reaction;
- `AfterFirstPeriod` emits one full period after enable.

Under `PreservePhase` with an existing anchor:

- enabling exactly on a phase boundary permits that boundary under `Immediate`;
- `AfterFirstPeriod` waits for the next boundary strictly after enable;
- enabling between boundaries waits for the next boundary under either policy.

### 54.4 Re-enable phase policy

```text
RestartPhase
PreservePhase
```

`RestartPhase` establishes a new anchor on every disabled-to-enabled transition.

`PreservePhase` keeps the established anchor while disabled. Phase boundaries continue conceptually, but disabled boundaries emit nothing and are never replayed.

### 54.5 Exact-boundary enable behavior

A due boundary has an instantaneous dependency on settled current `enable`.

If `enable` settles `Low` exactly on the boundary, that boundary is suppressed.

If a disabled node settles `High` exactly on a preserved-phase boundary:

- `Immediate` permits that boundary;
- `AfterFirstPeriod` waits for the next boundary strictly after enable.

### 54.6 Large time jumps

When a caller transaction advances across several eligible boundaries, the processor evaluates them chronologically as separate internal reactions.

The observable result is the same sequence of one-pulse emissions at their actual boundary times that stepwise advancement would have produced.

Disabled boundaries do not accumulate missed pulses.

### 54.7 Reconfiguration

Changing period, first-emission policy, or phase policy while a schedule or anchor exists requires an explicit migration choice, such as:

```text
PreserveNextDeadline
ReanchorAtPatchTime
RecomputeFromExistingAnchor
CancelSchedule
RejectPatch
```

Each policy must define the resulting anchor, next boundary, treatment of a boundary due at patch time, causal ancestry, and any reported state or schedule loss.

The library MUST NOT infer a policy from implementation convenience.

## 55. Temporal standard modules

Initially non-primitive:

```text
FixedOneShot
RetriggerableOneShot
Timeout
Watchdog
PulseStretcher
RateLimiter
```

`Debounce` is a direct alias of `InertialDelay`.

---

# Part VII — Boundaries and utilities

## 56. External inputs and outputs

External inputs and outputs are network boundaries, not ordinary nodes.

```rust
ExternalInputKey<Level>
ExternalInputKey<Pulse>
ExternalOutputKey<Level>
ExternalOutputKey<Pulse>
```

Inputs receive transaction-supplied values. Outputs expose internal signals without transforming or consuming them.

Several external outputs may expose one signal without a splitter.

## 57. Output establishment

A level output has no prior observable value before machine initialization.

Its first committed settled value produces:

```text
LevelEstablished {
    output,
    value,
    at,
    cause,
    revision,
}
```

Subsequent changes produce:

```text
LevelChanged {
    output,
    from,
    to,
    at,
    cause,
    revision,
}
```

The same establishment rule applies to an external level output added by reconfiguration.

Removing an output is a topology consequence, not a fabricated transition to `Low` or to an invented no-signal value.

Pulse outputs produced during machine initialization or a topology-induced reaction are ordinary pulse events when the node equations genuinely produce them.

## 58. Sources

Manual triggers, switches, observations, and caller-authored absolute schedules enter through external inputs.

`Periodic` represents network-owned recurring schedule.

There is no generic absolute-time calendar node.

## 59. Probes and named signals

Probes are inspection constructs, not semantic nodes.

Adding or removing a probe MUST NOT change behavior, fingerprints, state migration, propagation, explanations, snapshots, or semantic digests.

Intermediate signals may receive names and metadata without identity-buffer nodes.

## 60. Assertions, faults, and unused signals

Assertions belong to future verification or contract layers.

Library errors are diagnostics, not signal outputs.

A node exposes a fault signal only when fault is part of its actual signal semantics.

There is no sink primitive. An intentionally unused signal remains unconnected.

---

# Part VIII — Inspection, explanation, and migration obligations

## 61. Node inspection

Every built-in node exposes, where applicable:

```text
node kind and parameters
settled current inputs and outputs
previous committed state
proposed or latest committed successor state
latest transition cause
current supporters and blockers
pending events and next deadline
active diagnostic episodes
recent transient diagnostics
revision and logical time
```

Before machine initialization, only structural definition and declared initial-state fields are available.

## 62. Temporal inspection

Every temporal node additionally exposes:

```text
pending event identities
origins and deadlines
payloads or target levels
multiplicity
cancellation and migration policy
causal ancestry
next eligible deadline
```

Canceled work does not appear as pending, though optional retained history may describe it.

## 63. Reconfiguration obligations by family

### 63.1 Combinational nodes

Compatible identity preserves no internal state because none exists. Parameter or connection changes cause ordinary reevaluation.

### 63.2 Edge detectors

A compatible detector-kind change may preserve an established remembered observation. A reset or incompatible schema change re-enters its declared initialization policy.

### 63.3 Boolean state nodes

`Toggle`, both set/reset latches, and `SampleHold` may preserve their stored level across compatible parameter and connection changes.

Changing an initial level does not alter preserved state.

Changing latch conflict policy preserves stored level but reevaluates any active level-conflict diagnostic episode under the new policy.

Changing between pulse-controlled and level-controlled latch kinds is not automatically compatible because port semantics differ; preservation requires an explicit migration rule or otherwise resets or rejects.

### 63.4 Temporal nodes

Compatible survival requires explicit classification of temporal state and every pending event.

Removing a temporal node cancels pending work only when policy allows reported loss; otherwise the patch rejects.

Changing containing module or external connections does not by itself cancel compatible pending work.

### 63.5 Diagnostic episodes

A preserved subject preserves an episode only when the condition retains the same semantic identity.

Migration may preserve, transform, resolve, or terminate an episode explicitly. Dense-slot reuse must never transfer an episode to another subject.

---

# Part IX — Catalogue classification

## 64. Built-in primitives

### Level combinational

```text
Constant
Not
All
Any
Parity
AtLeast
Select
```

### Pulse combinational

```text
Merge
Coalesce
Zip
```

### Level-controlled pulse

```text
PulseGate
PulseSelect
PulseRoute
```

### Transition-sensitive and stateful

```text
RisingEdge
FallingEdge
AnyEdge
Toggle
PulseSetResetLatch
LevelSetResetLatch
SampleHold
```

### Temporal

```text
PulseDelay
TransportDelay
InertialDelay
Periodic
```

## 65. Standard modules and conveniences

Potential standard modules or constructors include:

```text
xor
nand
nor
xnor
level gate
exactly
at most
majority
all equal

any pulse
pulse multiplication
pulse cap
pulse parity

gate when Low
multi-way routing and selection

resettable toggle
resettable sample-and-hold
fixed one-shot
retriggerable one-shot
timeout
watchdog
pulse stretcher
rate limiter
```

## 66. Convenience visibility

Conveniences are classified explicitly.

### 66.1 Primitive aliases

A convenience that maps exactly to one primitive creates that primitive.

Examples:

```text
xor(a,b)         -> two-input Parity
debounce(x,span) -> InertialDelay
```

Inspection exposes the canonical primitive kind. The convenience name may remain as authoring metadata.

### 66.2 Standard modules

A convenience composed from several primitives creates a named module instance.

Examples may include `nand`, `majority`, `resettable toggle`, `watchdog`, and `retriggerable one-shot`.

The module remains visible in inspection, hierarchy, diagnostics, explanations, and reconfiguration plans even if flattened internally for execution.

Documentation MUST state whether each convenience is a primitive alias, named module, metadata-only operation, or non-semantic builder operation.

## 67. Explicitly absent primitives

The initial core does not include:

```text
splitter
identity buffer
pulse constant
manual source
absolute scheduled source
terminal sink
probe node
assertion node
generic timer
generic latch
counter
general state machine
unrestricted custom node
arbitrary signal payload
```

---

# Part X — Requirements for every node kind

## 68. Required specification fields

Every built-in or future node kind defines:

```text
name
category
input ports
output ports
parameters
initial state
first-reaction initialization behavior
current-reaction dependency signature
current output law
proposed successor-state law
simultaneous-input law
pulse multiplicity law
temporal law where applicable
due-obligation dependency
exact-deadline law
same-deadline batch law
strictly-future scheduling law
inspection schema
current explanation
why-not explanation
transition causality
pending-event representation
reconfiguration compatibility
state-dependent migration
pending-event migration
topology-induced first-reaction behavior
invalid configurations
non-blocking diagnostics
transient diagnostic policy
diagnostic-episode identity and lifecycle
```

A kind for which a field does not apply must state that explicitly rather than leave it ambiguous.

## 69. Naming

Names describe actual semantics rather than ambiguous hardware terminology.

Preferred:

```text
Parity
AtLeast
Zip
PulseRoute
TransportDelay
InertialDelay
```

Avoid vague names such as `PulseAnd`, `Timer`, `Latch`, `Delay`, or an unqualified `Gate`.

Familiar convenience names are acceptable where their mapping is unambiguous.

## 70. Orthogonality

Every primitive has one clear reason to exist.

Overlapping primitives differ along explicit semantic dimensions.

Policy-heavy operations should be decomposed into smaller primitives and named modules.

The catalogue optimizes for precise composition, comprehensible inspection, tractable reconfiguration, clear explanations, and minimal evaluator exceptions.

---

# Summary

The built-in language provides:

```text
a total glitch-free level algebra
a simultaneous multiplicity-preserving pulse algebra
explicit level-controlled pulse routing
a compact synchronous Boolean state system
a precise caller-driven temporal system
```

Its defining laws are:

```text
node semantics are defined per logical reaction
only settled reaction values are observable
simultaneous same-time events form unordered batches
distinct logical times remain distinct reactions
pulse multiplicity is non-negative and explicit
fan-out is intrinsic
state, output, pending work, diagnostic episodes, and history are distinct
machine initialization is explicit
Low is distinct from absence of established signal state
stateful nodes expose current results before atomic successor-state commit
stateful nodes are not automatically causality barriers
current-reaction dependencies are statically acyclic
temporal work is strictly future
exact-deadline behavior is node-specific and explicit
periodic boundaries remain separate events at their actual times
current explanation and latest transition are distinct
persistent diagnostics are semantic episodes
state loss and event migration are never implicit
new level outputs are established rather than changed from a fabricated default
boundaries, tooling, and diagnostics are not evaluator nodes
primitive aliases and named modules remain distinguishable
```

The catalogue is deliberately compact, but not minimal for its own sake. A node is primitive where first-class treatment improves semantic precision, causality, inspection, temporal correctness, or state preservation. Everything else should first be expressed through reusable modules.
