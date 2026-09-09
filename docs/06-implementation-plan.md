Rust → Swift Compiler \| Proposal v0.1 \| 13 October 2026

## M0 - feasibility

Pin toolchains; evaluate rustc-driver integration; prove extraction of typed functions and spans; document unsupported compiler-version assumptions.

## M1 - vertical slice

Translate pure finite-state functions and fixed-width wrapping arithmetic. Emit standalone Swift Package and run exhaustive comparison.

## M2 - structural language

Add structs, payload enums, nested match, early returns, loops, and explicit error diagnostics. Establish stable semantic IR schema.

## M3 - data semantics

Add Option/Result, UTF-8 operations, selected Vec behaviors, and generics via specialization. Build a minimal compatibility runtime.

## M4 - ownership

Introduce borrow classification and scope cleanup for supported cases. Reject unsupported aliasing and Drop behavior.

## M5 - hardening

Reproducible builds, source maps, fuzzing, minimized failures, incremental compile tests, documentation, and performance baselines.

## Exit criteria

Each milestone ends with source fixtures, compiled Swift, differential receipts, known exclusions, and review of code-generation quality.

## Key risks

rustc internal API churn; semantic gaps in ARC/ownership; standard-library mismatches; code explosion from monomorphization; test oracles that omit observable behavior.
