# Initial Implementation Orientation

**Status:** Provisional, non-normative implementation guidance  
**Defines:** Sensible default directions and guidance for early implementation  
**Audience:** Agents working on the initial `mossignal` implementation  
**Authority:** The specifications under `docs/specs/` remain authoritative

## Purpose

This document provides a sensible default direction for the early implementation of `mossignal`.

It is not another specification. It does not add semantics, settle public API questions, or require one permanent private architecture. It exists because the specifications intentionally leave ordinary implementation choices open, while an empty repository gives an implementation agent too many equally plausible ways to begin.

Use this guide when the specifications permit several implementation shapes and the current codebase has not yet established a stronger local convention.

The intended approach is:

> Build the simplest concrete implementation that directly expresses the specified model, supports a thin end-to-end slice, and leaves the important semantic distinctions intact.

Prefer neither a throwaway prototype nor a generalized framework for hypothetical future needs.

This guidance is most relevant during the initial implementation phase. As real code and stronger local patterns emerge, follow the mature repository where it remains consistent with the specifications. A material departure from these defaults is acceptable, but the implementation evidence should briefly explain why the current repository makes the alternative preferable.

## 1. Organize around real responsibilities

Begin with one cohesive library crate and ordinary Rust modules.

Create modules around recognizable domain responsibilities that own meaningful invariants, such as:

- signal and logical-time value types;
- stable structural identity;
- authored network definitions;
- typed authoring;
- validation and compilation;
- runtime state and transaction execution;
- inspection, explanation, persistence, and reconfiguration when those systems become real.

These are conceptual boundaries, not an instruction to create every possible file immediately.

Start relatively flat. Introduce a submodule when one of the following becomes true:

- a responsibility can be understood and tested independently;
- a file contains several distinct concepts with different owners;
- private implementation detail obscures the public or subsystem boundary;
- several files need a stable internal vocabulary that belongs to one subsystem.

Do not split merely because a specification has another section, because a type has another enum variant, or because a primitive has another name.

A primitive such as `Not`, `All`, or `Toggle` does not normally require its own class-like subsystem. The primitive universe is closed and several primitives share the same definition, compilation, evaluation, inspection, and testing machinery. Group code by the responsibility being performed and by semantic family where that improves clarity.

Keep a type near the subsystem that owns its invariants. Move it into a more foundational module only after multiple real consumers establish that it is genuinely shared.

Use private or `pub(crate)` implementation detail by default. Public organization is governed by the concrete Rust API specification and should not emerge accidentally from internal file layout.

## 2. Default early compiled model

Unless the current implementation gives a concrete reason to do otherwise, use the following straightforward model as the initial direction.

```text
stable-keyed authored structure
        ↓ validation
validated semantic definition
        ↓ compilation
immutable dense executable topology
        +
mutable semantic machine state
        ↓ transaction
full deterministic topological reaction evaluation
        ↓
atomic publication
```

The initial compiled representation should naturally contain equivalents of:

- stable-key-to-dense resolution tables;
- dense level, pulse, state, and temporal positions as they become necessary;
- a closed reaction-operation representation;
- the current-reaction dependency graph;
- one deterministic topological operation order;
- the adjacency and reverse metadata required by actual compilation, runtime, diagnostics, inspection, or graph queries.

The exact structs, integer widths, vectors, maps, arenas, and sharing mechanism remain implementation choices.

A plausible closed operation representation might eventually contain variants shaped approximately like:

```rust
enum ReactionOperation {
    Not {
        input: LevelSlot,
        output: LevelSlot,
    },
    All {
        inputs: InputRange<LevelSlot>,
        output: LevelSlot,
    },
    // Other primitive operations.
}
```

This is illustrative, not a required signature. The important default is a concrete, closed executable representation over dense positions—not one heap allocation and dynamic-dispatch object per node.

`CompiledNetwork` should represent immutable executable topology. A spawned `Machine` should own independent mutable semantic state. Authored stable identity and compiled dense identity serve different purposes and should not be collapsed into one representation.

## 3. What it mechanically means for a signal to pass through a gate

A signal is not a message object that physically travels from node to node.

For a level signal, the processor maintains a current semantic value associated with a source or compiled value slot. A compiled reaction operation reads settled predecessor values and computes another current value. Dependency order determines when that operation is ready to evaluate.

Consider this network:

```text
external input a ──→ Not ──┐
                            ├──→ All ──→ external output result
external input b ───────────┘
```

A straightforward compilation might assign:

```text
level slot 0 = external input a
level slot 1 = external input b
level slot 2 = Not output
level slot 3 = All output
```

and produce this deterministic topological operation sequence:

```text
Not { input: 0, output: 2 }
All { inputs: [2, 1], output: 3 }
```

During initialization or a full reference reaction:

1. The transaction establishes the complete external input facts in the candidate level storage.
2. `Not` reads the settled value in slot `0`.
3. `Not` writes its complete current result to slot `2`.
4. `All` runs afterward because its predecessors are now settled.
5. `All` reads slots `2` and `1`.
6. `All` writes `High` to slot `3` exactly when both inputs are `High`.
7. The external output reads slot `3`.
8. The transaction constructs the required output event, state changes, future work, diagnostics, and provenance.
9. The complete candidate result commits atomically.

Conceptually, the full evaluator may be as direct as:

```rust
for operation in compiled.topological_operations() {
    match operation {
        ReactionOperation::Not { input, output } => {
            levels[output] = levels[input].invert();
        }

        ReactionOperation::All { inputs, output } => {
            levels[output] = if inputs
                .iter()
                .all(|input| levels[*input].is_high())
            {
                LogicLevel::High
            } else {
                LogicLevel::Low
            };
        }

        // Remaining closed primitive operations.
    }
}
```

The real implementation will also need the specified failure, inspection, explanation, diagnostic, state, and transaction behavior. This example explains only the central execution mechanism.

For pulses, the same general model applies, but a reaction-scoped pulse slot contains a complete simultaneous multiplicity rather than persistent state. A pulse operation reads counts for the current reaction and computes a count for downstream operations. No individual same-time pulse token travels through an ordered sequence.

For stateful nodes, evaluation reads previous committed state and current settled inputs, computes its current output, and records at most one proposed successor for each state cell. Downstream operations may read the current output during the same reaction. The successor state becomes stored state only when the reaction commits.

For temporal nodes, current input may create strictly future work, while obligations already due at the current logical time act as reaction inputs according to the node's specified dependency and exact-deadline rules.

## 4. Implement primitive semantics through shared machinery

Each primitive eventually participates in several distinct concerns:

- authored definition and configuration;
- static validation;
- current-reaction dependency construction;
- compiled operation data;
- current output and successor-state law;
- future-work handling where applicable;
- diagnostics and explanation;
- inspection and persistence projections;
- reconfiguration compatibility;
- conformance verification.

Do not force all of those concerns into one monolithic per-node object.

Prefer shared subsystem machinery with closed primitive-specific cases. For example:

- the definition layer owns the closed node-kind and configuration values;
- validation matches node kinds to arity, parameter, and dependency rules;
- compilation converts stable authored structure into dense descriptors;
- the evaluator dispatches over closed compiled operations;
- inspection and explanation project from the same semantic identities and committed facts;
- conformance tests exercise the complete obligations for each primitive family.

Keep the local law for a basic primitive simple. `Not` should remain an inversion. `All` should remain an all-input reduction with its specified zero-arity result. Complexity belongs only where the surrounding system genuinely requires it.

Avoid duplicating the same semantic rule independently in several places when one shared domain definition or small pure helper can express it without coupling the reference and optimized control paths too tightly.

## 5. Build reference behavior before optimization

The first correct implementation of a subsystem should usually be the straightforward reference form required by the verification policy.

In particular:

- compile the complete definition rather than incrementally patching derived compilation data;
- evaluate every reaction operation once in deterministic topological order;
- use complete weak-region recomputation;
- use clone-and-swap transaction execution when establishing transaction semantics;
- use an ordered semantic event calendar when temporal behavior first appears;
- compute fresh inspection projections before caching or incrementally maintaining them.

These paths are not temporary disposable prototypes. They are long-lived correctness oracles.

Incremental reaction evaluation should come later. It should reuse the same primitive laws but use a different scheduling strategy:

```text
changed source facts
    ↓
affected operations become dirty
    ↓
dirty operations are processed in topological order
    ↓
changed outputs propagate dirtiness
    ↓
result is compared with full topological evaluation
```

Do not replace the reference path when adding an optimized path. The optimized implementation must remain demonstrably equivalent to it.

## 6. Prefer thin vertical progress

Early work should produce small coherent capabilities that pass through the real lifecycle rather than broad layers with no executable consumer.

A useful slice may cross several modules:

```text
author
→ validate
→ compile
→ initialize or execute
→ inspect the result
→ verify the specified behavior
```

It is acceptable for an early slice to support only the primitive family and lifecycle behavior named by its bead. It is not necessary to build temporal execution, persistence, provenance, reconfiguration, and every public convenience before the first combinational network can run.

At the same time, preserve distinctions that later semantics depend upon:

- stable identity versus dense execution position;
- authored structure versus compiled topology;
- immutable compiled program versus mutable machine state;
- current values versus previous committed state;
- proposed successor state versus published state;
- persistent levels versus reaction-scoped pulse counts;
- pending future work versus current signal values;
- structured semantic failure versus internal invariant violation.

Preserving these distinctions is enough to avoid obvious dead ends. It does not require implementing every future subsystem early.

## 7. Let the implementation reveal abstractions

Prefer a concrete solution for the current semantic family.

Introduce a reusable helper or abstraction when real code reveals repeated structure or one durable replacement boundary. An abstraction should make the specified model easier to see, not replace it with a vocabulary invented for generic graph engines, plugin systems, or hypothetical evaluators.

Good early abstractions are likely to describe actual `mossignal` concepts, such as:

- typed stable keys;
- dense slot newtypes;
- validated reports;
- compiled operation descriptors;
- previous and proposed state access;
- canonical semantic observations used for differential comparison.

A trait is most useful when there is a true replacement point or I/O boundary. Closed semantic variation should generally remain a closed enum and concrete dispatch unless the implementation produces a strong contrary reason.

## 8. Keep future requirements visible but inactive

When implementing an early subsystem, read the adjacent specifications so that the design does not erase information later requirements need.

For example:

- compiled dense positions must remain reconstructible from stable identity;
- state storage must be inspectable and persistable by semantic identity;
- node evaluation must be able to produce diagnostics and causal facts without invoking host callbacks;
- temporal work must eventually have stable identity independent of calendar position;
- reconfiguration must eventually classify state and pending work explicitly;
- optimized execution must remain comparable with reference execution.

These are design pressures, not instructions to implement all adjacent systems in the current bead.

Do not add placeholders, generalized callback hooks, compatibility layers, or public abstractions merely to claim future extensibility. Leave room by keeping the semantic boundaries clean.

## 9. Departing from these defaults

A private implementation choice may depart from this guide when the current code demonstrates a better fit.

Record a brief explanation in the implementation evidence when the departure is material:

- which default was not followed;
- what concrete repository condition made it unsuitable;
- which specified invariant the alternative preserves;
- how the alternative remains testable against the required reference behavior.

If the departure would change public architecture, observable semantics, identity, persistence compatibility, migration behavior, or failure policy, do not treat it as ordinary implementation freedom. Resolve it from the authoritative specifications or surface the exact design blocker.

## 10. Practical starting posture

For each early bead:

1. Read its cited specification sections and the adjacent invariants they affect.
2. Inspect the current code and continue its real boundaries where they are sound.
3. Choose the smallest concrete representation that completes the bead's vertical behavior.
4. Add modules only when the implemented responsibility warrants them.
5. Establish or extend the straightforward reference path.
6. Verify the complete specified result, including boundary cases and negative behavior.
7. Explain any material departure from this provisional direction.

The objective is not to predict the final repository perfectly.

The objective is to make each accepted implementation slice correct, understandable, and naturally extensible by the next one.
