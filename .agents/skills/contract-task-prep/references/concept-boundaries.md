# Choosing coherent contract subjects

A contract subject is a reusable unit of product truth, not a file, task, type, method, or individual rule.

Use these tests together.

## Positive tests

A subject is probably coherent when:

- an agent could naturally ask, “What is the reusable contract for X?”;
- future tasks are likely to reference X independently;
- its rules should normally be inspected together when one of them changes;
- it has a stable identity or name across documents;
- it can be summarized in one or two sentences;
- its contract can remain reasonably compact.

Examples:

- `DiagnosticPath`;
- the stable structural key family;
- `NetworkFingerprint`;
- machine lifecycle;
- current-reaction causality;
- `InputSnapshot` and `InputDelta` as one closely coupled family;
- `Toggle`;
- an edge-detector family when the members share one schema and differ only in a small law.

## Too small

Do not create a contract for:

- one method such as `DiagnosticPath::len`;
- one rule such as “path order matters”;
- one test case;
- one diagnostic enum variant when the namespace or evidence family is the coherent subject;
- one task-specific implementation step.

Place those facts inside the enclosing subject contract.

## Too broad

Avoid subjects such as:

- all persistence;
- all runtime behavior;
- the entire public API;
- all built-in nodes;
- the whole diagnostic system.

Split when independently useful subcontracts have distinct invariants, lifecycles, consumers, or change patterns.

## Family records

Use one family contract when members:

- share the same semantic category and record shape;
- are normally implemented and reviewed together;
- differ only in a bounded table or equation;
- would otherwise repeat most of the contract.

Keep members separate when they have materially different state, lifecycle, persistence, migration, or compatibility rules.

## Split and merge heuristics

Consider splitting when:

- the record exceeds roughly 200 lines because it contains independently reusable subjects;
- most future changes would affect only one isolated portion;
- separate agents could implement the parts without needing the other contract;
- one portion has different ownership or lifecycle rules.

Consider merging when:

- two records always cite the same sources and repeat the same invariants;
- neither record is meaningful without the other;
- every realistic task would reference both;
- the separation exists only because the Rust API uses two types.

Contract boundaries are revisable. Prefer a defensible, compact boundary over false precision.
