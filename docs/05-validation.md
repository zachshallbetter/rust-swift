Rust → Swift Compiler \| Proposal v0.1 \| 13 October 2026

## Fixture design

Each fixture contains Rust source, input corpus, declared observation contract, expected diagnostics, and optional resource trace. Maintain positive and negative cases.

## Execution

Compile Rust and generated Swift with pinned configurations; execute isolated runners on identical inputs; canonicalize only declared irrelevant differences.

## Comparisons

Compare values, serialized bytes, error variants, traps, and ordered effects. Preserve failing seeds and minimized counterexamples.

## Test families

Exhaustive finite-state transition tables; integer boundaries and randomized arithmetic; Unicode byte offsets; match exhaustiveness; early returns; resource cleanup.

## Metamorphic checks

Use invariants such as round-trip encode/decode and state-machine reset behavior, independently of Rust/Swift equality.

## Receipt

Record source hash, compiler revisions, target, flags, fixture version, case count, pass/fail, counterexamples, and scope of the resulting claim.

## Gate

No promotion on successful compilation alone. New lowering rules require focused vectors, regression corpus, and reviewed semantic assumptions.
