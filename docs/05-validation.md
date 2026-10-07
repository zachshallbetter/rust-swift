Rust → Swift Compiler | Empirical Qualification & Validation Doctrine | 14 October 2026

## Epistemic Integrity & Claim Taxonomy

This repository strictly enforces the Four-Tier Claim Taxonomy:

$$\text{Documented} \neq \text{Implemented} \neq \text{Tested} \neq \text{Empirically Validated}$$

- **No Premature Promotion**: A feature is not declared complete until qualified by differential execution evidence.
- **Fail-Closed Gate Design**: Every qualification gate must be proven capable of detecting divergence and failing when fed mutated code.

## Verification Tiers

### 1. Differential Execution Qualification (`rust-swift diff-test`)
Executes parallel reference binaries (`rustc` and `swiftc`) across identical inputs in isolated sandboxes:
- **State Machines**: 16 exhaustive state/event transition paths.
- **Wrapping Arithmetic**: 36 boundary overflow vectors for fixed-width integers.
- **Contracts & Invariants**: Positive assertion enforcement and identical process panic/precondition trap behavior.
- **Event Trace Model**: 50-step deterministic event history recording and byte-for-byte digest verification.
- **Data Semantics**: `Option`, `Result`, and Unicode string scalar slicing.
- **Ownership & Regions**: Exclusivity verification for `&mut` to `inout` conversions.
- **Iteration & Dynamic Collections**: `for`-in loops and dynamic `Vec<T>` operations.
- **Protocols & Traits**: Trait declarations and protocol extension default method dispatch.
- **SPM Package Builds**: Clean-room `swift build` and `swift test` execution across emitted package layouts.
- **Codable JSON Serialization**: Round-trip encode/decode parity between `serde_json` and Swift `JSONEncoder`/`JSONDecoder`.
- **SwiftUI `@Observable`**: State mutation change observation with `withObservationTracking`.
- **Property Fuzzing Oracle**: 10,000 continuous pseudo-random iterations verifying arithmetic invariants.
- **Negative Diagnostics**: Verification of rejections for `E0001` through `E0012`.

### 2. Failure Injection & Mutation Matrix (`rust-swift eval --mutations`)
Enforces the **"Gates That Cannot Fail"** doctrine:
- Synthesizes 6 distinct defect classes (`MUT-001` through `MUT-006` covering operator inversions, FSM transition corruption, invariant violations, schema drift, observable state suppression, and off-by-one loops).
- Confirms that every qualification gate fails closed as expected and records receipts permanently in `records/negative-results.jsonl`.

### 3. Differential Performance Benchmarking (`rust-swift bench`)
- **Compiler Throughput**: Measures syn parsing, CSG IR lowering, and Swift emission rates (lines/sec, KB/sec).
- **Runtime Parity**: Compares release-optimized binaries (`rustc -O` vs `swiftc -O`) across 100k arithmetic iterations, 25k vector transforms, and 50k FSM cycles, asserting bit-for-bit checksum equality.
- Telemetry is recorded in `records/benchmarks.jsonl`.

### 4. Adversarial Stress & No-Op Protection (`rust-swift eval --adversarial`)
- Stress tests 32 levels of recursive AST expression nesting, extreme Unicode (ZWJ clusters, multi-script UTF-8), and integer bounds (`i64::MIN/MAX`, `u64::MAX`).
- Enforces **No-Op Protection**: CI workflows verify that at least 20+ assertions execute, preventing skipped or false-green CI executions.
- Structured receipts are logged to `records/evidence.jsonl`.
