# Rust to Swift Compiler

[![CI](https://github.com/zachshallbetter/rust-swift/actions/workflows/ci.yml/badge.svg)](https://github.com/zachshallbetter/rust-swift/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-2021%20edition-orange.svg)](https://www.rust-lang.org/)
[![Swift](https://img.shields.io/badge/swift-5.9%2B-brightgreen.svg)](https://swift.org/)

**A high-assurance, source-to-source compiler producing independently executable Swift packages from a formally verified Rust subset.**

---

## Overview

`rust-swift` translates pure Rust domain logic, finite state machines, data structures, and algorithms into clean, idiomatic, standalone Swift packages. 

Unlike traditional FFI binding generators or C ABI wrappers, this compiler emits native Swift source code that compiles directly with `swiftc` or Swift Package Manager without linking the original Rust compiler, runtime, or intermediate C bridging layer.

### Key Capabilities

- **Zero C Bridging / Zero FFI**: Output is 100% native Swift value types (`struct`, `enum`, `Array`) with copy-on-write semantics.
- **Formal Invariant Preservation**: Rust integer wrapping semantics (`wrapping_add`, `wrapping_sub`, `wrapping_mul`) lower directly to Swift overflow operators (`&+`, `&-`, `&*`).
- **Bidirectional Source Mapping**: Emits native `#sourceLocation` directives so Xcode, LLDB, and crash logs map directly back to the original Rust source files and line numbers.
- **Compiler Semantic Graph (CSG)**: Emits target-neutral semantic provenance and CSG graphs compatible with the Reflective Rust architecture.
- **Empirical Differential Qualification**: A built-in differential harness executes parallel reference binaries and verifies bit-for-bit execution equality (including a 10,000-iteration pseudo-random property fuzzing oracle).

---

## Architecture Pipeline

```text
       Rust Source (.rs)
              │
              ▼
    ┌─────────────────────────┐
    │   rust-swift-frontend   │  AST parsing (syn) + Language Matrix admission pass
    └─────────────┬───────────┘  Emits deterministic diagnostic codes (E0001..E0012)
                  │
                  ▼
    ┌─────────────────────────┐
    │      rust-swift-ir      │  Target-neutral typed Semantic Intermediate Representation
    └─────────────┬───────────┘  CSG projection & ownership/region classification
                  │
                  ▼
    ┌─────────────────────────┐
    │    rust-swift-codegen   │  Lowers IR to Swift AST & standalone Swift Package
    └─────────────┬───────────┘  Emits `#sourceLocation` directives & `Package.swift`
                  │
                  ▼
    ┌─────────────────────────┐
    │   Native Swift Package  │  Compiles with `swift build` / `swiftc` (Zero FFI)
    └─────────────────────────┘
```

---

## Admitted Feature Matrix & Semantic Contracts

The compiler operates under a strict, formally specified admission boundary. Admitted features map to normative contracts in [`contracts/`](contracts/):

| Rust Construct | Lowering to Swift | Semantic Contract |
|---|---|---|
| `x.wrapping_add(y)` | `(x &+ y)` | Two's complement modulo word size ([`contracts/contract-execution.yaml`](contracts/contract-execution.yaml)) |
| `x.wrapping_sub(y)` | `(x &- y)` | Two's complement modulo word size ([`contracts/contract-execution.yaml`](contracts/contract-execution.yaml)) |
| `x.wrapping_mul(y)` | `(x &* y)` | Two's complement modulo word size ([`contracts/contract-execution.yaml`](contracts/contract-execution.yaml)) |
| `x + y`, `x - y` | `(x + y)`, `(x - y)` | Checked arithmetic, traps on overflow |
| `enum State { ... }` | `enum State: String, CaseIterable, ...` | Fieldless enum case preservation |
| `enum Event { Move(i64, i64) }` | `enum Event: Equatable, Hashable, Sendable` | Payload enum with associated values |
| `struct State { ... }` | `struct State: Equatable, Hashable, Sendable` | Immutable or mutating value types |
| `impl Struct { fn method(...) }` | `extension Struct { func method(...) }` | Inherent methods & static initializers |
| `for item in items { ... }` | `for item in items { ... }` | Sequence iteration ([`contracts/iteration-collections.yaml`](contracts/iteration-collections.yaml)) |
| `Vec::new()`, `vec.push(x)` | `[]`, `vec.append(x)` | Dynamic collection lifecycle |
| `Option<T>`, `Result<T, E>` | `Optional<T>`, `Result<T, E>` | Monadic unwrapping and error propagation ([`contracts/data-semantics.yaml`](contracts/data-semantics.yaml)) |
| `&mut param` | `inout param` | Region ownership exclusivity ([`contracts/ownership-regions.yaml`](contracts/ownership-regions.yaml)) |
| `assert!`, `debug_assert!` | `precondition`, `assert` | Runtime contract verification |
| Source locations | `#sourceLocation(file:line:)` | Debugger source mapping ([`contracts/source-mapping.yaml`](contracts/source-mapping.yaml)) |

### Rejection Policy (E0001..E0012)

Constructs outside the verified boundary are rejected at parse time with stable, actionable diagnostic codes:
- **`E0001_UNSAFE_REJECTED`**: `unsafe` blocks and functions are forbidden.
- **`E0002_ASYNC_REJECTED`**: Asynchronous execution requires explicit runtime adapters.
- **`E0003_RAW_POINTER_REJECTED`**: Raw pointers (`*const T`, `*mut T`) are rejected.
- **`E0004_MACRO_REJECTED`**: Unadmitted macro invocations.
- **`E0005_DYN_TRAIT_REJECTED`**: Dynamic trait objects (`dyn Trait`).
- **`E0006_TRAIT_IMPL_REJECTED`**: Complex or aliased trait implementations.
- **`E0007_CUSTOM_DROP_REJECTED`**: Custom `Drop` implementations.
- **`E0008_UNION_REJECTED`**: Untagged unions.
- **`E0009_SYNTAX_ERROR`**: Rust syntax errors.
- **`E0010_INT128_REJECTED`**: 128-bit integers without native 64-bit target equivalents.
- **`E0011_GENERICS_REJECTED`**: Unbounded generics.
- **`E0012_INTERIOR_MUTABILITY_REJECTED`**: `Cell` and `RefCell` types.

---

## Workspace Structure

The project is organized as a Cargo workspace containing five modular crates:

```text
crates/
├── rust-swift-ir/             # Target-neutral semantic IR and CSG bridge
├── rust-swift-frontend/       # Syn AST parser, lowering pass, and diagnostic engine
├── rust-swift-codegen/        # Swift AST lowerer, Package.swift emitter, and shims
├── rust-swift-qualification/  # Differential qualification suite and fuzzing oracle
└── rust-swift-cli/            # Command-line interface (`rust-swift`)
```

---

## Getting Started

### Prerequisites

- **Rust**: 1.80+ (`cargo`, `rustc`)
- **Swift**: 5.9+ / Xcode 15+ (`swiftc`, `swift`)
- **Python**: 3.10+ (for verification scripts)

### Installation

Clone the repository and build the CLI tool:

```bash
git clone https://github.com/zachshallbetter/rust-swift.git
cd rust-swift
cargo build --release
```

The compiler binary is located at `target/release/rust-swift`.

---

## CLI Usage

### 1. Compile Rust to Swift

Compile an admitted Rust source file into a standalone, compilable Swift package:

```bash
cargo run -p rust-swift-cli -- compile examples/fsm_example.rs \
  --output-dir build/GeneratedFSM \
  --package-name GeneratedFSM \
  --source-locations \
  --emit-csg \
  --emit-ir
```

Output directory layout:
```text
build/GeneratedFSM/
├── Package.swift
├── manifest.json
├── ir.json
├── csg.json
└── Sources/
    └── GeneratedFSM/
        ├── fsm_example.swift
        └── RustCompat.swift
```

Build the generated Swift package using the standard Swift toolchain:

```bash
cd build/GeneratedFSM
swift build
```

### 2. Differential Qualification Suite

Execute the 11-stage differential qualification harness:

```bash
cargo run -p rust-swift-cli -- diff-test
```

This suite executes:
1. **Finite State Machine Differential Matrix**: 16 state/event transition paths.
2. **Wrapping Arithmetic Differential Vectors**: 36 boundary and overflow vectors.
3. **Contract & Invariant Traps**: Precondition, assertion, and panic trap parity.
4. **Type System & Enums**: Struct initializers, tuple fields, and payload enum patterns.
5. **Event Dispatch & Trace Model**: 50-step deterministic event history recording.
6. **Data Semantics**: `Option`, `Result`, and UTF-8 string slicing boundaries.
7. **Ownership & Region Classification**: `&mut` to `inout` exclusivity.
8. **Iteration & Dynamic Collections**: `for-in` loops and `Vec<T>` mutations (`push`, `pop`, `clear`).
9. **Source Mapping & Traceability**: `#sourceLocation` validation and crash line mapping.
10. **Property-Based Differential Fuzzing**: 10,000 continuous pseudo-random iterations.
11. **Negative Rejection Policy**: Exact verification of stable diagnostic codes (E0001..E0012).

Receipts are cryptographically recorded in [`records/evidence.jsonl`](records/evidence.jsonl).

### 3. Toolchain & Environment Manifest

Inspect the current reproducible toolchain versions and commit digests:

```bash
cargo run -p rust-swift-cli -- manifest
```

---

## Real-World Applications & Examples

The repository includes real-world demonstrations proving that generated Swift code integrates directly with Apple's platform frameworks:

### 1. Native macOS SwiftUI Player (`examples/PlayerApp`)
A desktop media playback application built in SwiftUI on macOS Sonoma/Sequoia. All finite state machine logic (play, pause, stop, track advancement, volume clamping) is compiled directly from pure Rust:
- **Source**: [`examples/fsm_example.rs`](examples/fsm_example.rs)
- **App**: [`examples/PlayerApp/App.swift`](examples/PlayerApp/App.swift)

### 2. Peel Core Capabilities Matrix (`examples/PeelInspectorApp` & `examples/PeelCoreKit`)
Extracts and translates archive probing, extraction capabilities, and path extension detection from the Peel Core format engine (16 archive formats, 9 container types) into native Swift:
- **Source**: [`examples/peel_core.rs`](examples/peel_core.rs)
- **Swift Package**: [`examples/PeelCoreKit`](examples/PeelCoreKit)
- **XCTest Suite**: 8 unit tests validating formats with zero failures.

---

## Operating Doctrine & Status Honesty

This repository enforces the **Four-Tier Claim Taxonomy**:

$$\text{Documented} \neq \text{Implemented} \neq \text{Tested} \neq \text{Empirically Validated}$$

- **Zero Silent Failure**: Any Rust construct not formally admitted is rejected at compile time.
- **Deterministic Code Generation**: Given identical Rust source input, code generation produces identical, byte-for-byte Swift source code.
- **Evidence-Based Landing**: No PR is merged without differential execution evidence demonstrating behavioral parity.

---

## License

This project is licensed under either of:

- Apache License, Version 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([`LICENSE-MIT`](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
