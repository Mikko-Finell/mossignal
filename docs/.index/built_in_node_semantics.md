## docs/specs/built_in_node_semantics.md
- ``mossignal` Built-in Node Semantics` [1-20]
  Preview: **Status:** Consolidated design specification, version 2 **Defines:** Built-in nodes, boundary endpoints, shared laws, inspection, explanation, diagnostics, initialization, and reconfiguration **Excluded:** Processor implementation, testing policy, model checking, contracts, serialized wire formats, and application-specific modules This specification defines the built-in semantic language of `mossignal`.
  Symbols: `mossignal`

- ``mossignal` Built-in Node Semantics > 1. Purpose` [9-20]
  Preview: This specification defines the built-in semantic language of `mossignal`.
  Symbols: `mossignal`

- `Part I — Shared semantic rules` [21-349]
  Preview: The core kinds are: A level is persistent binary signal state.
  Symbols: `Low`, `Unknown`, `Unset`, `LogicLevel`, `PulseCount`, `ZERO`, `ONE`, `High`, `Constant`, `Not`, `Parity`, `AtLeast`, `Select`, `Merge`, `Coalesce`, `Zip`, `PulseGate`, `PulseSelect`, `PulseRoute`, `RisingEdge`, `FallingEdge`, `AnyEdge`, `Toggle`, `PulseSetResetLatch`, `LevelSetResetLatch`, `SampleHold`, `PulseDelay`, `TransportDelay`, `InertialDelay`, `Periodic`, `Reset`
  Normative: MUST NOT 5, MUST 2, SHOULD 4

- `Part I — Shared semantic rules > 2. Signal kinds` [23-58]
  Preview: The core kinds are: A level is persistent binary signal state.
  Symbols: `Low`, `Unknown`, `Unset`, `LogicLevel`, `PulseCount`, `ZERO`, `ONE`, `High`
  Normative: MUST 1

- `Part I — Shared semantic rules > 2. Signal kinds > 2.1 Level` [32-46]
  Preview: A level is persistent binary signal state.
  Symbols: `Low`, `Unknown`, `Unset`, `LogicLevel`

- `Part I — Shared semantic rules > 2. Signal kinds > 2.2 Pulse` [47-58]
  Preview: A pulse is a discrete occurrence at one logical time.
  Symbols: `PulseCount`, `ZERO`, `ONE`, `High`
  Normative: MUST 1

- `Part I — Shared semantic rules > 3. Reactions and transactions` [59-66]
  Preview: A **reaction** is one complete settlement at one logical time.

- `Part I — Shared semantic rules > 4. Node categories` [67-76]
  Preview: A node is: - **Combinational:** stateless and non-temporal; output depends only on settled current inputs.

- `Part I — Shared semantic rules > 5. Synchronous transducer model` [77-111]
  Preview: Every built-in node is interpreted as a deterministic synchronous transducer.

- `Part I — Shared semantic rules > 6. Glitch-free semantics` [112-137]
  Preview: Only settled pre-reaction and post-reaction values are semantically observable.
  Symbols: `Low`
  Normative: MUST NOT 1

- `Part I — Shared semantic rules > 7. Simultaneous batches` [138-147]
  Preview: All stimuli at one logical time form one unordered batch.
  Normative: MUST NOT 2

- `Part I — Shared semantic rules > 8. Current-time controls` [148-153]
  Preview: A node controlled by a level uses that level’s fully settled value for the current reaction.

- `Part I — Shared semantic rules > 9. Current-reaction dependency signatures` [154-189]
  Preview: Every node kind defines a conservative static signature describing which current inputs may affect which current outputs within one reaction.
  Symbols: `Constant`, `Not`, `Parity`, `AtLeast`, `Select`, `Merge`, `Coalesce`, `Zip`, `PulseGate`, `PulseSelect`, `PulseRoute`, `RisingEdge`, `FallingEdge`, `AnyEdge`, `Toggle`, `PulseSetResetLatch`, `LevelSetResetLatch`, `SampleHold`, `PulseDelay`, `TransportDelay`, `InertialDelay`, `Periodic`
  Normative: MUST 1

- `Part I — Shared semantic rules > 10. Pulse fan-out` [190-197]
  Preview: Pulses are information, not consumable tokens.

- `Part I — Shared semantic rules > 11. Pulse multiplicity` [198-203]
  Preview: Every pulse-consuming node must explicitly preserve, sum, coalesce, group, suppress, bound, or otherwise transform multiplicity.
  Normative: MUST NOT 1

- `Part I — Shared semantic rules > 12. Distinct state concepts` [204-217]
  Preview: Specifications must distinguish: The latest pulse time is history, not persistent pulse state.

- `Part I — Shared semantic rules > 13. Machine and node initialization` [218-236]
  Preview: A newly spawned machine is uninitialized.

- `Part I — Shared semantic rules > 14. Initial state declarations` [237-246]
  Preview: Every stateful node and level-producing temporal node has an explicit initial condition.
  Normative: SHOULD 1

- `Part I — Shared semantic rules > 15. Variadic inputs` [247-252]
  Preview: Every variadic input slot has stable port identity independent from display order.

- `Part I — Shared semantic rules > 16. Duplicate sources` [253-264]
  Preview: Connecting one upstream output to multiple ports of one node is valid and counted once per port.
  Normative: SHOULD 1

- `Part I — Shared semantic rules > 17. Total semantics` [265-281]
  Preview: Where a natural mathematical result exists, well-typed arities and parameters SHOULD have total semantics.
  Normative: SHOULD 1

- `Part I — Shared semantic rules > 18. Explanations and causes` [282-298]
  Preview: Current justification is distinct from the most recent transition cause.
  Normative: SHOULD 1

- `Part I — Shared semantic rules > 19. Diagnostic occurrences and episodes` [299-318]
  Preview: A transient condition produces a diagnostic occurrence for the reaction in which it exists.
  Normative: MUST NOT 1

- `Part I — Shared semantic rules > 20. Shared reconfiguration rules` [319-349]
  Preview: Reconfiguration is evaluated against the actual state reached at the patch’s effective time.
  Symbols: `Reset`

- `Part II — Level combinational nodes` [350-477]
  Preview: `net.low()` and `net.high()` may be typed conveniences.
  Symbols: `Parity`, `net.low()`, `net.high()`, `xor(a, b)`, `Exactly`, `AtMost`, `Majority`, `Select`

- `Part II — Level combinational nodes > 21. Catalogue` [352-363]

- `Part II — Level combinational nodes > 22. `Constant`` [364-378]
  Preview: `net.low()` and `net.high()` may be typed conveniences.
  Symbols: `net.low()`, `net.high()`

- `Part II — Level combinational nodes > 23. `Not`` [379-388]
  Preview: Explanation identifies the input and its opposite output.

- `Part II — Level combinational nodes > 24. `All`` [389-405]
  Preview: Zero and unary forms are valid with non-blocking diagnostics.

- `Part II — Level combinational nodes > 25. `Any`` [406-420]
  Preview: When High, all High inputs are supporters.

- `Part II — Level combinational nodes > 26. `Parity`` [421-438]
  Preview: `Parity` is not “exactly one.” A binary `xor(a, b)` convenience maps directly to `Parity` with two inputs.
  Symbols: `Parity`, `xor(a, b)`

- `Part II — Level combinational nodes > 27. `AtLeast`` [439-456]
  Preview: Constant-result configurations are valid with diagnostics.
  Symbols: `Exactly`, `AtMost`, `Majority`

- `Part II — Level combinational nodes > 28. `Select`` [457-477]
  Preview: Both branches are part of the static current-reaction dependency signature.
  Symbols: `Select`

- `Part III — Pulse combinational nodes` [478-545]
  Preview: Zero and unary forms are valid with diagnostics.
  Symbols: `Zip`, `any_pulse`, `Merge`, `Coalesce`

- `Part III — Pulse combinational nodes > 29. Catalogue` [480-487]

- `Part III — Pulse combinational nodes > 30. `Merge`` [488-505]
  Preview: Zero and unary forms are valid with diagnostics.

- `Part III — Pulse combinational nodes > 31. `Coalesce`` [506-521]
  Preview: All contributing causes remain available even though output multiplicity becomes one.
  Symbols: `any_pulse`, `Merge`, `Coalesce`

- `Part III — Pulse combinational nodes > 32. `Zip`` [522-545]
  Preview: Each output occurrence represents one simultaneous group containing one occurrence from every input.
  Symbols: `Zip`

- `Part IV — Level-controlled pulse nodes` [546-606]
  Preview: A persistent level may control current-reaction pulse flow.

- `Part IV — Level-controlled pulse nodes > 33. Catalogue` [548-557]
  Preview: A persistent level may control current-reaction pulse flow.

- `Part IV — Level-controlled pulse nodes > 34. `PulseGate`` [558-577]
  Preview: The fully settled current enable level controls the current pulse batch.

- `Part IV — Level-controlled pulse nodes > 35. `PulseSelect`` [578-591]
  Preview: The output receives the selected branch’s complete count.

- `Part IV — Level-controlled pulse nodes > 36. `PulseRoute`` [592-606]
  Preview: The complete input count goes to exactly one output according to the settled selector.

- `Part V — Transition-sensitive and stateful nodes` [607-823]
  Preview: Counters and general finite-state machines are excluded from the initial primitive core.
  Symbols: `s`, `n`, `PulseSetResetLatch`, `High`, `RetainAndDiagnose`, `set = High`, `reset = High`

- `Part V — Transition-sensitive and stateful nodes > 37. Catalogue` [609-622]
  Preview: Counters and general finite-state machines are excluded from the initial primitive core.

- `Part V — Transition-sensitive and stateful nodes > 38. Edge initialization` [623-642]
  Preview: Edge detectors use one explicit policy: There is no hidden default.

- `Part V — Transition-sensitive and stateful nodes > 39. `RisingEdge`` [643-654]
  Preview: All other observations emit zero.

- `Part V — Transition-sensitive and stateful nodes > 40. `FallingEdge`` [655-666]
  Preview: All other observations emit zero.

- `Part V — Transition-sensitive and stateful nodes > 41. `AnyEdge`` [667-678]
  Preview: Repeating the same level emits zero.

- `Part V — Transition-sensitive and stateful nodes > 42. `Toggle`` [679-703]
  Preview: Given previous state `s` and pulse count `n`: The resulting current output is available downstream in the same reaction.
  Symbols: `s`, `n`

- `Part V — Transition-sensitive and stateful nodes > 43. `PulseSetResetLatch`` [704-750]
  Preview: Positive count means the corresponding control is active.
  Symbols: `s`

- `Part V — Transition-sensitive and stateful nodes > 44. `LevelSetResetLatch`` [751-789]
  Preview: For previous state `s`: Policies are: They have the same state outcomes as for `PulseSetResetLatch`.
  Symbols: `s`, `PulseSetResetLatch`, `High`, `RetainAndDiagnose`, `set = High`, `reset = High`

- `Part V — Transition-sensitive and stateful nodes > 45. `SampleHold`` [790-815]
  Preview: For previous state `s`: Multiple simultaneous sample pulses are equivalent to one.
  Symbols: `s`

- `Part V — Transition-sensitive and stateful nodes > 46. Counters` [816-823]
  Preview: Counters are deferred because integer state introduces a new value domain and unresolved threshold, overflow, saturation, wrapping, reset, and signal-output semantics.

- `Part VI — Temporal nodes` [824-1132]
  Preview: Ordinary durations use positive spans and schedule strictly future work.
  Symbols: `InertialDelay`, `Immediate`, `AfterFirstPeriod`, `PreservePhase`, `TransportDelay`, `T₀ + delay`, `Debounce`, `High`, `RestartPhase`, `enable`, `PulseDelay`, `Periodic`, `T + delay`, `S + delay`, `D = S + delay`, `n`, `Low`
  Normative: MUST NOT 2

- `Part VI — Temporal nodes > 47. Catalogue` [826-838]
  Preview: Ordinary durations use positive spans and schedule strictly future work.

- `Part VI — Temporal nodes > 48. Pending events` [839-859]
  Preview: Every pending event exposes: - stable event identity; - owner node; - deadline; - event kind; - semantic payload; - multiplicity where applicable; - originating logical time; - originating cause; - revision context; - cancellation and migration policy.

- `Part VI — Temporal nodes > 49. Exact-deadline principle` [860-874]
  Preview: A pending obligation due at time `T` is evaluated from temporal state entering the reaction at `T`.

- `Part VI — Temporal nodes > 50. Temporal initialization` [875-891]
  Preview: `PulseDelay` begins with no pending pulse groups.
  Symbols: `TransportDelay`, `InertialDelay`, `T₀ + delay`, `PulseDelay`, `Periodic`

- `Part VI — Temporal nodes > 51. `PulseDelay`` [892-915]
  Preview: Every occurrence received at `T` is reproduced at `T + delay`.
  Symbols: `T + delay`

- `Part VI — Temporal nodes > 52. `TransportDelay`` [916-957]
  Preview: Every settled input transition at originating time `S` queues its target level for deadline `S + delay`.
  Symbols: `S + delay`

- `Part VI — Temporal nodes > 52. `TransportDelay` > 52.1 Same-deadline transition batches` [938-949]
  Preview: Duration changes or explicit migration may cause several queued transitions to mature at one deadline.

- `Part VI — Temporal nodes > 52. `TransportDelay` > 52.2 Reconfiguration` [950-957]
  Preview: The standard duration-change compatibility rule preserves existing queued deadlines and applies the new duration only to future input transitions.

- `Part VI — Temporal nodes > 53. `InertialDelay`` [958-1009]
  Preview: On every reaction, remembered input proposes the settled current input as its successor value.
  Symbols: `D = S + delay`, `Debounce`, `InertialDelay`

- `Part VI — Temporal nodes > 54. `Periodic`` [1010-1115]
  Preview: A fresh instance has no phase anchor.
  Symbols: `Immediate`, `AfterFirstPeriod`, `PreservePhase`, `High`, `RestartPhase`, `enable`, `n`, `Low`
  Normative: MUST NOT 2

- `Part VI — Temporal nodes > 54. `Periodic` > 54.1 Initial enable and phase anchor` [1026-1037]
  Preview: A fresh instance has no phase anchor.
  Symbols: `High`, `Immediate`, `AfterFirstPeriod`, `PreservePhase`

- `Part VI — Temporal nodes > 54. `Periodic` > 54.2 Periodic boundaries` [1038-1051]
  Preview: Once an anchor exists, periodic boundaries are: for non-negative integer `n` as admitted by the active first-emission and re-enable policies.
  Symbols: `n`
  Normative: MUST NOT 1

- `Part VI — Temporal nodes > 54. `Periodic` > 54.3 First-emission policy` [1052-1069]
  Preview: Under `RestartPhase`: - `Immediate` emits once in the enabling reaction; - `AfterFirstPeriod` emits one full period after enable.
  Symbols: `Immediate`, `AfterFirstPeriod`, `RestartPhase`, `PreservePhase`

- `Part VI — Temporal nodes > 54. `Periodic` > 54.4 Re-enable phase policy` [1070-1080]
  Preview: `RestartPhase` establishes a new anchor on every disabled-to-enabled transition.
  Symbols: `RestartPhase`, `PreservePhase`

- `Part VI — Temporal nodes > 54. `Periodic` > 54.5 Exact-boundary enable behavior` [1081-1091]
  Preview: A due boundary has an instantaneous dependency on settled current `enable`.
  Symbols: `enable`, `Low`, `High`, `Immediate`, `AfterFirstPeriod`

- `Part VI — Temporal nodes > 54. `Periodic` > 54.6 Large time jumps` [1092-1099]
  Preview: When a caller transaction advances across several eligible boundaries, the processor evaluates them chronologically as separate internal reactions.

- `Part VI — Temporal nodes > 54. `Periodic` > 54.7 Reconfiguration` [1100-1115]
  Preview: Changing period, first-emission policy, or phase policy while a schedule or anchor exists requires an explicit migration choice, such as: Each policy must define the resulting anchor, next boundary, treatment of a boundary due at patch time, causal ancestry, and any reported state or schedule loss.
  Normative: MUST NOT 1

- `Part VI — Temporal nodes > 55. Temporal standard modules` [1116-1132]
  Preview: Initially non-primitive: `Debounce` is a direct alias of `InertialDelay`.
  Symbols: `Debounce`, `InertialDelay`

- `Part VII — Boundaries and utilities` [1133-1212]
  Preview: External inputs and outputs are network boundaries, not ordinary nodes.
  Symbols: `Low`, `Periodic`
  Normative: MUST NOT 1

- `Part VII — Boundaries and utilities > 56. External inputs and outputs` [1135-1149]
  Preview: External inputs and outputs are network boundaries, not ordinary nodes.

- `Part VII — Boundaries and utilities > 57. Output establishment` [1150-1184]
  Preview: A level output has no prior observable value before machine initialization.
  Symbols: `Low`

- `Part VII — Boundaries and utilities > 58. Sources` [1185-1192]
  Preview: Manual triggers, switches, observations, and caller-authored absolute schedules enter through external inputs.
  Symbols: `Periodic`

- `Part VII — Boundaries and utilities > 59. Probes and named signals` [1193-1200]
  Preview: Probes are inspection constructs, not semantic nodes.
  Normative: MUST NOT 1

- `Part VII — Boundaries and utilities > 60. Assertions, faults, and unused signals` [1201-1212]
  Preview: Assertions belong to future verification or contract layers.

- `Part VIII — Inspection, explanation, and migration obligations` [1213-1285]
  Preview: Every built-in node exposes, where applicable: Before machine initialization, only structural definition and declared initial-state fields are available.
  Symbols: `Toggle`, `SampleHold`

- `Part VIII — Inspection, explanation, and migration obligations > 61. Node inspection` [1215-1233]
  Preview: Every built-in node exposes, where applicable: Before machine initialization, only structural definition and declared initial-state fields are available.

- `Part VIII — Inspection, explanation, and migration obligations > 62. Temporal inspection` [1234-1249]
  Preview: Every temporal node additionally exposes: Canceled work does not appear as pending, though optional retained history may describe it.

- `Part VIII — Inspection, explanation, and migration obligations > 63. Reconfiguration obligations by family` [1250-1285]
  Preview: Compatible identity preserves no internal state because none exists.
  Symbols: `Toggle`, `SampleHold`

- `Part VIII — Inspection, explanation, and migration obligations > 63. Reconfiguration obligations by family > 63.1 Combinational nodes` [1252-1255]
  Preview: Compatible identity preserves no internal state because none exists.

- `Part VIII — Inspection, explanation, and migration obligations > 63. Reconfiguration obligations by family > 63.2 Edge detectors` [1256-1259]
  Preview: A compatible detector-kind change may preserve an established remembered observation.

- `Part VIII — Inspection, explanation, and migration obligations > 63. Reconfiguration obligations by family > 63.3 Boolean state nodes` [1260-1269]
  Preview: `Toggle`, both set/reset latches, and `SampleHold` may preserve their stored level across compatible parameter and connection changes.
  Symbols: `Toggle`, `SampleHold`

- `Part VIII — Inspection, explanation, and migration obligations > 63. Reconfiguration obligations by family > 63.4 Temporal nodes` [1270-1277]
  Preview: Compatible survival requires explicit classification of temporal state and every pending event.

- `Part VIII — Inspection, explanation, and migration obligations > 63. Reconfiguration obligations by family > 63.5 Diagnostic episodes` [1278-1285]
  Preview: A preserved subject preserves an episode only when the condition retains the same semantic identity.

- `Part IX — Catalogue classification` [1286-1421]
  Preview: Potential standard modules or constructors include: Conveniences are classified explicitly.
  Symbols: `nand`, `majority`, `resettable toggle`, `watchdog`, `retriggerable one-shot`
  Normative: MUST 1

- `Part IX — Catalogue classification > 64. Built-in primitives` [1288-1338]

- `Part IX — Catalogue classification > 64. Built-in primitives > Level combinational` [1290-1301]

- `Part IX — Catalogue classification > 64. Built-in primitives > Pulse combinational` [1302-1309]

- `Part IX — Catalogue classification > 64. Built-in primitives > Level-controlled pulse` [1310-1317]

- `Part IX — Catalogue classification > 64. Built-in primitives > Transition-sensitive and stateful` [1318-1329]

- `Part IX — Catalogue classification > 64. Built-in primitives > Temporal` [1330-1338]

- `Part IX — Catalogue classification > 65. Standard modules and conveniences` [1339-1371]
  Preview: Potential standard modules or constructors include:

- `Part IX — Catalogue classification > 66. Convenience visibility` [1372-1398]
  Preview: Conveniences are classified explicitly.
  Symbols: `nand`, `majority`, `resettable toggle`, `watchdog`, `retriggerable one-shot`
  Normative: MUST 1

- `Part IX — Catalogue classification > 66. Convenience visibility > 66.1 Primitive aliases` [1376-1388]
  Preview: A convenience that maps exactly to one primitive creates that primitive.

- `Part IX — Catalogue classification > 66. Convenience visibility > 66.2 Standard modules` [1389-1398]
  Preview: A convenience composed from several primitives creates a named module instance.
  Symbols: `nand`, `majority`, `resettable toggle`, `watchdog`, `retriggerable one-shot`
  Normative: MUST 1

- `Part IX — Catalogue classification > 67. Explicitly absent primitives` [1399-1421]
  Preview: The initial core does not include:

- `Part X — Requirements for every node kind` [1422-1493]
  Preview: Every built-in or future node kind defines: A kind for which a field does not apply must state that explicitly rather than leave it ambiguous.
  Symbols: `PulseAnd`, `Timer`, `Latch`, `Delay`, `Gate`

- `Part X — Requirements for every node kind > 68. Required specification fields` [1424-1462]
  Preview: Every built-in or future node kind defines: A kind for which a field does not apply must state that explicitly rather than leave it ambiguous.

- `Part X — Requirements for every node kind > 69. Naming` [1463-1481]
  Preview: Names describe actual semantics rather than ambiguous hardware terminology.
  Symbols: `PulseAnd`, `Timer`, `Latch`, `Delay`, `Gate`

- `Part X — Requirements for every node kind > 70. Orthogonality` [1482-1493]
  Preview: Every primitive has one clear reason to exist.

- `Summary` [1494-1532]
  Preview: The built-in language provides: Its defining laws are: The catalogue is deliberately compact, but not minimal for its own sake.
