# Rust to Swift Compiler

**A high-assurance, source-to-source compiler producing independently executable Swift for a constrained Rust subset.**

This repository implements the Rust → Swift compiler defined in [`docs/`](docs/), adopting the formal operating contracts from [`formal-project-bootstrap`](../_formal-project-bootstrap/formal-project-bootstrap) and integrating the Compiler Semantic Graph (CSG) exchange model from [`reflective-rust`](../reflective-rust).

Unlike FFI binding generators or C ABI shims, this compiler produces standalone Swift packages that compile natively with `swiftc` without linking the source Rust library or runtime.

---

## Architecture Pipeline

```text
Rust Source (.rs)
      │
      ▼
┌─────────────────────────┐
│   rust-swift-frontend   │  AST parsing (syn) + Language Matrix policy rejection
└────────────┬────────────┘  Emits stable diagnostic codes (E0001..E0009)
             │
             ▼
┌─────────────────────────┐
│      rust-swift-ir      │  Target-neutral typed Semantic IR
└────────────┬────────────┘  Direct projection into Compiler Semantic Graph (CSG)
             │
             ▼
┌─────────────────────────┐
│    rust-swift-codegen   │  Lowers to Swift AST & emits standalone Swift package
└────────────┬────────────┘  Natively maps wrapping operations to `&+`, `&-`, `&*`
             │
             ▼
┌─────────────────────────┐
│   Swift Package (.swift)│  Compiles with `swift build` without Rust runtime
└─────────────────────────┘
```

---

## Workspace Crates

| Crate | Purpose |
|---|---|
| [`crates/rust-swift-ir`](crates/rust-swift-ir) | Target-neutral typed semantic IR with `reflective-rust-csg` Compiler Semantic Graph bridge |
| [`crates/rust-swift-frontend`](crates/rust-swift-frontend) | Syntax parser, lowering pass, and language matrix rejection with stable diagnostic codes |
| [`crates/rust-swift-codegen`](crates/rust-swift-codegen) | Deterministic Swift code emitter, `Package.swift` generator, and symbol mapping |
| [`crates/rust-swift-qualification`](crates/rust-swift-qualification) | Differential test harness executing Rust and Swift binaries on identical test vectors |
| [`crates/rust-swift-cli`](crates/rust-swift-cli) | Compiler CLI (`rust-swift compile`, `rust-swift diff-test`, `rust-swift manifest`) |

---

## Semantic Preservation Matrix (v0.1)

| Rust Construct | Lowering to Swift | Semantic Invariant |
|---|---|---|
| `x.wrapping_add(y)` | `(x &+ y)` | Two's complement wrapping modulo word size |
| `x.wrapping_sub(y)` | `(x &- y)` | Two's complement wrapping modulo word size |
| `x.wrapping_mul(y)` | `(x &* y)` | Two's complement wrapping modulo word size |
| `x + y`, `x - y` | `(x + y)`, `(x - y)` | Checked arithmetic, traps on overflow |
| `enum State { ... }` | `enum State: String, CaseIterable, ...` | Direct case preservation |
| `struct Point { ... }` | `struct Point: Equatable, Hashable, Sendable` | Value type semantics |
| `match (s, e) { ... }` | `switch (s, e) { case (.A, .B): ... }` | Exhaustive pattern match |
| `unsafe { ... }` | **Rejected (`E0001_UNSAFE_REJECTED`)** | No silent unsafe emission |
| `async fn` | **Rejected (`E0002_ASYNC_REJECTED`)** | Pure synchronous scope |
| `*const T`, `*mut T` | **Rejected (`E0003_RAW_POINTER_REJECTED`)** | Memory-safe baseline |

---

## Quick Start & Verification

### 1. Build Workspace
```bash
cargo build --workspace
cargo test --workspace
```

### 2. Run Compiler CLI
```bash
# Inspect reproducibility manifest
cargo run -p rust-swift-cli -- manifest

# Compile an admitted Rust file into a standalone Swift package
cargo run -p rust-swift-cli -- compile examples/fsm_example.rs -o examples/GeneratedFSM --emit-csg --emit-ir

# Build the generated Swift package with SwiftPM
cd examples/GeneratedFSM && swift build
```

### 3. Run Differential Qualification Suite
```bash
cargo run -p rust-swift-cli -- diff-test
```

This runs:
- **Finite State Machine Differential Matrix**: Tests all 16 state/event combinations against the Rust reference implementation and compiled Swift executable.
- **Wrapping Arithmetic Differential Vectors**: Tests 36 edge cases (`0`, `MAX`, `MIN`, overflow points) against Rust and Swift.
- **Negative Diagnostics**: Proves that rejected constructs fail with stable, actionable diagnostic codes.
- **Formal Evidence Receipts**: Automatically appends verifiable qualification records to [`records/evidence.jsonl`](records/evidence.jsonl).

---

## Formal Bootstrap Compliance

This repository is governed under [`formal-project-bootstrap`](../_formal-project-bootstrap/formal-project-bootstrap):
- [`PROJECT_INTENT.md`](PROJECT_INTENT.md): Project intent, scope, constraints, and non-goals.
- [`FORMAL_RESOURCE_MANIFEST.json`](FORMAL_RESOURCE_MANIFEST.json): Pinned specification documents and CSG exchange schema.
- Verification script: `python3 scripts/validate-bootstrap.py`.
- Compiled context generator: `python3 scripts/gen-context.py`.
