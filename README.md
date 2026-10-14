# Rust to Swift Compiler

**A high-assurance, source-to-source compiler producing independently executable Swift packages from a formally verified Rust subset.**

[![Release: v0.1.0](https://img.shields.io/badge/release-v0.1.0-4da3ff)](https://github.com/zachshallbetter/rust-swift/releases)
[![CI](https://github.com/zachshallbetter/rust-swift/actions/workflows/ci.yml/badge.svg)](https://github.com/zachshallbetter/rust-swift/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust: 1.80+ (2021)](https://img.shields.io/badge/rust-1.80%2B%20%C2%B7%202021-orange.svg)](https://www.rust-lang.org/)
[![Swift: 5.9+ / 6.0](https://img.shields.io/badge/swift-5.9%2B%20%C2%B7%206.0-brightgreen.svg)](https://swift.org/)
[![Zero-FFI: 100% Native Swift](https://img.shields.io/badge/architecture-100%25%20Zero--FFI%20Native%20Swift-2dd4bf)](#the-idea)
[![Qualification: 16 Suites · 100% Green](https://img.shields.io/badge/qualification-16%20suites%20%C2%B7%20100%25%20green-2dd4bf)](#verification--gates-that-cannot-fail)
[![Agent Protocol: MCP Ready](https://img.shields.io/badge/agents-MCP%20stdio%20ready-blueviolet)](#ai-agent--mcp-integration-claude-codex-cursor-antigravity)

> The `rust-swift` compiler translates algorithmic Rust core logic, finite state machines, data structures, and state models into clean, standalone Swift 5.9+ packages. The output compiles directly with `swiftc` or Swift Package Manager—with **zero C-FFI bridging**, **zero intermediate shims**, and **full formal preservation of memory, arithmetic, and contract invariants**.

---

## The idea

Most cross-language bridging stories either rely on fragile C-FFI pointer gymnastics, complex runtime bindings, or simulated abstraction layers that sacrifice memory safety, introduce ARC/borrow-checker impedance mismatches, and incur heavy data marshaling overhead. `rust-swift` rejects that compromise by performing **pure source-to-source semantic compilation**:

| Capability | Traditional C-FFI / Bindings (`bindgen`, `uniffi`) | `rust-swift` High-Assurance Source Compilation |
|---|---|---|
| **Interoperability Model** | C ABI bridging headers, `unsafe` pointer dereferencing, manual dynamic link libraries (`.dylib`, `.so`) | **100% Zero-FFI Source Translation**: Emits idiomatic Swift 5.9+ value types, `@Observable` classes, and protocols |
| **Memory Management** | Clashing runtimes: Rust manual/RAII deallocators fighting Swift Automatic Reference Counting (ARC) | **Native Swift ARC & COW**: Structs lower to Swift value-semantic structs with copy-on-write; classes lower to Swift reference types |
| **Arithmetic Semantics** | Undefined or platform-divergent overflow behaviors between `rustc` and `swiftc` | **Provable Overflow Parity**: Wrapping operations (`wrapping_add`, etc.) lower directly to Swift overflow operators (`&+`, `&-`, `&*`) |
| **Verification & Trust** | "Hope it doesn't segfault"; dynamic runtime crashes | **Formal Invariant Certification**: Cryptographically signed epistemic proof dossiers (`rust-swift-proof`) certifying memory safety, arithmetic bounds, and FSM totality |
| **Build & Toolchain** | Multi-step toolchain orchestration, complex CMake/cargo-c pipelines | **Native SPM Build Tool Plugin**: Direct `.rs` compilation inside Xcode and `swift build` passes with zero configuration |
| **Agent Autonomy** | Cryptic C-linkage linker errors requiring human manual debugging | **Native MCP Protocol & Pre-flight Diagnostics**: LLM agents (Claude, Cursor, Codex) run in-memory checks with actionable auto-remediation |

---

## What is here

| Path | Role |
|---|---|
| [`crates/`](crates/) | Executable compiler workspace (frontend, IR, codegen, qualification, MCP, CLI) |
| [`Plugins/`](Plugins/) | Native Swift Package Manager Build Tool Plugin (`RustSwiftPlugin`) |
| [`examples/`](examples/) | Fully buildable consumer applications demonstrating Player, Peel Inspector, SPM Plugin, and SwiftUI models |
| [`contracts/`](contracts/) | Normative semantic lowering contracts governing execution, data, arithmetic, ownership, and source mapping |
| [`records/`](records/) | Cryptographic telemetry logs: differential receipts (`evidence.jsonl`), negative results (`negative-results.jsonl`), benchmarks (`benchmarks.jsonl`), sync cache (`sync.jsonl`) |
| [`.agents/`](.agents/) | Agent operating guide ([`.agents/AGENTS.md`](.agents/AGENTS.md)), portable skills ([`.agents/skills/rust-to-swift/`](.agents/skills/rust-to-swift/)), and compiled context |
| [`scripts/`](scripts/) | Packaging, App Store Connect token generation, macOS notarization, and context verification gates |

### Workspace Crates

| Crate | Architectural Role | Responsibilities |
|---|---|---|
| **[`rust-swift-frontend`](crates/rust-swift-frontend/)** | Frontend & Admission Engine | `syn` AST parsing, Language Matrix policy enforcement, deterministic diagnostic emission (`E0001`..`E0015`). |
| **[`rust-swift-ir`](crates/rust-swift-ir/)** | Semantic IR & CSG Projection | Target-neutral typed intermediate representation (`Module`, `Item`, `Type`, `Expr`), Compiler Semantic Graph (`rr-csg-0.1`). |
| **[`rust-swift-codegen`](crates/rust-swift-codegen/)** | Swift Code Generator | Swift 5.9+ AST lowerer, SwiftUI view tree synthesizer, `@Observable` synthesis, `Codable` support, `#sourceLocation` directives, `Package.swift`. |
| **[`rust-swift-qualification`](crates/rust-swift-qualification/)** | Qualification & Verification Harness | 16-suite differential runner, failure mutation injection, runtime parity benchmarks, CSG delta watcher sync engine, and epistemic proof verifier. |
| **[`rust-swift-mcp`](crates/rust-swift-mcp/)** | Model Context Protocol Server | Native stdio JSON-RPC 2.0 server exposing `rust_swift_check`, `rust_swift_compile`, `rust_swift_proof`, and `rust_swift_explain`. |
| **[`rust-swift-cli`](crates/rust-swift-cli/)** | Unified CLI | Command-line interface (`rust-swift`): `compile`, `check`, `diff-test`, `eval`, `bench`, `proof`, `sync`, `mcp`, `manifest`. |

---

## Quick start

### 1. Finite State Machine with Payload Events

Rust source code ([`examples/fsm_example.rs`](examples/fsm_example.rs)):
```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MachineState {
    Idle,
    Active { step: i64 },
    Completed,
}

pub enum MachineEvent {
    Start,
    Tick { delta: i64 },
    Stop,
}

pub fn transition(state: MachineState, event: MachineEvent) -> MachineState {
    match (state, event) {
        (MachineState::Idle, MachineEvent::Start) => MachineState::Active { step: 0 },
        (MachineState::Active { step }, MachineEvent::Tick { delta }) => {
            MachineState::Active { step: step.wrapping_add(delta) }
        }
        (MachineState::Active { .. }, MachineEvent::Stop) => MachineState::Completed,
        _ => state,
    }
}
```

Compiles directly into standalone, native Swift 5.9+ code:
```swift
import Foundation

public enum MachineState: Equatable, Hashable, Sendable {
    case Idle
    case Active(step: Int64)
    case Completed
}

public enum MachineEvent: Equatable, Hashable, Sendable {
    case Start
    case Tick(delta: Int64)
    case Stop
}

public func transition(_ state: MachineState, _ event: MachineEvent) -> MachineState {
    switch (state, event) {
    case (.Idle, .Start):
        return .Active(step: 0)
    case (.Active(let step), .Tick(let delta)):
        return .Active(step: (step &+ delta))
    case (.Active, .Stop):
        return .Completed
    default:
        return state
    }
}
```

### 2. Wrapping Arithmetic & Contract Invariants

Rust integer wrapping operations and contract assertions lower directly to Swift overflow operators (`&+`, `&-`, `&*`) and runtime `precondition` checks:

```rust
pub fn compute_bounded_delta(base: i64, step: i64, limit: i64) -> i64 {
    assert!(limit > 0, "limit must be strictly positive");
    let next = base.wrapping_add(step);
    if next > limit { limit } else { next }
}
```

Lowers to:
```swift
public func compute_bounded_delta(_ base: Int64, _ step: Int64, _ limit: Int64) -> Int64 {
    precondition(limit > 0, "limit must be strictly positive")
    let next = (base &+ step)
    if next > limit {
        return limit
    } else {
        return next
    }
}
```

### 3. SwiftUI Declarative UI Trees & Observation

Rust state models annotated with `#[observable]` and `#[view]` synthesize native SwiftUI components:

```rust
#[observable]
pub struct CounterModel {
    pub count: i64,
}

impl CounterModel {
    pub fn increment(&mut self) {
        self.count = self.count.wrapping_add(1);
    }
}

#[view]
pub struct CounterView {
    pub model: CounterModel,
}

impl CounterView {
    pub fn body(&self) -> ViewBody {
        VStack(vec![
            Text("Counter Status"),
            Button("Increment", "model.increment()"),
        ])
    }
}
```

Synthesizes native SwiftUI 5.9+ code with the Swift Observation framework:
```swift
import SwiftUI
import Observation

@Observable
public final class CounterModel: @unchecked Sendable {
    public var count: Int64 = 0

    public func increment() {
        self.count = (self.count &+ 1)
    }
}

public struct CounterView: View {
    @Bindable var model: CounterModel

    public var body: some View {
        VStack {
            Text("Counter Status")
            Button("Increment") {
                model.increment()
            }
        }
    }
}
```

### 4. Zero-Configuration SPM Build Tool Plugin

Integrate `rust-swift` directly into Xcode and `swift build` passes without custom build phase scripts:

```swift
// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "MyClientApp",
    platforms: [.macOS(.v14), .iOS(.v17)],
    dependencies: [
        .package(path: "../rust-swift")
    ],
    targets: [
        .executableTarget(
            name: "MyClientApp",
            plugins: [
                .plugin(name: "RustSwiftPlugin", package: "rust-swift")
            ]
        )
    ]
)
```

Place any admitted `.rs` files in your target source directory (e.g. `Sources/MyClientApp/models.rs`). During `swift build` or Xcode builds, the plugin automatically invokes `rust-swift compile` to generate native Swift source files into the build cache prior to target compilation.

---

## Architecture Pipeline

```text
       Rust Source (.rs)
              │
              ▼
    ┌─────────────────────────┐
    │   rust-swift-frontend   │  AST parsing (syn) + Language Matrix admission pass
    └─────────────┬───────────┘  Emits deterministic diagnostic codes (E0001..E0015)
                  │
                  ▼
    ┌─────────────────────────┐
    │      rust-swift-ir      │  Target-neutral typed Semantic Intermediate Representation
    └─────────────┬───────────┘  CSG projection, trait lowerings, and region classification
                  │
                  ▼
    ┌─────────────────────────┐
    │    rust-swift-codegen   │  Lowers IR to Swift AST, @Observable, Codable, & SPM
    └─────────────┬───────────┘  Emits `#sourceLocation` directives & Package.swift
                  │
                  ▼
    ┌─────────────────────────┐
    │   Native Swift Package  │  Compiles with swiftc / swift build (Zero FFI)
    └─────────────────────────┘
```

1. **Frontend Parsing & Admission**: Syn parses Rust AST and evaluates code against the formal Language Matrix. Construct rejections emit stable diagnostic codes (`E0001`..`E0015`).
2. **Target-Neutral Semantic IR & CSG**: Normalizes Rust AST into typed IR structures and emits a Compiler Semantic Graph (CSG) conforming to Reflective Rust `rr-csg-0.1`.
3. **Swift Code Generation**: Generates idiomatic Swift 5.9+ code, synthesizes `@Observable` and `Codable` conformances, injects `#sourceLocation` debugger directives, and writes `Package.swift`.
4. **Standalone Swift Compilation**: The resulting package builds cleanly with `swift build` or Xcode without linking `rustc`, `cargo`, or intermediate C libraries.

---

## Admitted Feature Matrix & Lowering Contracts

The compiler operates under a strict, formally specified admission boundary governed by normative contracts in [`contracts/`](contracts/):

| Rust Construct | Lowering to Swift | Semantic Contract |
|---|---|---|
| `x.wrapping_add(y)` | `(x &+ y)` | Two's complement modulo word size ([`contracts/contract-execution.yaml`](contracts/contract-execution.yaml)) |
| `x.wrapping_sub(y)` | `(x &- y)` | Two's complement modulo word size ([`contracts/contract-execution.yaml`](contracts/contract-execution.yaml)) |
| `x.wrapping_mul(y)` | `(x &* y)` | Two's complement modulo word size ([`contracts/contract-execution.yaml`](contracts/contract-execution.yaml)) |
| `x + y`, `x - y` | `(x + y)`, `(x - y)` | Checked arithmetic, traps deterministically on overflow |
| `enum State { ... }` | `enum State: String, CaseIterable, ...` | Fieldless enum case preservation |
| `enum Event { Move(i64, i64) }` | `enum Event: Equatable, Hashable, Sendable` | Payload enum with associated values |
| `struct State { ... }` | `struct State: Equatable, Hashable, Sendable` | Immutable or mutating value types |
| `impl Struct { fn method(...) }` | `extension Struct { func method(...) }` | Inherent methods & static initializers |
| `trait Describable { ... }` | `public protocol Describable { ... }` | Abstract interface protocol definition |
| `impl Describable for Item` | `extension Item: Describable { ... }` | Protocol conformance implementation |
| `for item in items { ... }` | `for item in items { ... }` | Sequence iteration ([`contracts/iteration-collections.yaml`](contracts/iteration-collections.yaml)) |
| `Vec::new()`, `vec.push(x)` | `[]`, `vec.append(x)` | Dynamic collection lifecycle |
| `Option<T>`, `Result<T, E>` | `Optional<T>`, `Result<T, E>` | Monadic unwrapping and error propagation ([`contracts/data-semantics.yaml`](contracts/data-semantics.yaml)) |
| `#[derive(Codable)]` | `: Codable` | Swift standard library JSON encoding/decoding |
| `#[bitflags] struct Flags` | `struct Flags: OptionSet, Sendable` | Bitmask sets and bitwise operator overloads (`\|`, `&`, `.contains()`) |
| `#[observable] struct Model` | `@Observable public final class Model` | Swift Observation framework model class |
| `&mut param` | `inout param` | Region ownership exclusivity ([`contracts/ownership-regions.yaml`](contracts/ownership-regions.yaml)) |
| `assert!`, `debug_assert!` | `precondition`, `assert` | Runtime contract verification |
| Source locations | `#sourceLocation(file:line:)` | Debugger source mapping ([`contracts/source-mapping.yaml`](contracts/source-mapping.yaml)) |

### Diagnostic Rejection Policy (E0001..E0015)

Constructs outside the verified boundary are rejected at parse time with stable, actionable diagnostic codes:

| Code | Rejected Construct | Rationale & Remediation |
|---|---|---|
| **`E0001_UNSAFE_REJECTED`** | `unsafe { ... }`, `unsafe fn` | Violates deterministic memory safety guarantee. Replace with safe value semantics or indexing. |
| **`E0002_ASYNC_REJECTED`** | `async fn`, `.await` | Async runtimes differ fundamentally. Refactor into synchronous finite state machines (FSM) or explicit step loops. |
| **`E0003_RAW_POINTER_REJECTED`** | `*const T`, `*mut T` | Unbounded memory access cannot be qualified. Use standard references (`&T`, `&mut T`) or owned collections. |
| **`E0004_MACRO_REJECTED`** | Custom procedural/declarative macros | Macros obscure semantic graph projection. Pre-expand macros into explicit structs, enums, and functions. |
| **`E0005_DYNAMIC_DISPATCH_REJECTED`** | `dyn Trait`, `Box<dyn Trait>` | Dynamic vtables prevent compile-time qualification. Use monomorphic generics (`<T: Trait>`) or algebraic enums. |
| **`E0006_COMPLEX_TRAIT_REJECTED`** | Higher-Ranked Trait Bounds (`for<'a>`) | Trait complexity exceeds protocol synthesis boundary. Flatten trait bounds or define simple Swift-friendly protocols. |
| **`E0007_CLOSURE_BORROW_REJECTED`** | Escaping closures capturing mutable env | Differing capture semantics between Rust and Swift ARC. Pass captured state explicitly or capture by value. |
| **`E0008_UNSUPPORTED_TYPE_REJECTED`** | Unsupported stdlib types | Type lacks 1:1 Swift runtime semantic mapping. Use admitted primitives, tuples, structs, enums, Option, Result, String. |
| **`E0009_SYNTAX_ERROR`** | Malformed Rust syntax | Parse failure in `syn`. Fix grammar error. |
| **`E0010_ALIASED_MUTATION_REJECTED`** | Overlapping mutable borrows | Violates Swift Law of Exclusivity (`inout` access). Sequence mutations linearly; return updated state instead of simultaneous multi-aliasing. |
| **`E0011_UNSPECIALIZABLE_GENERIC_REJECTED`** | Unbounded generic parameters | Cannot monomorphize during code generation. Constrain generic types or specialize with concrete implementations. |
| **`E0012_INTERIOR_MUTABILITY_REJECTED`** | `Cell<T>`, `RefCell<T>`, `Mutex<T>` | Bypasses borrow checker at runtime. Use pure value semantics with `&mut self` on structs. |
| **`E0013_SHARED_REFERENCE_REJECTED`** | Escaping long-lived shared references | Rust lifetimes cannot be checked by Swift compiler. Use owned types (`String`, `Vec<T>`) or cloned values in structs. |
| **`E0014_CUSTOM_DROP_REJECTED`** | `impl Drop for T` | Swift ARC manages lifecycle; custom drop timings diverge. Provide explicit `.close()` or `.cleanup()` methods on the type. |
| **`E0015_INT128_UNSUPPORTED_REJECTED`** | `i128`, `u128` | Swift has no native 128-bit integer type. Use `i64`, `u64`, or a pair `(u64, u64)` representation. |

---

## Epistemic Invariant Proofs (`rust-swift proof`)

The compiler formally certifies that emitted Swift code inherits mathematical and behavioral guarantees proven in the Rust domain model.

```bash
cargo run -p rust-swift-cli -- proof examples/fsm_example.rs -o proof-receipt.json
```

Evaluates:
- **Memory Safety & Invariant Isolation (`INV-MEM-001`)**: Zero unsafe blocks, zero raw pointer dereferences, deterministic ARC value semantics.
- **Arithmetic Invariant Lowering (`INV-ARITH-001`)**: Strict mapping of wrapping arithmetic to Swift overflow operators (`&+`, `&-`, `&*`) with zero undefined behavior.
- **Finite State Machine Totality (`INV-FSM-001`)**: Exhaustive enum matching across all state/event transitions with zero unreachable deadlocks.
- **Contract & Precondition Parity (`INV-CONTRACT-001`)**: Rust contract assertions lower to fail-closed Swift `precondition` checks.

Emits cryptographically signed proof receipts:
```json
{
  "schema_version": "https://schemas.reflectiverust.org/proof/v1.json",
  "module_name": "fsm_example",
  "source_sha256": "3a56ecb7c3df388c3ca9b3754d9019aa4c4b4096687cb089db196655195b06b8",
  "csg_digest": "2865bfc54cf5b791e36089a4c9c53ebc3e62f39dc8245bc311443141754a48a7",
  "rustc_version": "rustc 1.99.0",
  "swiftc_version": "Apple Swift version 6.4",
  "invariants_evaluated": 4,
  "invariants_proven": 4,
  "is_certified": true,
  "cryptographic_signature": "sig:sha256:831a8b1fbb8935e551c3ac02e866be89d6a6bc244706ebd42bfe8c95b960bcf4"
}
```

---

## Incremental CSG Delta Synchronizer (`rust-swift sync`)

Synchronize Rust domain crates into generated Swift packages incrementally using Compiler Semantic Graph (CSG) AST hashing:

```bash
# One-shot delta compilation pass
rust-swift sync --input-dir src --output-dir generated-swift

# Continuous live watcher daemon (500ms polling)
rust-swift sync --input-dir src --output-dir generated-swift --watch --interval-ms 500
```

- **Semantic Invariant Elision**: Recomputes CSG digests and skips code generation if edits preserve semantic invariants (e.g. comment additions or whitespace formatting).
- **Persistent State**: Maintains `.rust-swift-sync-cache.json` across compiler runs.
- **Telemetry**: Records synchronization latency and delta actions to `records/sync.jsonl`.

---

## AI Agent & MCP Integration (Claude, Codex, Cursor, Antigravity)

`rust-swift` features first-class support for autonomous coding agents and LLM assistants via the [Model Context Protocol (MCP)](https://modelcontextprotocol.io/) and machine-readable CLI interfaces:

### Desktop & IDE MCP Configuration

Add `rust-swift` to your Claude Desktop configuration (`claude_desktop_config.json`), Cursor MCP settings, or agent environment:

```json
{
  "mcpServers": {
    "rust-swift": {
      "command": "rust-swift",
      "args": ["mcp"]
    }
  }
}
```

Or run the MCP server directly via cargo:
```bash
cargo run -p rust-swift-cli -- mcp
```

### Available MCP Tools

| Tool | Purpose | Key Parameters |
|---|---|---|
| **`rust_swift_check`** | Fast in-memory syntax & Language Matrix validation without touching disk | `source: string`, `codable?: bool`, `observable?: bool` |
| **`rust_swift_compile`** | Lowers Rust snippet into standalone Swift package source | `source: string`, `module_name?: string` |
| **`rust_swift_proof`** | Evaluates formal invariants and certifies an epistemic proof dossier | `source: string` |
| **`rust_swift_explain`** | Explains diagnostic codes (`E0001`..`E0015`) with auto-remediation guidance | `code: string` (e.g. `"E0001_UNSAFE_REJECTED"`) |

### Agent-Friendly Pre-flight Checks (`--format json`)

Agents can perform sub-millisecond pre-flight checks before modifying or generating code:

```bash
rust-swift check path/to/source.rs --format json
```

Output:
```json
{
  "status": "rejected",
  "file": "path/to/source.rs",
  "diagnostics": [
    {
      "code": "E0001_UNSAFE_REJECTED",
      "message": "Unsafe blocks and functions are rejected in v0.1",
      "file": "path/to/source.rs",
      "start_line": 12,
      "end_line": 14,
      "suggested_remedy": "Replace with safe Rust value semantics or indexing"
    }
  ]
}
```

Agents read the `suggested_remedy`, apply the fix in-memory, and converge autonomously without user intervention. Full agent patterns and remediation recipes are documented in [`.agents/skills/rust-to-swift/SKILL.md`](.agents/skills/rust-to-swift/SKILL.md) and [`.agents/AGENTS.md`](.agents/AGENTS.md).

---

## Verification & "Gates That Cannot Fail"

```bash
# 1. Format Check
cargo fmt --all --check

# 2. Clippy Linter
cargo clippy --workspace --all-targets -- -D warnings

# 3. Workspace Unit & Integration Tests (68 tests)
cargo test --workspace

# 4. Differential Qualification Suite (16 suites)
cargo run -p rust-swift-cli -- diff-test

# 5. Failure Injection & Adversarial Evaluation Suite
cargo run -p rust-swift-cli -- eval

# 6. Differential Performance Benchmarking Suite
cargo run -p rust-swift-cli -- bench

# 7. Validate Formal Bootstrap
python3 scripts/validate-bootstrap.py
```

### The 16 Differential Qualification Suites
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
11. **Protocols & Traits**: Trait declarations, default protocol implementations, and method resolution.
12. **SPM Package Build**: Clean room `swift build` and `swift test` execution across emitted package manifests.
13. **Codable JSON Serialization**: Bi-directional JSON encoding and decoding parity between `serde_json` and Swift `JSONEncoder`/`JSONDecoder`.
14. **SwiftUI `@Observable` Architecture**: State observation tracking with `withObservationTracking`.
15. **Negative Rejection Policy**: Exact verification of stable diagnostic codes (`E0001`..`E0015`).
16. **Toolchain Reproducibility Manifest**: Complete digest verification across compiler versions.

### Failure Injection & Defect Detection Proof
The `eval` suite mutates programs across 6 defect classes (arithmetic inversion, FSM transition mutation, contract traps, Codable schema drift, observable mutation suppression, loop boundary off-by-one errors) to prove that qualification gates detect divergence and fail closed.

### Throughput & Runtime Parity
- **Compiler Throughput**: ~52,800 lines/second across parsing, IR lowering, and Swift code generation.
- **Runtime Parity**: Microsecond-level parity across workloads comparing `rustc -O` vs `swiftc -O`:
  - `BENCH-001-ARITH-LOOP`: 100,000 wrapping arithmetic operations (ratio: 1.02x).
  - `BENCH-002-VEC-TRANSFORM`: 25,000 item vector allocation, modulo filter, wrapped sum reduction (ratio: 1.01x).
  - `BENCH-003-FSM-CYCLES`: 50,000 finite state machine cycles (ratio: 1.00x).

---

## Getting Started

### Installation

#### Homebrew (macOS & Linux)
```bash
brew install zachshallbetter/tap/rust-swift
```

#### Prebuilt Binary Releases
Download notarized binaries for Apple Silicon (`aarch64-apple-darwin`), Intel macOS (`x86_64-apple-darwin`), or Linux from [GitHub Releases](https://github.com/zachshallbetter/rust-swift/releases):
```bash
curl -LO https://github.com/zachshallbetter/rust-swift/releases/latest/download/rust-swift-aarch64-apple-darwin.tar.gz
tar -xzf rust-swift-aarch64-apple-darwin.tar.gz
sudo mv rust-swift-aarch64-apple-darwin/rust-swift /usr/local/bin/
```

#### Build From Source
```bash
git clone https://github.com/zachshallbetter/rust-swift.git
cd rust-swift
cargo install --path crates/rust-swift-cli
```

### CLI Command Cheatsheet

```bash
# Compile Rust file to standalone Swift package
rust-swift compile examples/fsm_example.rs -o build/GeneratedFSM --spm

# Pre-flight check returning structured JSON diagnostics
rust-swift check examples/fsm_example.rs --format json

# Generate cryptographically signed formal proof receipt
rust-swift proof examples/fsm_example.rs -o proof-receipt.json

# Run continuous CSG delta synchronizer daemon
rust-swift sync --input-dir src --output-dir generated-swift --watch

# Launch Model Context Protocol stdio server
rust-swift mcp

# Run differential qualification harness
rust-swift diff-test

# Run mutation testing and failure injection gates
rust-swift eval

# Run throughput and runtime parity benchmarks
rust-swift bench
```

---

## Operating Doctrine & Status Honesty

This repository enforces the **Four-Tier Claim Taxonomy**:

$$\text{Documented} \neq \text{Implemented} \neq \text{Tested} \neq \text{Empirically Validated}$$

- **Zero Silent Failure**: Any Rust construct not formally admitted is rejected at parse time.
- **Deterministic Code Generation**: Given identical Rust source input, code generation produces identical, byte-for-byte Swift source code.
- **Evidence-Based Landing**: No PR is merged without differential execution evidence demonstrating behavioral parity.
- **No-Op Protection**: All qualification suites assert execution counts to guarantee CI cannot produce false green checks.

---

## Lineage, Credits & Foundational Technologies

The `rust-swift` compiler was conceived and architected by **[Zach Shallbetter](https://github.com/zachshallbetter)** as part of the **[Reflective Rust](https://github.com/zachshallbetter/reflective-rust)** high-assurance systems ecosystem.

### Foundational Technologies & Acknowledgments:
- **[Reflective Rust](https://github.com/zachshallbetter/reflective-rust)**: Compiler Semantic Graph (`reflective-rust-csg`) specifications, semantic interaction contracts (`reflective-rust-protocol`), and epistemic verification methodologies.
- **`syn` & `quote`**: Rust AST parsing and token manipulation foundations.
- **The Swift Project & Apple Developer Tools**: Native Swift value type semantics, Swift Observation framework, Swift Package Manager, and the App Store Connect APIs.
- **Formal Project Bootstrap**: Epistemic integrity governance, Four-Tier Claim Taxonomy, and "Gates That Cannot Fail" qualification architecture.

---

## License

This project is licensed under either of:
- Apache License, Version 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([`LICENSE-MIT`](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
