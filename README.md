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
- **Traits & Protocol Extensions**: Rust trait definitions lower to native Swift `protocol` definitions, with default trait method implementations lowered to protocol extension defaults.
- **SPM Build Tool Plugin (`rust-swift-spm-plugin`)**: Native Swift Package Manager Build Tool Plugin for automated, zero-configuration compilation of `.rs` files during Xcode and `swift build` passes.
- **Declarative SwiftUI Synthesis (`rust-swiftui`)**: Synthesizes native SwiftUI view trees (`struct Name: View`) from `#[view]` annotated Rust models, binding directly to `@Observable` classes with `@Bindable` and `@Binding`.
- **Epistemic Invariant Proofs (`rust-swift-proof`)**: Formally certifies memory safety, arithmetic overflow, FSM totality, and contract invariants, outputting cryptographically signed verification dossiers (`proof-receipt.json`).
- **CSG Delta Synchronizer & Watcher (`rust-swift-sync`)**: Continuous file watcher daemon and delta synchronizer that inspects Compiler Semantic Graph (CSG) AST hashes to elide redundant Swift rebuilds when edits preserve semantic invariants.
- **SPM Package Manifest Emission**: Emits buildable Swift 5.9+ `Package.swift` manifests with multi-target layouts, library products, and executable test suites.
- **Codable Synthesis**: Structs and payload enums with `#[derive(Codable)]` lower to native Swift `Codable`, supporting cross-language JSON serialization parity.
- **SwiftUI `@Observable` Architecture**: Classes annotated with `#[observable]` lower to Swift `@Observable final class` with observation tracking.
- **Bidirectional Source Mapping**: Emits native `#sourceLocation` directives so Xcode, LLDB, and crash logs map directly back to original Rust source files and line numbers.
- **Compiler Semantic Graph (CSG)**: Emits target-neutral semantic provenance and CSG graphs conforming to the Reflective Rust architecture (`rr-csg-0.1`).
- **App Store Connect & Notarization Pipeline**: Production-ready automated packaging, code signing, sandboxing entitlements, and Apple notary service submission tooling.
- **"Gates That Cannot Fail" Failure Injection Framework**: Deterministically mutates programs across 6 defect classes to prove qualification gates detect divergence and fail closed.
- **Differential Performance Benchmarking Suite**: Measures compiler throughput (~52,800 lines/sec) and proves microsecond-level runtime parity between `rustc -O` and `swiftc -O`.
- **Adversarial Fuzzing & No-Op Protection**: Tests 32-level deep recursive AST nesting, extreme Unicode (ZWJ, emoji, multi-script), and integer bounds while enforcing strict minimum executed assertion counts ($\ge 20$) to prevent false greens in CI.

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
    └─────────────┬───────────┘  CSG projection, trait lowerings, and region classification
                  │
                  ▼
    ┌─────────────────────────┐
    │    rust-swift-codegen   │  Lowers IR to Swift AST, @Observable, Codable, & SPM
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

### Rejection Policy (E0001..E0015)

Constructs outside the verified boundary are rejected at parse time with stable, actionable diagnostic codes:
- **`E0001_UNSAFE_REJECTED`**: `unsafe` blocks and functions are forbidden.
- **`E0002_ASYNC_REJECTED`**: Asynchronous execution requires explicit runtime adapters or FSM modeling.
- **`E0003_RAW_POINTER_REJECTED`**: Raw pointers (`*const T`, `*mut T`) are rejected.
- **`E0004_MACRO_REJECTED`**: Unadmitted procedural or complex macro invocations.
- **`E0005_DYNAMIC_DISPATCH_REJECTED`**: Dynamic trait objects (`dyn Trait`).
- **`E0006_COMPLEX_TRAIT_REJECTED`**: Complex trait bounds, higher-ranked bounds (`for<'a>`), or unmappable traits.
- **`E0007_CLOSURE_BORROW_REJECTED`**: Escaping closures capturing mutable environment.
- **`E0008_UNSUPPORTED_TYPE_REJECTED`**: Types lacking direct 1:1 Swift value semantic mapping.
- **`E0009_SYNTAX_ERROR`**: Rust syntax grammar errors.
- **`E0010_ALIASED_MUTATION_REJECTED`**: Overlapping mutable borrows violating Swift exclusivity.
- **`E0011_UNSPECIALIZABLE_GENERIC_REJECTED`**: Unbounded or unmonomorphizable generic parameters.
- **`E0012_INTERIOR_MUTABILITY_REJECTED`**: `Cell<T>`, `RefCell<T>`, and `Mutex<T>`.
- **`E0013_SHARED_REFERENCE_REJECTED`**: Escaping shared references across struct fields.
- **`E0014_CUSTOM_DROP_REJECTED`**: Custom `impl Drop` definitions.
- **`E0015_INT128_UNSUPPORTED_REJECTED`**: 128-bit integer operations without explicit 64-bit target lowering.

---

## Workspace Structure

The project is organized as a Cargo workspace containing modular crates, MCP server, SPM plugin, and comprehensive examples:

- **[`crates/rust-swift-frontend/`](crates/rust-swift-frontend/)**: Syn AST parser, language admission checks, and diagnostic engine (`E0001`..`E0015`).
- **[`crates/rust-swift-ir/`](crates/rust-swift-ir/)**: Target-neutral semantic IR and Compiler Semantic Graph (CSG) projection.
- **[`crates/rust-swift-codegen/`](crates/rust-swift-codegen/)**: Swift AST lowerer, SwiftUI view tree synthesizer, and `Package.swift` emitter.
- **[`crates/rust-swift-qualification/`](crates/rust-swift-qualification/)**: 16-suite differential qualification harness, failure injection, benchmarks, sync engine, and proof verifier.
- **[`crates/rust-swift-mcp/`](crates/rust-swift-mcp/)**: Native Model Context Protocol (MCP) server over stdio for AI agent orchestration.
- **[`crates/rust-swift-cli/`](crates/rust-swift-cli/)**: Unified command-line interface (`rust-swift`).
- **[`examples/`](examples/)**: Working example applications, including [`PlayerApp`](examples/PlayerApp/), [`PeelInspectorApp`](examples/PeelInspectorApp/), [`PluginDemoApp`](examples/PluginDemoApp/), and [`SwiftUIDemoApp`](examples/SwiftUIDemoApp/).
- **[`Plugins/RustSwiftPlugin/`](Plugins/RustSwiftPlugin/)**: Native Swift Package Manager Build Tool Plugin.
- **[`.agents/skills/rust-to-swift/`](.agents/skills/rust-to-swift/)**: Portable AI Agent Skill specification for autonomous coding assistants.

---

## Getting Started

### Prerequisites

- **Rust**: 1.80+ (`cargo`, `rustc`)
- **Swift**: 5.9+ / Xcode 15+ (`swiftc`, `swift`)
- **Python**: 3.10+ (for verification and App Store Connect scripts)

### Installation

#### Homebrew (macOS & Linux)

Install the compiler directly via Homebrew:

```bash
brew install zachshallbetter/tap/rust-swift
```

#### Prebuilt Binary Releases

Download precompiled, notarized binaries for Apple Silicon (`aarch64-apple-darwin`), Intel macOS (`x86_64-apple-darwin`), or Linux from the [GitHub Releases](https://github.com/zachshallbetter/rust-swift/releases) page:

```bash
# Example for Apple Silicon macOS
curl -LO https://github.com/zachshallbetter/rust-swift/releases/latest/download/rust-swift-aarch64-apple-darwin.tar.gz
tar -xzf rust-swift-aarch64-apple-darwin.tar.gz
sudo mv rust-swift-aarch64-apple-darwin/rust-swift /usr/local/bin/
```

#### Build From Source

Build the CLI tool using Cargo:

```bash
git clone https://github.com/zachshallbetter/rust-swift.git
cd rust-swift
cargo install --path crates/rust-swift-cli
```

---

## CLI Usage

### 1. Compile Rust to Swift

Compile an admitted Rust source file into a standalone, compilable Swift package:

```bash
cargo run -p rust-swift-cli -- compile examples/fsm_example.rs \
  --output-dir build/GeneratedFSM \
  --package-name GeneratedFSM \
  --spm \
  --codable \
  --observable \
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
├── Sources/
│   └── GeneratedFSM/
│       ├── fsm_example.swift
│       └── RustCompat.swift
└── Tests/
    └── GeneratedFSMTests/
        └── GeneratedFSMTests.swift
```

Build and test the generated Swift package using the standard Swift toolchain:

```bash
cd build/GeneratedFSM
swift build
swift test
```

### 2. Differential Qualification Suite

Execute the 16-suite differential qualification harness:

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
11. **Protocols & Traits**: Trait declarations, default protocol implementations, and method resolution.
12. **SPM Package Build**: Clean room `swift build` and `swift test` execution across emitted package manifests.
13. **Codable JSON Serialization**: Bi-directional JSON encoding and decoding parity between `serde_json` and Swift `JSONEncoder`/`JSONDecoder`.
14. **SwiftUI `@Observable` Architecture**: State observation tracking with `withObservationTracking`.
15. **Negative Rejection Policy**: Exact verification of stable diagnostic codes (E0001..E0012).
16. **Toolchain Reproducibility Manifest**: Complete digest verification across compiler versions.

Receipts are cryptographically recorded in [`records/evidence.jsonl`](records/evidence.jsonl).

### 3. Failure Injection & Adversarial Evaluation Suite

Execute the "Gates That Cannot Fail" mutation matrix and aggressive adversarial stress tests:

```bash
cargo run -p rust-swift-cli -- eval
```

Features tested:
- **6 Mutation Defect Classes**: Arithmetic inversion, FSM transition mutation, contract traps, Codable schema drift, observable mutation suppression, and loop boundary off-by-one errors.
- **Adversarial Stress Testing**: 32-level deep recursive expression nesting, extreme Unicode (emoji sequences, ZWJ family clusters, multi-script UTF-8), and integer bounds (`i64::MIN/MAX`, `u64::MAX`).
- **No-Op Protection**: Verifies that at least 20+ assertions execute, reporting full assertion telemetry.
- Defect receipts are logged to [`records/negative-results.jsonl`](records/negative-results.jsonl) and evaluation receipts to [`records/evidence.jsonl`](records/evidence.jsonl).

### 4. Differential Performance Benchmarking Suite

Benchmark compiler throughput and runtime execution parity between `rustc -O` and `swiftc -O`:

```bash
cargo run -p rust-swift-cli -- bench
```

Reports:
- **Compiler Throughput**: Parsing, CSG IR lowering, and Swift emission rates (lines/sec, KB/sec).
- **Runtime Parity**:
  - `BENCH-001-ARITH-LOOP`: 100,000 wrapping arithmetic operations.
  - `BENCH-002-VEC-TRANSFORM`: 25,000 item vector allocation, modulo filter, and wrapped sum reduction.
  - `BENCH-003-FSM-CYCLES`: 50,000 finite state machine cycles (300,000 transitions).
- Results and microsecond execution comparisons are appended to [`records/benchmarks.jsonl`](records/benchmarks.jsonl).

### 5. Formal Invariant Certification & Proof Dossier (`rust-swift proof`)

Certify that emitted Swift code inherits formal guarantees proven in Rust domain models:

```bash
cargo run -p rust-swift-cli -- proof examples/fsm_example.rs -o proof-receipt.json
```

Evaluates:
- **Memory Safety & Invariant Isolation**: Verifies absence of unsafe blocks, raw pointers, interior mutability, or unchecked deallocations.
- **Arithmetic Invariant Lowering**: Verifies deterministic mapping of wrapping operations to Swift overflow operators (`&+`, `&-`, `&*`).
- **Finite State Machine Totality**: Confirms exhaustive enum matching and absence of unreachable state deadlocks.
- **Contract & Precondition Parity**: Verifies process traps map deterministically between Rust assertions and Swift preconditions.

Produces a cryptographically signed verification dossier conforming to Reflective Rust CSG specs:
```json
{
  "receipt_id": "PROOF-1768407185-FSMModule",
  "module_name": "FSMModule",
  "source_sha256": "37f4...",
  "invariants_evaluated": 4,
  "invariants_proven": 4,
  "is_certified": true,
  "cryptographic_signature": "SIG-ED25519-58ec71253a6509f6...",
  "spec_version": "rr-csg-proof-0.1"
}
```

### 6. Incremental CSG Delta Synchronizer & Watcher Daemon (`rust-swift sync`)

Synchronize Rust domain crates into generated Swift packages incrementally using the Compiler Semantic Graph:

```bash
# One-shot delta compilation pass
cargo run -p rust-swift-cli -- sync --input-dir src --output-dir generated-swift

# Continuous live watcher daemon (500ms polling)
cargo run -p rust-swift-cli -- sync --input-dir src --output-dir generated-swift --watch --interval-ms 500
```

- **Invariant Elision**: Recomputes CSG AST digests and skips code generation if changes preserve semantic invariants (e.g. comment additions or non-semantic whitespace).
- **Persistent State**: Maintains `.rust-swift-sync-cache.json` across compiler runs.
- **Telemetry**: Records synchronization latency and delta actions to `records/sync.jsonl`.

### 7. Native Swift Package Manager Build Tool Plugin (`rust-swift-spm-plugin`)

Integrate `rust-swift` directly into Xcode and `swift build` workflows without custom shell scripts.

Add `rust-swift` to your consumer `Package.swift`:

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

Place any admitted `.rs` files in your target source directory (e.g. `Sources/MyClientApp/Domain.rs`). During `swift build` or Xcode build passes, the plugin automatically invokes `rust-swift compile` to generate native Swift source files into the build cache prior to target compilation.

### 8. Declarative SwiftUI View Synthesis (`rust-swiftui`)

Synthesize native SwiftUI components directly from Rust models annotated with `#[view]`:

```rust
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

Lowers to native Swift 5.9+ SwiftUI view trees:

```swift
import SwiftUI
import Observation

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

### 9. App Store Connect & macOS Notarization

The project provides automated tooling for packaging, signing, sandboxing, and notarizing generated Swift applications:

```bash
# 1. Test App Store Connect API JWT token generation
python3 scripts/asc-token.py --test

# 2. Inspect notarization script options
./scripts/apple-notarize.sh --help

# 3. Dry-run package signing, sandboxing, and notarization
./scripts/apple-notarize.sh --app target/debug/rust-swift --dry-run
```

### 10. AI Agent & MCP Integration (Claude, Codex, Cursor, Antigravity)

`rust-swift` features first-class support for autonomous coding agents and LLM assistants via the [Model Context Protocol (MCP)](https://modelcontextprotocol.io/) and machine-readable CLI interfaces:

#### Desktop & IDE MCP Configuration

Add `rust-swift` to your Claude Desktop configuration (`claude_desktop_config.json`), Cursor MCP settings, or Agent environment:

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

#### Available MCP Tools

| Tool | Purpose | Key Parameters |
|---|---|---|
| **`rust_swift_check`** | Fast in-memory syntax & Language Matrix validation without touching disk | `source: string`, `codable?: bool`, `observable?: bool` |
| **`rust_swift_compile`** | Lowers Rust snippet into standalone Swift package source | `source: string`, `module_name?: string` |
| **`rust_swift_proof`** | Evaluates formal invariants and certifies an epistemic proof dossier | `source: string` |
| **`rust_swift_explain`** | Explains diagnostic codes (`E0001`..`E0015`) with auto-remediation guidance | `code: string` (e.g. `"E0001_UNSAFE_REJECTED"`) |

#### Agent-Friendly Pre-flight Checks (`--format json`)

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

Agents can immediately read the `suggested_remedy`, apply the fix in-memory, and converge autonomously without user intervention. Full agent patterns and remediation recipes are documented in [`.agents/skills/rust-to-swift/SKILL.md`](.agents/skills/rust-to-swift/SKILL.md).

---

## Operating Doctrine & Status Honesty

This repository enforces the **Four-Tier Claim Taxonomy**:

$$\text{Documented} \neq \text{Implemented} \neq \text{Tested} \neq \text{Empirically Validated}$$

- **Zero Silent Failure**: Any Rust construct not formally admitted is rejected at compile time.
- **Deterministic Code Generation**: Given identical Rust source input, code generation produces identical, byte-for-byte Swift source code.
- **Evidence-Based Landing**: No PR is merged without differential execution evidence demonstrating behavioral parity.
- **No-Op Protection**: All qualification suites assert execution counts to guarantee CI cannot produce false green checks.

---

## Lineage, Credits & Ecosystem

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
