## docs/specs/standard_module_catalogue_spec.md
- ``mossignal` Standard Module Catalogue` [1-66]
  Preview: **Status:** Design specification, version 2 **Defines:** The standard-module system; classification of conveniences; canonical catalogue identity and expansion; the complete initial standard-module inventory; typed and dynamic construction APIs; inspection, explanation, diagnostics, persistence, reconfiguration, migration, and verification requirements **Does not define:** New primitive node kinds, new signal kinds, the exhaustive global diagnostic catalogue, unrestricted user-defined automata, editor interaction design, application-domain modules, or future temporal capabilities absent from the current primitive language This specification defines the named, versioned, reusable behaviors supplied by the `mossignal` standard module catalogue.
  Symbols: `mossignal`, `ModuleDef<D>`, `ModuleDef`, `ModuleBuilder`, `ModuleInstanceKey`, `ModuleInputKey<S>`, `ModuleOutputKey<S>`, `AddedModuleInstance`
  Normative: MUST NOT 1, MUST 1, SHOULD NOT 1, SHOULD 1, MAY 1

- ``mossignal` Standard Module Catalogue > 1. Purpose` [9-36]
  Preview: This specification defines the named, versioned, reusable behaviors supplied by the `mossignal` standard module catalogue.
  Symbols: `mossignal`

- ``mossignal` Standard Module Catalogue > 2. Normative language` [37-48]
  Preview: The terms **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.
  Symbols: `ModuleDef<D>`
  Normative: MUST NOT 1, MUST 1, SHOULD NOT 1, SHOULD 1, MAY 1

- ``mossignal` Standard Module Catalogue > 3. Relationship to the other specifications` [49-66]
  Preview: The API and semantics specification remains authoritative for network, machine, transaction, inspection, diagnostic, and lifecycle semantics.
  Symbols: `ModuleDef`, `ModuleBuilder`, `ModuleInstanceKey`, `ModuleInputKey<S>`, `ModuleOutputKey<S>`, `AddedModuleInstance`

- `Part I — Boundaries and classification` [67-200]
  Preview: The standard catalogue introduces no evaluator extension mechanism.
  Symbols: `ModuleInstance`, `annotate_signal`, `ModuleDef`, `ModuleDef<D>`
  Normative: MUST NOT 1, MUST 2, SHOULD NOT 1, SHOULD 2, MAY 2

- `Part I — Boundaries and classification > 4. Closed semantic boundary` [69-94]
  Preview: The standard catalogue introduces no evaluator extension mechanism.
  Normative: MUST NOT 1, MUST 1

- `Part I — Boundaries and classification > 5. Exhaustive convenience classification` [95-152]
  Preview: Every named convenience supplied by the crate MUST belong to exactly one of these categories.
  Symbols: `ModuleInstance`, `annotate_signal`, `ModuleDef`
  Normative: MUST 1, SHOULD NOT 1, SHOULD 1, MAY 2

- `Part I — Boundaries and classification > 5. Exhaustive convenience classification > 5.1 Primitive alias` [99-117]
  Preview: A primitive alias maps exactly to one existing primitive node kind and parameterization.
  Symbols: `ModuleInstance`
  Normative: MAY 1

- `Part I — Boundaries and classification > 5. Exhaustive convenience classification > 5.2 Standard module` [118-129]
  Preview: A standard module expands into a canonical graph containing more than one semantically relevant primitive operation, or otherwise provides a durable abstraction whose visible module boundary materially improves public semantics, explanation, inspection, persistence, or migration.
  Symbols: `ModuleInstance`

- `Part I — Boundaries and classification > 5. Exhaustive convenience classification > 5.3 Metadata-only convenience` [130-137]
  Preview: A metadata-only convenience annotates an existing semantic subject without changing signal behavior.
  Symbols: `annotate_signal`

- `Part I — Boundaries and classification > 5. Exhaustive convenience classification > 5.4 Builder-only operation` [138-152]
  Preview: A builder-only operation deterministically authors ordinary primitives but leaves no semantic object representing the operation itself.
  Symbols: `ModuleInstance`, `ModuleDef`
  Normative: SHOULD NOT 1, SHOULD 1, MAY 1

- `Part I — Boundaries and classification > 6. Admission criteria` [153-177]
  Preview: A behavior belongs in the standard catalogue only when it is: - domain-neutral; - broadly reusable; - semantically complete; - expressible through the supported semantic language; - meaningfully clearer as a named public unit; - valuable as an inspection or reconfiguration boundary; - capable of a durable typed interface; - capable of canonical stable internal identity; - testable independently of application assumptions.
  Normative: SHOULD 1

- `Part I — Boundaries and classification > 7. Explicit non-goals` [178-200]
  Preview: The initial catalogue does not introduce: User-authored `ModuleDef<D>` values remain supported and are not part of the standard namespace merely because they happen to match a standard module's behavior.
  Symbols: `ModuleDef<D>`

- `Part II — Catalogue identity and canonical descriptors` [201-627]
  Preview: The initial catalogue version is: The catalogue version identifies one released set of descriptors and discovery metadata.
  Symbols: `StandardModuleRef`, `ModuleInputKey<S>`, `ModuleDef<D>`, `ModuleOrigin::User`, `mossignal.standard`, `LogicLevel`, `U64`, `ModuleOutputKey<S>`, `1`, `ModuleFingerprint`, `NetworkFingerprint`
  Normative: MUST NOT 8, MUST 6, SHOULD 1, MAY 3

- `Part II — Catalogue identity and canonical descriptors > 8. Standard catalogue version` [203-218]
  Preview: The initial catalogue version is: The catalogue version identifies one released set of descriptors and discovery metadata.
  Normative: MUST NOT 1

- `Part II — Catalogue identity and canonical descriptors > 9. Standard module identifiers` [219-245]
  Preview: Each standard module has one permanent machine-readable identifier.
  Symbols: `mossignal.standard`
  Normative: MUST NOT 1, MUST 1

- `Part II — Catalogue identity and canonical descriptors > 10. Descriptor versions` [246-299]
  Preview: Each descriptor carries two independent versions: Numeric adjacency implies no compatibility.
  Normative: MUST NOT 1

- `Part II — Catalogue identity and canonical descriptors > 10. Descriptor versions > 10.1 Semantic version` [257-275]
  Preview: The semantic version changes when any of these change materially: - public input or output meaning; - parameter meaning or valid domain; - current output law; - successor-state law; - simultaneous-input law; - pulse multiplicity law; - current-reaction dependency signature; - initialization law; - exact-deadline or scheduling behavior; - public explanation meaning where structurally authoritative; - diagnostic condition or episode identity; - public migration contract.
  Normative: MUST NOT 1

- `Part II — Catalogue identity and canonical descriptors > 10. Descriptor versions > 10.2 Expansion version` [276-290]
  Preview: The expansion version changes when any of these change: - canonical internal primitive kinds; - canonical internal connections; - internal stable role assignment; - internal state schema or ownership; - internal pending-event ownership; - canonical export wiring; - nested standard-module versions; - expansion fingerprint projection.

- `Part II — Catalogue identity and canonical descriptors > 10. Descriptor versions > 10.3 Initial versions` [291-299]
  Preview: Every module in catalogue version 1 has:

- `Part II — Catalogue identity and canonical descriptors > 11. Descriptor reference` [300-315]
  Preview: The exact standard identity of a module definition is: A display name or Rust constructor name is not part of identity.
  Symbols: `StandardModuleRef`

- `Part II — Catalogue identity and canonical descriptors > 12. Authoritative descriptor` [316-344]
  Preview: A standard module descriptor contains at least: The descriptor is authoritative.
  Symbols: `StandardModuleRef`
  Normative: MUST 2

- `Part II — Catalogue identity and canonical descriptors > 13. Standard module origin` [345-369]
  Preview: `ModuleDef<D>` is extended with an origin classification broadly equivalent to: A standard declaration contains: Fixed public interface keys are supplied by the descriptor and need not be repeated as caller choices, although they remain present in the expanded `ModuleDef<D>`.
  Symbols: `ModuleDef<D>`, `ModuleOrigin::User`

- `Part II — Catalogue identity and canonical descriptors > 14. Parameter representation` [370-400]
  Preview: Strongly typed constructors use module-specific configuration values.
  Symbols: `LogicLevel`, `U64`

- `Part II — Catalogue identity and canonical descriptors > 15. Public port identity` [401-424]
  Preview: Every fixed public port has one catalogue-defined stable role and one deterministic `ModuleInputKey<S>` or `ModuleOutputKey<S>`.
  Symbols: `ModuleInputKey<S>`, `ModuleOutputKey<S>`
  Normative: SHOULD 1, MAY 1

- `Part II — Catalogue identity and canonical descriptors > 16. Canonical internal expansion` [425-464]
  Preview: For one declaration, the descriptor determines exactly one stable-keyed primitive graph.
  Normative: MUST NOT 1

- `Part II — Catalogue identity and canonical descriptors > 17. Internal stable-key derivation` [465-523]
  Preview: The logical network identity of an internal subject remains: The module-local stable key for a generated subject is derived from: It MUST NOT depend on: - module instance key; - dense runtime position; - construction order; - hash iteration order; - display name; - diagnostic metadata; - ordinary parameter values unless the parameter creates a genuinely distinct role; - catalogue version.
  Symbols: `1`
  Normative: MUST NOT 2, MUST 1

- `Part II — Catalogue identity and canonical descriptors > 18. Variadic internal identity` [524-538]
  Preview: When one internal node has one corresponding input port for each public variadic input, its internal port key is qualified by the public `ModuleInputKey<S>`.
  Symbols: `ModuleInputKey<S>`

- `Part II — Catalogue identity and canonical descriptors > 19. Canonical expansion persistence` [539-559]
  Preview: The canonical persisted representation contains both: 1.
  Symbols: `StandardModuleRef`
  Normative: MUST NOT 1, MUST 2, MAY 2

- `Part II — Catalogue identity and canonical descriptors > 20. Standard-module encapsulation` [560-575]
  Preview: Semantic internals of a standard module are canonical and are not individually authorable.
  Symbols: `ModuleOrigin::User`
  Normative: MUST NOT 1

- `Part II — Catalogue identity and canonical descriptors > 21. Fingerprints` [576-627]
  Preview: The standard-module system defines: It uses the domain: and covers the exact canonical semantic expansion, including module-local nodes, typed ports, connections, exports, internal role keys, parameters that select the expansion, public interface keys, semantic and expansion versions, and nested descriptor references.
  Symbols: `ModuleFingerprint`, `NetworkFingerprint`

- `Part III — Initial catalogue and non-module conveniences` [628-695]
  Preview: Catalogue version 1 contains exactly these standard modules: Each has semantic version `1` and expansion version `1`.
  Symbols: `1`, `InertialDelay`, `xor(a, b)`, `Parity([a, b])`, `debounce(input, delay, initial)`, `level_gate(value, enable)`, `All([value, enable])`, `majority(inputs)`, `AtLeast(floor(arity / 2) + 1, inputs)`, `debounce`, `Not`, `level_gate`, `PulseGate`, `majority([]) = Low`, `AtLeast(1, [])`, `annotate_signal`
  Normative: MUST NOT 1, MUST 1, SHOULD 2

- `Part III — Initial catalogue and non-module conveniences > 22. Complete catalogue version 1 inventory` [630-646]
  Preview: Catalogue version 1 contains exactly these standard modules: Each has semantic version `1` and expansion version `1`.
  Symbols: `1`

- `Part III — Initial catalogue and non-module conveniences > 23. Primitive aliases in version 1` [647-659]
  Preview: The initial primitive aliases are: | Convenience | Canonical primitive | |---|---| | `xor(a, b)` | `Parity([a, b])` | | `debounce(input, delay, initial)` | `InertialDelay` with the supplied configuration | | `level_gate(value, enable)` | `All([value, enable])` | | `majority(inputs)` | `AtLeast(floor(arity / 2) + 1, inputs)` | `debounce` MUST expose every semantic `InertialDelay` parameter.
  Symbols: `InertialDelay`, `xor(a, b)`, `Parity([a, b])`, `debounce(input, delay, initial)`, `level_gate(value, enable)`, `All([value, enable])`, `majority(inputs)`, `AtLeast(floor(arity / 2) + 1, inputs)`, `debounce`
  Normative: MUST NOT 1, MUST 1

- `Part III — Initial catalogue and non-module conveniences > 24. Builder-only operations in version 1` [660-687]
  Preview: The initial builder-only operation set is: The typed builders SHOULD expose concise methods for this set.
  Symbols: `Not`, `level_gate`, `PulseGate`, `majority([]) = Low`, `AtLeast(1, [])`
  Normative: SHOULD 2

- `Part III — Initial catalogue and non-module conveniences > 25. Metadata-only operations in version 1` [688-695]
  Preview: `annotate_signal` and equivalent metadata helpers remain metadata-only.
  Symbols: `annotate_signal`

- `Part IV — Shared standard-module semantics` [696-878]
  Preview: Every standard module has a direct public reference law independent of the internal implementation.
  Symbols: `initial`, `Low`, `LevelResettableToggle`, `High`, `toggle_state`, `reset_baseline`, `LevelResettableSampleHold`, `reset_to`, `why not`
  Normative: MUST NOT 3, MUST 8, MAY 3

- `Part IV — Shared standard-module semantics > 26. Public behavioral law and expansion equivalence` [698-721]
  Preview: Every standard module has a direct public reference law independent of the internal implementation.

- `Part IV — Shared standard-module semantics > 27. Public current-reaction dependency signature` [722-739]
  Preview: A descriptor states the conservative current public input-to-output dependency relation.
  Normative: MUST 1, MAY 1

- `Part IV — Shared standard-module semantics > 28. Initialization` [740-751]
  Preview: A newly instantiated standard module receives the declared initial state of every internal primitive.
  Symbols: `Low`
  Normative: MAY 1

- `Part IV — Shared standard-module semantics > 29. Aggregate state` [752-764]
  Preview: A standard module may expose a coherent aggregate state distinct from its internal primitive state.
  Normative: MUST 2

- `Part IV — Shared standard-module semantics > 30. Cross-internal state invariants` [765-782]
  Preview: A stateful standard descriptor defines every invariant required for its internal state to represent a reachable module state.
  Symbols: `initial`, `LevelResettableToggle`, `High`, `toggle_state`, `reset_baseline`, `LevelResettableSampleHold`, `reset_to`
  Normative: MUST 2

- `Part IV — Shared standard-module semantics > 31. Explanations` [783-800]
  Preview: A standard module provides a module-level explanation that groups the primitive derivation into the public concept.
  Normative: MUST NOT 1, MUST 2

- `Part IV — Shared standard-module semantics > 32. Why-not explanations` [801-825]
  Preview: Each descriptor defines a public `why not` interpretation.
  Symbols: `why not`
  Normative: MUST 1

- `Part IV — Shared standard-module semantics > 33. Inspection` [826-861]
  Preview: Every standard module instance exposes at least: Primitive internals remain inspectable through an explicit expanded view containing: Flattening for execution MUST NOT remove this hierarchy or role mapping.
  Normative: MUST NOT 1

- `Part IV — Shared standard-module semantics > 34. Public pulse activity` [862-878]
  Preview: Pulse values remain reaction-scoped.
  Normative: MUST NOT 1, MAY 1

- `Part V — Combinational standard modules` [879-1271]
  Preview: `Exactly`, `AtMost`, and `AllEqual` accept zero or more `Level` inputs.
  Symbols: `High`, `Low`, `result`, `k = 0`, `Exactly`, `k > n`, `k >= n`, `AtMost`, `AllEqual`, `n = 0`, `n = 1`, `n > 1`, `0 < k < n`, `k - h`, `h - k`, `Level`, `0 <= k <= n`, `n > 0`, `k = n = 1`, `k = n > 1`, `h < k`, `h > k`, `AtLeast`, `n`, `h`, `k`, `k < n`, `Not`, `n <= 1`, `n >= 2`
  Normative: MUST NOT 1, MAY 1

- `Part V — Combinational standard modules > 35. Shared variadic level rules` [881-896]
  Preview: `Exactly`, `AtMost`, and `AllEqual` accept zero or more `Level` inputs.
  Symbols: `Exactly`, `AtMost`, `AllEqual`, `Level`
  Normative: MUST NOT 1, MAY 1

- `Part V — Combinational standard modules > 36. `Exactly`` [897-1064]
  Preview: `Exactly` deserves a standard boundary because its public concept is exact cardinality, not merely the lower-and-upper threshold formula used internally.
  Symbols: `k > n`, `k = 0`, `result`, `n = 0`, `Low`, `Exactly`, `0 <= k <= n`, `n > 0`, `n = 1`, `n > 1`, `k = n = 1`, `k = n > 1`, `0 < k < n`, `High`, `h < k`, `k - h`, `h > k`, `h - k`

- `Part V — Combinational standard modules > 36. `Exactly` > 36.1 Identity` [899-908]

- `Part V — Combinational standard modules > 36. `Exactly` > 36.2 Inclusion rationale` [909-912]
  Preview: `Exactly` deserves a standard boundary because its public concept is exact cardinality, not merely the lower-and-upper threshold formula used internally.
  Symbols: `Exactly`

- `Part V — Combinational standard modules > 36. `Exactly` > 36.3 Public interface` [913-927]
  Preview: No default threshold exists.

- `Part V — Combinational standard modules > 36. `Exactly` > 36.4 Behavioral law` [928-955]
  Preview: Let: Then: Total boundary cases include: Duplicate sources contribute once per public port.

- `Part V — Combinational standard modules > 36. `Exactly` > 36.5 Public dependency signature` [956-961]
  Preview: When `0 <= k <= n` and `n > 0`, every public input may affect `result`.
  Symbols: `result`, `0 <= k <= n`, `n > 0`, `k > n`, `n = 0`

- `Part V — Combinational standard modules > 36. `Exactly` > 36.6 Canonical expansion` [962-1026]
  Preview: The exact expansion is selected by arity and threshold.
  Symbols: `k = 0`, `k > n`, `n = 0`, `n = 1`, `n > 1`, `k = n = 1`, `k = n > 1`, `0 < k < n`

- `Part V — Combinational standard modules > 36. `Exactly` > 36.6 Canonical expansion > Case A — impossible threshold` [966-974]
  Preview: When `k > n`:
  Symbols: `k > n`

- `Part V — Combinational standard modules > 36. `Exactly` > 36.6 Canonical expansion > Case B — zero threshold` [975-997]
  Preview: When `k = 0` and `n = 0`: When `k = 0` and `n = 1`: When `k = 0` and `n > 1`:
  Symbols: `k = 0`, `n = 0`, `n = 1`, `n > 1`

- `Part V — Combinational standard modules > 36. `Exactly` > 36.6 Canonical expansion > Case C — threshold equals arity` [998-1012]
  Preview: When `k = n = 1`: When `k = n > 1`:
  Symbols: `k = n = 1`, `k = n > 1`

- `Part V — Combinational standard modules > 36. `Exactly` > 36.6 Canonical expansion > Case D — interior threshold` [1013-1026]
  Preview: When `0 < k < n`: The case split avoids generating internally degenerate threshold warnings as an implementation artifact of a valid public module declaration.
  Symbols: `0 < k < n`

- `Part V — Combinational standard modules > 36. `Exactly` > 36.7 Explanation` [1027-1042]
  Preview: When `High`, the explanation reports: When `Low` because `h < k`, the why-not explanation reports the deficit `k - h` and the Low inputs that prevent the threshold from being reached.
  Symbols: `Low`, `High`, `h < k`, `k - h`, `h > k`, `h - k`, `k > n`

- `Part V — Combinational standard modules > 36. `Exactly` > 36.8 Inspection` [1043-1056]
  Preview: The module summary exposes:

- `Part V — Combinational standard modules > 36. `Exactly` > 36.9 Reconfiguration` [1057-1064]
  Preview: Changing threshold or variadic arity is stateless and uses ordinary target reevaluation.

- `Part V — Combinational standard modules > 37. `AtMost`` [1065-1178]
  Preview: `AtMost` is included because its public semantics are an upper-bound cardinality constraint with allowance and excess explanations, not merely a negated `AtLeast` spelling.
  Symbols: `k >= n`, `High`, `k = 0`, `AtMost`, `AtLeast`, `n`, `h`, `k`, `k < n`, `result`, `n = 1`, `n > 1`, `0 < k < n`, `k - h`, `Low`, `h - k`, `Exactly`

- `Part V — Combinational standard modules > 37. `AtMost` > 37.1 Identity` [1067-1076]

- `Part V — Combinational standard modules > 37. `AtMost` > 37.2 Inclusion rationale` [1077-1080]
  Preview: `AtMost` is included because its public semantics are an upper-bound cardinality constraint with allowance and excess explanations, not merely a negated `AtLeast` spelling.
  Symbols: `AtMost`, `AtLeast`

- `Part V — Combinational standard modules > 37. `AtMost` > 37.3 Public interface` [1081-1093]

- `Part V — Combinational standard modules > 37. `AtMost` > 37.4 Behavioral law` [1094-1109]
  Preview: With `n`, `h`, and `k` defined as above: Boundary cases include:
  Symbols: `n`, `h`, `k`

- `Part V — Combinational standard modules > 37. `AtMost` > 37.5 Public dependency signature` [1110-1115]
  Preview: When `k < n`, every public input may affect `result`.
  Symbols: `k < n`, `result`, `k >= n`, `High`

- `Part V — Combinational standard modules > 37. `AtMost` > 37.6 Canonical expansion` [1116-1152]
  Preview: When `k >= n`: When `k = 0` and `n = 1`: When `k = 0` and `n > 1`: When `0 < k < n`:
  Symbols: `k = 0`, `k >= n`, `n = 1`, `n > 1`, `0 < k < n`

- `Part V — Combinational standard modules > 37. `AtMost` > 37.6 Canonical expansion > Case A — threshold covers arity` [1118-1126]
  Preview: When `k >= n`:
  Symbols: `k >= n`

- `Part V — Combinational standard modules > 37. `AtMost` > 37.6 Canonical expansion > Case B — zero threshold below arity` [1127-1142]
  Preview: When `k = 0` and `n = 1`: When `k = 0` and `n > 1`:
  Symbols: `k = 0`, `n = 1`, `n > 1`

- `Part V — Combinational standard modules > 37. `AtMost` > 37.6 Canonical expansion > Case C — interior threshold` [1143-1152]
  Preview: When `0 < k < n`:
  Symbols: `0 < k < n`

- `Part V — Combinational standard modules > 37. `AtMost` > 37.7 Explanation` [1153-1160]
  Preview: When `High`, the explanation reports the current High count and remaining allowance `k - h`.
  Symbols: `High`, `k - h`, `Low`, `h - k`, `k >= n`

- `Part V — Combinational standard modules > 37. `AtMost` > 37.8 Inspection` [1161-1174]
  Preview: The summary exposes:

- `Part V — Combinational standard modules > 37. `AtMost` > 37.9 Reconfiguration` [1175-1178]
  Preview: Threshold and arity changes are stateless.
  Symbols: `Exactly`

- `Part V — Combinational standard modules > 38. `AllEqual`` [1179-1271]
  Preview: `AllEqual` expresses consensus over a variadic set.
  Symbols: `High`, `Low`, `result`, `AllEqual`, `Not`, `n <= 1`, `n >= 2`

- `Part V — Combinational standard modules > 38. `AllEqual` > 38.1 Identity` [1181-1190]

- `Part V — Combinational standard modules > 38. `AllEqual` > 38.2 Inclusion rationale` [1191-1194]
  Preview: `AllEqual` expresses consensus over a variadic set.
  Symbols: `AllEqual`, `Not`

- `Part V — Combinational standard modules > 38. `AllEqual` > 38.3 Public interface` [1195-1206]

- `Part V — Combinational standard modules > 38. `AllEqual` > 38.4 Behavioral law` [1207-1221]
  Preview: Total boundary cases are: For arity at least two, `result` is `High` exactly when all inputs are `Low` or all are `High`.
  Symbols: `High`, `result`, `Low`

- `Part V — Combinational standard modules > 38. `AllEqual` > 38.5 Public dependency signature` [1222-1227]
  Preview: For arity zero or one, the result is constant `High` and has no current public input dependency.
  Symbols: `High`, `result`

- `Part V — Combinational standard modules > 38. `AllEqual` > 38.6 Canonical expansion` [1228-1246]
  Preview: When `n <= 1`: When `n >= 2`:
  Symbols: `n <= 1`, `n >= 2`

- `Part V — Combinational standard modules > 38. `AllEqual` > 38.7 Explanation` [1247-1252]
  Preview: When `High`, the explanation identifies whether the common value is `Low`, `High`, or vacuous because arity is zero or one.
  Symbols: `High`, `Low`

- `Part V — Combinational standard modules > 38. `AllEqual` > 38.8 Inspection` [1253-1265]
  Preview: The summary exposes:

- `Part V — Combinational standard modules > 38. `AllEqual` > 38.9 Reconfiguration` [1266-1271]
  Preview: Arity changes are stateless and preserve surviving public-port identity.

- `Part VI — Stateful standard modules` [1272-1878]
  Preview: The initial resettable modules use one fixed reset kind and one fixed priority law per descriptor.
  Symbols: `Low`, `High`, `reset_to`, `initial`, `Reset`, `q`, `toggle_state`, `reset_baseline`, `a`, `b`, `RisingEdge(Assume(Low))`, `a XOR b = Low`, `q = Low`, `a = new initial`, `b = Low`, `n`, `reset_edge`

- `Part VI — Stateful standard modules > 39. Shared resettable-module rules` [1274-1293]
  Preview: The initial resettable modules use one fixed reset kind and one fixed priority law per descriptor.
  Symbols: `initial`, `Reset`

- `Part VI — Stateful standard modules > 40. `PulseResettableToggle`` [1294-1465]
  Preview: This module gives resettable parity state one durable public identity while preserving reset-dominant same-reaction behavior.
  Symbols: `toggle_state`, `reset_baseline`, `a`, `b`, `Low`, `a XOR b = Low`, `q = Low`, `initial`, `Reset`, `a = new initial`, `b = Low`

- `Part VI — Stateful standard modules > 40. `PulseResettableToggle` > 40.1 Identity` [1296-1305]

- `Part VI — Stateful standard modules > 40. `PulseResettableToggle` > 40.2 Inclusion rationale` [1306-1309]
  Preview: This module gives resettable parity state one durable public identity while preserving reset-dominant same-reaction behavior.

- `Part VI — Stateful standard modules > 40. `PulseResettableToggle` > 40.3 Public interface` [1310-1325]
  Preview: Both ports are fixed and have catalogue-defined stable keys.

- `Part VI — Stateful standard modules > 40. `PulseResettableToggle` > 40.4 Public reference law` [1326-1355]
  Preview: Let: Then: Reset multiplicity beyond presence is irrelevant to the reset decision but all reset causes remain available.
  Symbols: `Low`

- `Part VI — Stateful standard modules > 40. `PulseResettableToggle` > 40.5 Public dependency signature` [1356-1364]
  Preview: Both dependencies are current-reaction dependencies.

- `Part VI — Stateful standard modules > 40. `PulseResettableToggle` > 40.6 Canonical expansion` [1365-1406]
  Preview: Internal roles are: The expansion is: Let internal stored levels be `a` for `toggle_state` and `b` for `reset_baseline`.
  Symbols: `toggle_state`, `reset_baseline`, `a`, `b`, `a XOR b = Low`

- `Part VI — Stateful standard modules > 40. `PulseResettableToggle` > 40.7 Initialization` [1407-1420]
  Preview: Initial internal state is: An initialization reset pulse forces `q = Low` in the first reaction.
  Symbols: `q = Low`

- `Part VI — Stateful standard modules > 40. `PulseResettableToggle` > 40.8 Explanation` [1421-1436]
  Preview: The module-level explanation reports: When reset is present, the primitive drill-down shows the reset baseline sampling the post-toggle internal state.

- `Part VI — Stateful standard modules > 40. `PulseResettableToggle` > 40.9 Inspection` [1437-1452]
  Preview: The aggregate summary exposes: Expanded inspection additionally exposes `a`, `b`, and their stable internal roles.
  Symbols: `a`, `b`

- `Part VI — Stateful standard modules > 40. `PulseResettableToggle` > 40.10 Reconfiguration` [1453-1465]
  Preview: Under an exact same descriptor replacement: - `toggle_state` stored level is preserved; - `reset_baseline` stored level is preserved; - aggregate state is thereby preserved; - changing `initial` preserves both stored levels; - binding changes cause ordinary patch-time reevaluation; - a topology-migration `Reset` outcome initializes `a = new initial` and `b = Low` and reports discarded source state as required.
  Symbols: `toggle_state`, `reset_baseline`, `initial`, `Reset`, `a = new initial`, `b = Low`

- `Part VI — Stateful standard modules > 41. `LevelResettableToggle`` [1466-1646]
  Preview: This module defines a level-held reset contract that suppresses toggle work while asserted, establishes a precise rising-reset transition, and resumes from `Low` after release.
  Symbols: `Low`, `High`, `q`, `n`, `RisingEdge(Assume(Low))`, `initial`, `Reset`

- `Part VI — Stateful standard modules > 41. `LevelResettableToggle` > 41.1 Identity` [1468-1477]

- `Part VI — Stateful standard modules > 41. `LevelResettableToggle` > 41.2 Inclusion rationale` [1478-1481]
  Preview: This module defines a level-held reset contract that suppresses toggle work while asserted, establishes a precise rising-reset transition, and resumes from `Low` after release.
  Symbols: `Low`

- `Part VI — Stateful standard modules > 41. `LevelResettableToggle` > 41.3 Public interface` [1482-1495]

- `Part VI — Stateful standard modules > 41. `LevelResettableToggle` > 41.4 Public reference law` [1496-1518]
  Preview: Let `q` be previous aggregate state and `n` the current toggle count.
  Symbols: `Low`, `High`, `q`, `n`

- `Part VI — Stateful standard modules > 41. `LevelResettableToggle` > 41.5 Public dependency signature` [1519-1525]

- `Part VI — Stateful standard modules > 41. `LevelResettableToggle` > 41.6 Canonical expansion` [1526-1590]
  Preview: Internal roles are: The expansion is: The aggregate stored state when the module is not held in reset is: The rising reset edge samples the current toggle state into the reset baseline, making the relative state `Low`.
  Symbols: `Low`, `High`

- `Part VI — Stateful standard modules > 41. `LevelResettableToggle` > 41.7 Initialization` [1591-1601]
  Preview: When initial reset is `Low`, the configured initial level is the previous aggregate state and the initialization toggle batch applies the ordinary parity law in the first reaction.
  Symbols: `Low`, `High`, `RisingEdge(Assume(Low))`

- `Part VI — Stateful standard modules > 41. `LevelResettableToggle` > 41.8 Explanation` [1602-1614]
  Preview: When reset is `High`, the explanation reports: When reset is `Low`, the explanation reports previous aggregate state and accepted toggle parity.
  Symbols: `High`, `Low`

- `Part VI — Stateful standard modules > 41. `LevelResettableToggle` > 41.9 Inspection` [1615-1630]
  Preview: The aggregate summary exposes: Expanded inspection exposes every internal role, including the edge detector's remembered reset observation.

- `Part VI — Stateful standard modules > 41. `LevelResettableToggle` > 41.10 Reconfiguration` [1631-1646]
  Preview: Exact same-descriptor replacement preserves: Changing `initial` preserves those values.
  Symbols: `initial`, `Reset`

- `Part VI — Stateful standard modules > 42. `LevelResettableSampleHold`` [1647-1878]
  Preview: This module provides a stable, inspectable resettable storage abstraction whose public behavior is materially clearer than its edge-detection, selection, pulse-merging, and sample-hold expansion.
  Symbols: `reset_to`, `High`, `initial`, `q`, `Reset`, `Low`, `RisingEdge(Assume(Low))`, `reset_edge`

- `Part VI — Stateful standard modules > 42. `LevelResettableSampleHold` > 42.1 Identity` [1649-1658]

- `Part VI — Stateful standard modules > 42. `LevelResettableSampleHold` > 42.2 Inclusion rationale` [1659-1670]
  Preview: This module provides a stable, inspectable resettable storage abstraction whose public behavior is materially clearer than its edge-detection, selection, pulse-merging, and sample-hold expansion.

- `Part VI — Stateful standard modules > 42. `LevelResettableSampleHold` > 42.3 Public interface` [1671-1690]
  Preview: Neither parameter has a default.
  Symbols: `initial`, `reset_to`

- `Part VI — Stateful standard modules > 42. `LevelResettableSampleHold` > 42.4 Public state` [1691-1708]
  Preview: The public reference state is: For a fresh module: The remembered reset state corresponds to `RisingEdge(Assume(Low))`.
  Symbols: `RisingEdge(Assume(Low))`

- `Part VI — Stateful standard modules > 42. `LevelResettableSampleHold` > 42.5 Public reference law` [1709-1746]
  Preview: At one reaction, let: Then: Consequences: - a rising reset immediately captures `reset_to`; - simultaneous sample and rising reset captures `reset_to`; - while reset remains `High`, any sample pulse captures `reset_to`; - value changes without a sample or rising reset do not change the held output; - falling reset does not sample the current value; - multiple sample pulses are equivalent to one capture, while all causes remain available.
  Symbols: `reset_to`, `q`, `High`, `initial`, `Reset`, `Low`

- `Part VI — Stateful standard modules > 42. `LevelResettableSampleHold` > 42.6 Public dependency signature` [1747-1756]
  Preview: All three are conservative current-reaction dependencies because each can affect the current output under a valid prior state and current batch.

- `Part VI — Stateful standard modules > 42. `LevelResettableSampleHold` > 42.7 Canonical expansion` [1757-1798]
  Preview: Internal roles are: The expansion is:

- `Part VI — Stateful standard modules > 42. `LevelResettableSampleHold` > 42.8 Initialization` [1799-1804]
  Preview: With initial reset `Low`, an initialization sample captures the settled input value; otherwise the output begins at configured `initial`.
  Symbols: `Low`, `initial`, `High`, `reset_edge`, `reset_to`

- `Part VI — Stateful standard modules > 42. `LevelResettableSampleHold` > 42.9 Explanation` [1805-1825]
  Preview: The module explanation reports: Why-not-change reports that no sample pulse and no rising reset occurred.
  Symbols: `High`, `reset_to`

- `Part VI — Stateful standard modules > 42. `LevelResettableSampleHold` > 42.10 Inspection` [1826-1841]
  Preview: The aggregate summary exposes: Expanded inspection exposes the internal edge-detector and sample-hold state.

- `Part VI — Stateful standard modules > 42. `LevelResettableSampleHold` > 42.11 Reconfiguration` [1842-1878]
  Preview: Exact same-descriptor replacement ordinarily preserves: Parameter changes have these standard outcomes: Changing the reset binding may create a patch-time rising edge relative to the preserved remembered observation.
  Symbols: `Reset`, `High`, `reset_to`

- `Part VII — Deferred and rejected initial candidates` [1879-2015]
  Preview: `Nand`, `Nor`, and `Xnor` are not standard modules in catalogue version 1.
  Symbols: `PulseDelay`, `Nand`, `Nor`, `Xnor`, `LevelGate`, `Majority`, `AnyPulse`, `PulseMultiply`, `Merge`, `PulseCap(limit)`, `limit = 1`, `Coalesce`, `limit = 0`, `PulseParity`, `Select`, `PulseSelect`, `PulseRoute`, `latest_trigger + duration`, `Toggle`, `InertialDelay`, `Timer`

- `Part VII — Deferred and rejected initial candidates > 43. Thin Boolean conveniences` [1881-1895]
  Preview: `Nand`, `Nor`, and `Xnor` are not standard modules in catalogue version 1.
  Symbols: `Nand`, `Nor`, `Xnor`, `LevelGate`, `Majority`

- `Part VII — Deferred and rejected initial candidates > 44. Pulse algebra candidates` [1896-1933]
  Preview: `AnyPulse` is builder-only: Its two-step expansion is direct and owns no state.
  Symbols: `AnyPulse`, `PulseMultiply`, `Merge`, `PulseCap(limit)`, `limit = 1`, `Coalesce`, `limit = 0`, `PulseParity`

- `Part VII — Deferred and rejected initial candidates > 44. Pulse algebra candidates > 44.1 `AnyPulse`` [1898-1907]
  Preview: `AnyPulse` is builder-only: Its two-step expansion is direct and owns no state.
  Symbols: `AnyPulse`

- `Part VII — Deferred and rejected initial candidates > 44. Pulse algebra candidates > 44.2 `PulseMultiply`` [1908-1915]
  Preview: `PulseMultiply` is deferred.
  Symbols: `PulseMultiply`, `Merge`

- `Part VII — Deferred and rejected initial candidates > 44. Pulse algebra candidates > 44.3 `PulseCap`` [1916-1927]
  Preview: `PulseCap(limit)` is deferred because the current primitive language cannot express: for arbitrary limits greater than one.
  Symbols: `PulseCap(limit)`, `limit = 1`, `Coalesce`, `limit = 0`

- `Part VII — Deferred and rejected initial candidates > 44. Pulse algebra candidates > 44.4 `PulseParity`` [1928-1933]
  Preview: `PulseParity` is deferred.
  Symbols: `PulseParity`

- `Part VII — Deferred and rejected initial candidates > 45. Routing and selection candidates` [1934-1947]
  Preview: Low-enabled variants remain explicit inversion plus the canonical high-enabled primitive.
  Symbols: `Select`, `PulseSelect`, `PulseRoute`

- `Part VII — Deferred and rejected initial candidates > 46. `PulseResettableSampleHold`` [1948-1953]
  Preview: A pulse-resettable sample-and-hold is not included.

- `Part VII — Deferred and rejected initial candidates > 47. Temporal candidates` [1954-2015]
  Preview: The following remain deferred: They are not merely awaiting names.
  Symbols: `PulseDelay`, `latest_trigger + duration`, `Toggle`, `InertialDelay`, `Timer`

- `Part VII — Deferred and rejected initial candidates > 47. Temporal candidates > 47.1 Fixed one-shot` [1969-1976]
  Preview: A fixed one-shot must accept a trigger only while idle and schedule exactly one expiration for the accepted trigger.
  Symbols: `PulseDelay`

- `Part VII — Deferred and rejected initial candidates > 47. Temporal candidates > 47.2 Retriggerable one-shot and pulse stretcher` [1977-1982]
  Preview: These require one active expiration that moves to `latest_trigger + duration`.
  Symbols: `latest_trigger + duration`, `PulseDelay`

- `Part VII — Deferred and rejected initial candidates > 47. Temporal candidates > 47.3 Timeout and watchdog` [1983-1990]
  Preview: These require an obligation that is armed or restarted by arbitrary activity and fires only after a complete quiet interval.
  Symbols: `Toggle`, `InertialDelay`

- `Part VII — Deferred and rejected initial candidates > 47. Temporal candidates > 47.4 Rate limiter` [1991-1996]
  Preview: A rate limiter requires a refractory state whose acceptance decision observes previous availability and whose reset occurs at a future deadline.

- `Part VII — Deferred and rejected initial candidates > 47. Temporal candidates > 47.5 Admission condition for future temporal modules` [1997-2015]
  Preview: A temporal module may enter a later catalogue only after the primitive language or standard-module mechanism can express all of: No vague `Timer` module is permitted.
  Symbols: `Timer`

- `Part VIII — Construction and discovery API` [2016-2236]
  Preview: The crate SHOULD expose types broadly equivalent to: No global mutable registry is required.
  Symbols: `ModuleDef<D>`, `StandardCatalogue<D>`, `latest`, `StandardModuleRef`, `ModuleOrigin::Standard`, `Signal<S>`, `NetworkBuilder<D>`, `ModuleBuilder<D>`, `ModuleInstanceKey`, `DiagnosticMeta`, `AddedStandardModule<Signal<Level>>`, `NetworkBuilder::instantiate`
  Normative: MUST NOT 1, MUST 6, SHOULD 2, MAY 3

- `Part VIII — Construction and discovery API > 48. Public catalogue types` [2018-2043]
  Preview: The crate SHOULD expose types broadly equivalent to: No global mutable registry is required.
  Symbols: `StandardCatalogue<D>`
  Normative: SHOULD 1, MAY 1

- `Part VIII — Construction and discovery API > 49. Descriptor discovery` [2044-2090]
  Preview: The catalogue API MUST support: Discovery metadata includes: `latest` is an authoring convenience only.
  Symbols: `latest`, `StandardModuleRef`
  Normative: MUST 2

- `Part VIII — Construction and discovery API > 50. Dynamic construction` [2091-2117]
  Preview: A dynamic request supplies: Fixed public port keys come from the descriptor.
  Symbols: `ModuleDef<D>`, `ModuleOrigin::Standard`
  Normative: MUST 1

- `Part VIII — Construction and discovery API > 51. Typed construction result` [2118-2139]
  Preview: Explicit typed standard-module construction SHOULD return: with accessors for: A one-output module uses `Signal<S>` as `O`.
  Symbols: `Signal<S>`
  Normative: SHOULD 1

- `Part VIII — Construction and discovery API > 52. Keyed variadic bindings` [2140-2152]
  Preview: Explicit variadic construction uses a shape broadly equivalent to: The iterator is consumed immediately and does not remain borrowed.

- `Part VIII — Construction and discovery API > 53. Typed convenience methods` [2153-2206]
  Preview: `NetworkBuilder<D>` and `ModuleBuilder<D>` MUST both support the standard modules so user-defined modules may contain standard modules.
  Symbols: `NetworkBuilder<D>`, `ModuleBuilder<D>`
  Normative: MUST 1, MAY 1

- `Part VIII — Construction and discovery API > 54. Explicit keyed forms` [2207-2228]
  Preview: Every standard module has an explicit form broadly equivalent to: Fixed-interface methods receive `ModuleInstanceKey`, typed signals, semantic configuration, and `DiagnosticMeta`.
  Symbols: `ModuleInstanceKey`, `DiagnosticMeta`, `AddedStandardModule<Signal<Level>>`
  Normative: MUST NOT 1, MUST 1

- `Part VIII — Construction and discovery API > 55. Ordinary module instantiation` [2229-2236]
  Preview: Callers MAY also obtain a generated `ModuleDef<D>` from the catalogue and use the existing generic `NetworkBuilder::instantiate` API.
  Symbols: `ModuleDef<D>`, `NetworkBuilder::instantiate`
  Normative: MUST 1, MAY 1

- `Part IX — Diagnostics` [2237-2327]
  Preview: A module-level condition belongs to the `ModuleInstanceKey` or standard `ModuleDef` as its primary subject.
  Symbols: `Exactly`, `AtMost`, `AllEqual`, `constant_result`, `ModuleInstanceKey`, `ModuleDef`, `standard_module.unknown_id`, `standard_module.unsupported_version`, `standard_module.missing_parameter`, `standard_module.unexpected_parameter`, `standard_module.parameter_kind_mismatch`, `standard_module.invalid_parameter`, `standard_module.interface_mismatch`, `standard_module.expansion_mismatch`, `standard_module.noncanonical_internal_edit`, `standard_module.incompatible_version_migration`, `standard_module.internal_key_collision`, `standard_module.catalogue_invariant`, `internal_key_collision`, `catalogue_invariant`, `standard_module.deprecated_alias`, `standard_module.empty_variadic`, `standard_module.unary_degenerate`, `standard_module.impossible_threshold`, `threshold > arity`, `Low`, `standard_module.constant_result`, `standard_module.duplicate_source`, `empty_variadic`, `impossible_threshold`, `AtMost(k >= arity)`, `duplicate_source`
  Normative: MUST NOT 2, MUST 1, SHOULD 1, MAY 1

- `Part IX — Diagnostics > 56. Diagnostic ownership` [2239-2248]
  Preview: A module-level condition belongs to the `ModuleInstanceKey` or standard `ModuleDef` as its primary subject.
  Symbols: `ModuleInstanceKey`, `ModuleDef`
  Normative: MUST NOT 1, MAY 1

- `Part IX — Diagnostics > 57. Blocking catalogue diagnostics` [2249-2269]
  Preview: The standard catalogue introduces these blocking conditions: | Stable code | Stage | Meaning | |---|---|---| | `standard_module.unknown_id` | dynamic construction, decoding | No descriptor exists for the identifier | | `standard_module.unsupported_version` | construction, validation, restoration, replay | The exact semantic or expansion version is unavailable | | `standard_module.missing_parameter` | construction, validation | A required parameter is absent | | `standard_module.unexpected_parameter` | construction, validation | The declaration contains an unknown parameter | | `standard_module.parameter_kind_mismatch` | construction, decoding | A parameter has the wrong value kind | | `standard_module.invalid_parameter` | construction, validation | A parameter value violates its schema | | `standard_module.interface_mismatch` | validation, restoration, patch preparation | Public ports do not match the descriptor | | `standard_module.expansion_mismatch` | validation, restoration, patch preparation | Persisted or patched internals differ from canonical expansion | | `standard_module.noncanonical_internal_edit` | patch preparation | A semantic internal edit attempts to retain standard identity | | `standard_module.incompatible_version_migration` | patch preparation or finalization | No declared migration exists between versions | | `standard_module.internal_key_collision` | descriptor validation or compilation | Canonical key derivation produced a duplicate key | | `standard_module.catalogue_invariant` | descriptor validation or compilation | Descriptor data and generated expansion disagree | `internal_key_collision` and `catalogue_invariant` indicate a library defect when produced by a shipped descriptor.
  Symbols: `standard_module.unknown_id`, `standard_module.unsupported_version`, `standard_module.missing_parameter`, `standard_module.unexpected_parameter`, `standard_module.parameter_kind_mismatch`, `standard_module.invalid_parameter`, `standard_module.interface_mismatch`, `standard_module.expansion_mismatch`, `standard_module.noncanonical_internal_edit`, `standard_module.incompatible_version_migration`, `standard_module.internal_key_collision`, `standard_module.catalogue_invariant`, `internal_key_collision`, `catalogue_invariant`

- `Part IX — Diagnostics > 58. Non-blocking module diagnostics` [2270-2300]
  Preview: The initial catalogue introduces these non-blocking static conditions: | Stable code | Applies to | Meaning | |---|---|---| | `standard_module.deprecated_alias` | any | An authoring alias is deprecated | | `standard_module.empty_variadic` | `Exactly`, `AtMost`, `AllEqual` | The module has zero public inputs | | `standard_module.unary_degenerate` | `Exactly`, `AtMost`, `AllEqual` | The module has one input and simplifies to a constant or identity-like law | | `standard_module.impossible_threshold` | `Exactly` | `threshold > arity`, so result is always `Low` | | `standard_module.constant_result` | `Exactly`, `AtMost`, `AllEqual` | Parameters and arity make output constant | | `standard_module.duplicate_source` | variadic modules | One upstream source is bound to several public ports | These are validation or patch-preparation occurrences, not runtime diagnostic episodes.
  Symbols: `Exactly`, `AtMost`, `AllEqual`, `constant_result`, `standard_module.deprecated_alias`, `standard_module.empty_variadic`, `standard_module.unary_degenerate`, `standard_module.impossible_threshold`, `threshold > arity`, `Low`, `standard_module.constant_result`, `standard_module.duplicate_source`, `empty_variadic`, `impossible_threshold`, `AtMost(k >= arity)`, `duplicate_source`
  Normative: MUST NOT 1, MUST 1

- `Part IX — Diagnostics > 59. Diagnostic evidence` [2301-2319]
  Preview: Module diagnostics SHOULD provide structured evidence including, where applicable: Rendered prose is non-authoritative.
  Normative: SHOULD 1

- `Part IX — Diagnostics > 60. No new runtime episode family in catalogue 1` [2320-2327]
  Preview: No catalogue version 1 module introduces a module-owned persistent runtime diagnostic episode.

- `Part X — Reconfiguration and migration` [2328-2465]
  Preview: A standard module parameter, version, or public variadic-port change is represented as replacement of the complete canonical module instance definition.
  Symbols: `Reset`, `initial`, `LevelResettableSampleHold`, `reset_to`, `StandardModuleRef`, `ModuleInstance`, `Exactly`, `AtMost`, `AllEqual`, `PulseResettableToggle`, `LevelResettableToggle`, `Low`, `High`
  Normative: MUST NOT 1, MUST 2

- `Part X — Reconfiguration and migration > 61. Module replacement boundary` [2330-2337]
  Preview: A standard module parameter, version, or public variadic-port change is represented as replacement of the complete canonical module instance definition.
  Normative: MUST NOT 1

- `Part X — Reconfiguration and migration > 62. Public compatibility dimensions` [2338-2375]
  Preview: Preparation considers: The standard outcome is determined as follows.
  Symbols: `StandardModuleRef`, `ModuleInstance`

- `Part X — Reconfiguration and migration > 62. Public compatibility dimensions > 62.1 Same exact descriptor` [2356-2359]
  Preview: The same `StandardModuleRef` is compatible subject to module-specific parameter and interface rules.
  Symbols: `StandardModuleRef`

- `Part X — Reconfiguration and migration > 62. Public compatibility dimensions > 62.2 Different descriptor version` [2360-2365]
  Preview: A version change is compatible only when the target descriptor contains an explicit migration entry from the source descriptor.

- `Part X — Reconfiguration and migration > 62. Public compatibility dimensions > 62.3 Different module id` [2366-2371]
  Preview: No standard correspondence is inferred between different module identifiers.

- `Part X — Reconfiguration and migration > 62. Public compatibility dimensions > 62.4 Changed module instance key` [2372-2375]
  Preview: A key-changing replacement requires explicit `ModuleInstance` reassociation.
  Symbols: `ModuleInstance`

- `Part X — Reconfiguration and migration > 63. Public interface compatibility` [2376-2391]
  Preview: For fixed-interface modules: - all fixed public keys and signal kinds must match the descriptor; - no port may be added or removed while claiming the same descriptor.

- `Part X — Reconfiguration and migration > 64. Internal correspondence` [2392-2405]
  Preview: Internal correspondence is derived from equal permanent role keys and qualifying public port keys.
  Normative: MUST 1

- `Part X — Reconfiguration and migration > 65. Parameter migration matrix` [2406-2420]
  Preview: | Module | Parameter change | Standard outcome | |---|---|---| | `Exactly` | threshold | Stateless replacement and reevaluation | | `AtMost` | threshold | Stateless replacement and reevaluation | | `AllEqual` | none | Not applicable | | `PulseResettableToggle` | `initial` | Preserve internal state; affects fresh state or a migration `Reset` outcome only | | `LevelResettableToggle` | `initial` | Preserve internal state; affects fresh state or a migration `Reset` outcome only | | `LevelResettableSampleHold` | `initial` | Preserve held and edge state; affects fresh state or a migration `Reset` outcome only | | `LevelResettableSampleHold` | `reset_to`, settled reset `Low` | Preserve held and edge state; affects future reset-controlled captures | | `LevelResettableSampleHold` | `reset_to`, settled reset `High` | Migrate held state immediately to the new `reset_to`; preserve edge observation | To apply a new `initial` immediately, the patch must select the topology-migration `Reset` outcome.
  Symbols: `initial`, `Reset`, `LevelResettableSampleHold`, `reset_to`, `Exactly`, `AtMost`, `AllEqual`, `PulseResettableToggle`, `LevelResettableToggle`, `Low`, `High`

- `Part X — Reconfiguration and migration > 66. Binding changes` [2421-2435]
  Preview: Changing a public input binding preserves compatible internal state but seeds ordinary patch-time reevaluation.

- `Part X — Reconfiguration and migration > 67. Topology-migration `Reset` for a stateful module` [2436-2449]
  Preview: Selecting the topology-migration `Reset` outcome for a stateful standard module resets every internal state owner to the target descriptor's declared initial state.
  Symbols: `Reset`
  Normative: MUST 1

- `Part X — Reconfiguration and migration > 68. Pending work` [2450-2455]
  Preview: Catalogue version 1 standard modules own no temporal primitive and therefore no pending event.

- `Part X — Reconfiguration and migration > 69. Diagnostics and provenance migration` [2456-2465]
  Preview: Static module diagnostics are recomputed for the target declaration.

- `Part XI — Persistence and replay` [2466-2541]
  Preview: A standard-origin module definition persists: Display name and documentation text are not authoritative identity.
  Normative: MUST NOT 1, MUST 2

- `Part XI — Persistence and replay > 70. Persisted standard declaration` [2468-2483]
  Preview: A standard-origin module definition persists: Display name and documentation text are not authoritative identity.

- `Part XI — Persistence and replay > 71. Restoration validation` [2484-2497]
  Preview: Restoration and decoded definition validation MUST: 1.
  Normative: MUST NOT 1, MUST 1

- `Part XI — Persistence and replay > 72. Snapshot state` [2498-2509]
  Preview: Snapshots persist internal state under qualified stable identity: They also persist every ordinary state, pending event, diagnostic episode, and provenance root required by the persistence specification.
  Normative: MUST 1

- `Part XI — Persistence and replay > 73. Replay` [2510-2523]
  Preview: Exact replay requires the same standard module references and expansion meanings used by the recorded network and patches.

- `Part XI — Persistence and replay > 74. Schema-only changes` [2524-2541]
  Preview: A representation-only persistence schema upgrade may reorganize standard declaration fields only when it preserves: A behavior or expansion change is a semantic migration, not a schema upgrade.

- `Part XII — Verification and generated artifacts` [2542-2764]
  Preview: Every standard module descriptor MUST classify and test: Non-applicable fields must be marked explicitly.
  Symbols: `Exactly`, `AtMost`, `AllEqual`
  Normative: MUST 7, SHOULD 2

- `Part XII — Verification and generated artifacts > 75. Required standard-module conformance matrix` [2544-2580]
  Preview: Every standard module descriptor MUST classify and test: Non-applicable fields must be marked explicitly.
  Normative: MUST 1

- `Part XII — Verification and generated artifacts > 76. Dual-view equivalence` [2581-2594]
  Preview: For every descriptor: The comparison MUST include public output, successor aggregate state, normalized internal-state projection, diagnostics, and normalized provenance.
  Normative: MUST 1, SHOULD 1

- `Part XII — Verification and generated artifacts > 77. Exhaustive combinational verification` [2595-2626]
  Preview: For `Exactly`, `AtMost`, and `AllEqual`, tests MUST exhaustively enumerate all level valuations for bounded arities.
  Symbols: `Exactly`, `AtMost`, `AllEqual`
  Normative: MUST 3

- `Part XII — Verification and generated artifacts > 78. Exhaustive stateful verification` [2627-2674]
  Preview: Tests enumerate: Tests cover: Tests cover:

- `Part XII — Verification and generated artifacts > 78. Exhaustive stateful verification > 78.1 Pulse-resettable toggle` [2629-2641]
  Preview: Tests enumerate:

- `Part XII — Verification and generated artifacts > 78. Exhaustive stateful verification > 78.2 Level-resettable toggle` [2642-2656]
  Preview: Tests cover:

- `Part XII — Verification and generated artifacts > 78. Exhaustive stateful verification > 78.3 Level-resettable sample hold` [2657-2674]
  Preview: Tests cover:

- `Part XII — Verification and generated artifacts > 79. Canonical expansion verification` [2675-2690]
  Preview: Tests MUST verify that: - descriptor generation is deterministic; - insertion order does not change the expansion; - variadic reordering with stable keys does not change the expansion fingerprint; - every generated key is unique; - every role maps to the required primitive kind; - public dependency signatures equal expanded graph dependencies; - persisted expansion mismatch is rejected; - direct semantic internal edits are rejected while standard identity remains; - generated documentation diagrams match the descriptor graph.
  Normative: MUST 1, SHOULD 1

- `Part XII — Verification and generated artifacts > 80. Reconfiguration verification` [2691-2711]
  Preview: For every module, tests include: The production path is compared with complete expansion, ordinary stable-key correspondence, and the reference topology-patch path.

- `Part XII — Verification and generated artifacts > 81. Persistence verification` [2712-2730]
  Preview: Required tests include:

- `Part XII — Verification and generated artifacts > 82. Machine-readable catalogue outputs` [2731-2749]
  Preview: The authoritative descriptor source MUST generate or validate: Generated presentation artifacts are not semantic persistence artifacts.
  Normative: MUST 1

- `Part XII — Verification and generated artifacts > 83. Release gate` [2750-2764]
  Preview: A descriptor may enter the released standard catalogue only when: - its public law is complete; - its canonical expansion is fixed; - its internal roles and keys are fixed; - all blocking and non-blocking conditions are classified; - its persistence and migration rules are complete; - its conformance suite passes; - its generated documentation and descriptor data agree; - its addition does not alter existing descriptor fingerprints.

- `Appendix A — Catalogue version 1 summary` [2765-2775]
  Preview: | ID | Public name | Category | State | Temporal work | Parameters | |---|---|---|---|---|---| | `mossignal.standard.exactly` | `Exactly` | Variadic level combinational | None | None | `threshold: u64` | | `mossignal.standard.at_most` | `AtMost` | Variadic level combinational | None | None | `threshold: u64` | | `mossignal.standard.all_equal` | `AllEqual` | Variadic level combinational | None | None | None | | `mossignal.standard.pulse_resettable_toggle` | `PulseResettableToggle` | Stateful | Two stored levels | None | `initial: LogicLevel` | | `mossignal.standard.level_resettable_toggle` | `LevelResettableToggle` | Stateful | Two stored levels plus reset observation | None | `initial: LogicLevel` | | `mossignal.standard.level_resettable_sample_hold` | `LevelResettableSampleHold` | Stateful | Held level plus reset observation | None | `initial: LogicLevel`, `reset_to: LogicLevel` |
  Symbols: `initial: LogicLevel`, `threshold: u64`, `mossignal.standard.exactly`, `Exactly`, `mossignal.standard.at_most`, `AtMost`, `mossignal.standard.all_equal`, `AllEqual`, `mossignal.standard.pulse_resettable_toggle`, `PulseResettableToggle`, `mossignal.standard.level_resettable_toggle`, `LevelResettableToggle`, `mossignal.standard.level_resettable_sample_hold`, `LevelResettableSampleHold`, `reset_to: LogicLevel`

- `Appendix B — Standard migration summary` [2776-2794]
  Preview: | Change | Standard result | |---|---| | Same exact descriptor and parameters | Preserve compatible internal state | | Combinational threshold change | Regenerate stateless expansion and reevaluate | | Combinational variadic port reorder | Preserve by stable public keys | | Combinational variadic port add/remove | Add/remove corresponding stateless incidence | | Stateful `initial` change | Preserve runtime state; new value applies only to fresh state or a migration `Reset` outcome | | Sample-hold `reset_to` change while reset is `Low` | Preserve runtime state; apply the new target to future reset-controlled captures | | Sample-hold `reset_to` change while reset is `High` | Migrate held state immediately to the new target | | Stateful topology-migration `Reset` | Reset all internal state; report discarded source state | | Fixed public port change | Reject as descriptor mismatch | | Expansion version change | Reject unless an explicit compatibility entry exists | | Semantic version change | Reject unless an explicit compatibility entry exists | | Module id change | No automatic correspondence | | Instance key change | Require explicit module-instance reassociation | | Semantic internal edit retaining standard origin | Reject |
  Symbols: `Reset`, `reset_to`, `initial`, `Low`, `High`

- `Appendix C — Convenience classification summary` [2795-2821]
  Preview: | Convenience | Classification | |---|---| | `xor` | Primitive alias to `Parity` | | `debounce` | Primitive alias to `InertialDelay` | | `nand` | Builder-only composition | | `nor` | Builder-only composition | | `xnor` | Builder-only composition | | `level_gate` | Primitive alias to `All` | | low-enabled gate variants | Not separately supplied; explicit inversion plus canonical gate | | `majority` | Primitive alias to `AtLeast` | | `any_pulse` | Builder-only `Merge` plus `Coalesce` | | `annotate_signal` | Metadata-only | | `Exactly` | Standard module | | `AtMost` | Standard module | | `AllEqual` | Standard module | | pulse-resettable toggle | Standard module | | level-resettable toggle | Standard module | | level-resettable sample-and-hold | Standard module | | pulse multiplication | Deferred | | pulse cap | Deferred | | pulse parity | Deferred | | multi-way selection and routing | Deferred | | pulse-resettable sample-and-hold | Deferred | | one-shots, timeout, watchdog, stretcher, limiter | Deferred |
  Symbols: `xor`, `Parity`, `debounce`, `InertialDelay`, `nand`, `nor`, `xnor`, `level_gate`, `majority`, `AtLeast`, `any_pulse`, `Merge`, `Coalesce`, `annotate_signal`, `Exactly`, `AtMost`, `AllEqual`

- `Appendix D — Design consequences` [2822-2830]
  Preview: The initial catalogue is intentionally conservative.
