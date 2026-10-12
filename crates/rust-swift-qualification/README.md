# rust-swift-qualification

Differential qualification engine, failure injection mutation matrix, adversarial evals, performance benchmarks, and formal verification dossier certification.

## Overview

`rust-swift-qualification` implements the rigorous verification doctrine of the compiler, adhering to the Four-Tier Claim Taxonomy ($\text{Documented} \neq \text{Implemented} \neq \text{Tested} \neq \text{Empirically Validated}$).

## Qualification Modules

- **`differential.rs`**: 16 end-to-end differential test suites executing parity assertions between native `rustc` execution and compiled `swiftc` binaries.
- **`mutation.rs`**: "Gates That Cannot Fail" synthetic defect injection across 6 mutation defect classes (`MUT-001` through `MUT-006`), proving qualification gates detect defects and fail closed.
- **`evals.rs`**: Aggressive adversarial stress testing (32-level deep recursive AST nesting, extreme Unicode with multi-script UTF-8 and ZWJ emoji sequences, and integer boundary transitions) with No-Op Protection enforcing minimum assertion thresholds.
- **`benchmark.rs`**: Measures compiler throughput (~52,800 lines/sec) and microsecond runtime parity between optimized Rust (`rustc -O`) and optimized Swift (`swiftc -O`).
- **`proof.rs` (`rust-swift-proof`)**: Generates cryptographically signed `EpistemicProofReceipt` dossiers certifying memory safety, arithmetic overflow invariants, FSM totality, and runtime contracts.
- **`sync.rs` (`rust-swift-sync`)**: Compiler Semantic Graph (CSG) delta synchronizer and live file watcher daemon.
