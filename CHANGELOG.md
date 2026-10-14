# Changelog

All notable changes to the Rust to Swift Compiler will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **SPM Build Tool Plugin (`rust-swift-spm-plugin`, `WI-SPM-PLUGIN-001`)**:
  - Root `Package.swift` declaring `.buildTool()` plugin capability for Swift Package Manager and Xcode.
  - Native build tool plugin implementation in `Plugins/RustSwiftPlugin/plugin.swift` automatically scanning target source directories for `.rs` files and compiling them to native Swift source files ahead of target compilation.
  - Consumer integration fixture in `examples/PluginDemoApp`.
  - Differential qualification suite verifying clean-room `swift build` execution of plugin-enabled target packages.
- **Declarative SwiftUI View Tree Synthesis (`rust-swiftui`, `WI-SWIFTUI-001`)**:
  - Semantic IR additions (`ViewDef`, `ViewNode`, `ViewKind`, `ViewModifier`, `PropertyBindingKind`) projected to Compiler Semantic Graph (CSG) nodes and edges.
  - Syntax lowering in `rust-swift-frontend` admitting `#[view]` annotated structs, view hierarchies (`VStack`, `HStack`, `ZStack`, `Text`, `Button`, `TextField`, `List`, `Spacer`, `Divider`), and modifiers (`padding`, `font`, `foregroundColor`, `background`, `frame`).
  - Swift code generator synthesis into idiomatic `struct Name: View { var body: some View { ... } }` with `@Bindable`, `@Binding`, and `@Observable` state binding.
  - Differential qualification suite compiling generated SwiftUI views with `swiftc -parse-as-library` and validating observation tracking.
- **Formal Invariant Verification & Proof Dossier Generator (`rust-swift-proof`, `WI-PROOF-001`)**:
  - `ProofVerifier` assessing memory safety, arithmetic overflow (&+, &-, &*), FSM totality, and contract runtime invariants.
  - Cryptographically signed `EpistemicProofReceipt` generation (`proof-receipt.json`) certifying that emitted Swift artifacts inherit formal guarantees proven in Rust domain models.
  - CLI `rust-swift proof <input.rs> [-o receipt.json]` subcommand with SHA-256 integrity proofs.
  - Differential qualification suite certifying formal verification dossiers.
- **Incremental CSG Delta Synchronizer & Watcher Daemon (`rust-swift-sync`, `WI-SYNC-001`)**:
  - `SyncEngine` in `crates/rust-swift-qualification/src/sync.rs` providing bidirectional delta synchronization and persistent `.rust-swift-sync-cache.json` cache management across compilation sessions.
  - Compiler Semantic Graph (CSG) AST hash tracking to elide redundant Swift code generation when source edits preserve semantic invariants.
  - CLI `rust-swift sync [--input-dir <dir>] [--output-dir <dir>] [--watch] [--interval-ms <ms>]` subcommand supporting single-pass delta execution and continuous daemon file watching.
  - Differential qualification suite and telemetry records appended to `records/sync.jsonl`.
- **Protocols & Traits (`WI-TRAIT-001`)**:
  - Rust trait definitions (`pub trait Foo { ... }`) lowering into native Swift `public protocol Foo { ... }`.
  - Default trait method implementations lowered to protocol extension defaults.
  - Conformance implementation blocks (`impl Foo for Bar { ... }`) lowered to `extension Bar: Foo { ... }`.
  - Differential qualification suite verifying protocol dynamic dispatch and method overrides between `rustc` and `swiftc`.
- **Swift Package Manager Manifest Emission (`WI-SPM-001`)**:
  - Generation of buildable Swift 5.9+ `Package.swift` manifests via `SwiftGenerator::emit_package` and CLI `--spm`.
  - Clean-room package layout generation including library products, sources directory, and XCTest suite test targets.
  - Verified with clean-room `swift build` and `swift test` qualification runs.
- **Codable JSON Serialization (`WI-CODABLE-001`)**:
  - Lowering of `#[derive(Codable)]` on structs and payload enums into Swift `: Codable` protocol conformance.
  - JSON serialization differential qualification suite validating bi-directional parity between `serde_json` and Swift `JSONEncoder`/`JSONDecoder`.
- **SwiftUI `@Observable` Architecture (`WI-OBSERVABLE-001`)**:
  - Lowering of `#[observable]` annotations to Swift `@Observable public final class`.
  - Observation tracking differential suite verifying that state property mutations trigger `withObservationTracking` onChange handlers.
- **App Store Connect & Notarization Pipeline (`WI-NOTARIZE-001`)**:
  - Automated ES256 JWT authentication token generator (`scripts/asc-token.py`) for App Store Connect API.
  - Production packaging, code signing, sandboxing entitlements, and notarization script (`scripts/apple-notarize.sh`).
- **Failure Injection & Mutation Testing Framework (`WI-EVAL-MUT-001`)**:
  - `MutationRunner` covering 6 synthetic defect mutation classes (`MUT-001` through `MUT-006`) enforcing the "Gates That Cannot Fail" doctrine:
    * `MUT-001-ARITH-INVERSION` (Wrapping arithmetic operator inversion)
    * `MUT-002-FSM-TRANSITION` (FSM state machine transition defect)
    * `MUT-003-CONTRACT-TRAP` (Contract precondition invariant violation)
    * `MUT-004-CODABLE-DRIFT` (Codable schema drift / payload corruption)
    * `MUT-005-OBSERVABLE-SUPPRESSION` (SwiftUI observable state mutation suppression)
    * `MUT-006-BOUNDARY-OFF-BY-ONE` (Loop iteration boundary defect)
  - Permanent retention of failure receipts in `records/negative-results.jsonl`.
  - CLI `eval --mutations` subcommand and CI qualification step.
- **Differential Performance Benchmarking Suite (`WI-PERF-BENCH-001`)**:
  - `BenchmarkRunner` measuring compiler throughput (lines/sec, KB/sec) and runtime parity between `rustc -O` and `swiftc -O` across:
    * `BENCH-001-ARITH-LOOP` (100,000 wrapping arithmetic operations)
    * `BENCH-002-VEC-TRANSFORM` (25,000 item vector allocation, modulo filter, sum reduction)
    * `BENCH-003-FSM-CYCLES` (50,000 FSM cycles / 300,000 transitions)
  - Output validation asserting bit-for-bit checksum parity.
  - Telemetry records written to `records/benchmarks.jsonl`.
  - CLI `bench` subcommand and CI qualification step.
- **Aggressive Adversarial Evals & Assertion Telemetry (`WI-EVAL-ADV-001`)**:
  - Multi-domain stress testing: 32-level deep recursive expression nesting, extreme Unicode (emoji sequences, ZWJ clusters, multi-script UTF-8), and integer boundary overflow transitions.
  - No-Op Protection enforcing minimum executed assertion thresholds (24 executed, threshold $\ge 20$) to prevent false greens in CI.
  - Structured qualification records appended to `records/evidence.jsonl`.
  - CLI `eval --adversarial` subcommand.
- **Event Dispatch & Trace Model (`WI-EVENT-001`)**:
  - Support for `Type::Array` and dynamic vectors `Vec<T>`, slices `[T]`, and fixed-size arrays `[T; N]`.
  - Admitted array literals `[e1, e2]` and the `vec![...]` macro in expression positions.
  - Swift codegen lowering of array expressions and types to native Swift arrays `[T]`.
  - Method translation for `push` $\to$ `append`, `len` $\to$ `count`, and `is_empty` $\to$ `isEmpty`.
  - Formal event trace contract specification in `contracts/event-trace-protocol.yaml`.
  - Differential qualification suite verifying 50-step multi-transition state machine execution history with effect emission, asserting line-by-line parity and bit-exact SHA-256 trace digest equivalence between `rustc` and `swiftc`.
- **Type System Expansion (`WI-TYPE-001`)**:
  - Struct inherent implementation blocks (`impl Type { ... }`) lowering into Swift `extension Type`.
  - Receiver classification: `&mut self` lowered to `mutating func`, `&self` and value receivers lowered to standard `func`, and static methods lowered to `static func`.
  - Associated value / payload enums (`enum Value { Stop, Move(i64, i64), Teleport { x: i64, y: i64 } }`) lowering to Swift enums with associated values (`case Stop`, `case Move(Int64, Int64)`, `case Teleport(x: Int64, y: Int64)`).
  - Advanced pattern matching with value binding extraction (`case .Move(let dx, let dy):`, `case .Teleport(x: let x, y: let y):`) and tuple matching.
  - Native tuple types, tuple literals, and tuple field indexing (`.0`, `.1`).
  - Strict rejection of complex traits (`impl Trait for Type`, `E0006`) and dynamic dispatch (`&dyn Trait`, `E0005`).
  - Differential qualification suite verifying mutating struct methods, payload enums, and tuple workflows between `rustc` and `swiftc`.
- **Contract System & Runtime Invariants (`WI-CONTRACT-001`)**:
  - Support for `assert!`, `debug_assert!`, `assert_eq!`, and `panic!` in `rust-swift-frontend`.
  - Semantic IR representation with `Stmt::Assert`, `Stmt::DebugAssert`, `Stmt::AssertEq`, `Stmt::Panic`, and `ExprKind::Panic`.
  - Swift codegen lowering:
    - `assert!(cond, msg)` lowered to Swift `precondition(cond, msg)`.
    - `debug_assert!(cond, msg)` lowered to Swift `assert(cond, msg)`.
    - `assert_eq!(a, b, msg)` lowered to Swift `precondition(a == b, msg)`.
    - `panic!(msg)` lowered to Swift `fatalError(msg)`.
  - Formal contract execution specification in `contracts/contract-execution.yaml`.
  - Differential qualification suite verifying positive assertions and identical process trap behavior on invariant failure across `rustc` and `swiftc` binaries.

### Fixed
- **String Literal Control Character Escaping in Swift Codegen**:
  - Properly escapes raw newlines (`\n`), carriage returns (`\r`), tabs (`\t`), null bytes (`\0`), backslashes (`\\`), and double quotes (`\"`) when emitting Swift string literals.

## [0.1.0] - 2026-10-13

### Added
- **Language Frontend (`rust-swift-frontend`)**:
  - Full syntactic parser based on `syn` with source span preservation.
  - Language Matrix policy rejection mechanism emitting stable diagnostic error codes (`E0001` through `E0009`) for unsafe blocks, async functions, raw pointers, unexpanded macros, dynamic dispatch, and complex traits.
- **Target-Neutral Semantic IR (`rust-swift-ir`)**:
  - Types (`i8`..`i128`, `u8`..`u128`, `bool`, tuples, structs, enums).
  - Explicit integer arithmetic modes (`Checked`, `Wrapping`, `Saturating`).
  - Compiler Semantic Graph (CSG) projection engine conforming to `reflective-rust-csg` schema (`rr-csg-0.1`) with compiler-certified provenance.
- **Swift Code Generator (`rust-swift-codegen`)**:
  - Standalone Swift package layout emitter (`Package.swift`, `Sources/`, `manifest.json`).
  - Native lowering of Rust `wrapping_add`, `wrapping_sub`, and `wrapping_mul` to Swift overflow operators (`&+`, `&-`, `&*`).
  - Value type lowering for structs and fieldless enums.
  - Provenance symbol maps linking generated Swift tokens to Rust source locations.
- **Differential Qualification Suite (`rust-swift-qualification`)**:
  - Exhaustive 16-case finite state machine differential test runner comparing Rust reference against compiled Swift binary.
  - 36-vector fixed-width wrapping arithmetic boundary suite.
  - Negative diagnostic assertion tests.
  - Verifiable evidence records automatically written to `records/evidence.jsonl`.
- **Compiler CLI (`rust-swift-cli`)**:
  - `rust-swift compile <input.rs> -o <pkg> [--emit-csg] [--emit-ir]`
  - `rust-swift diff-test [--output-dir <dir>]`
  - `rust-swift manifest`
- **Continuous Integration (`.github/workflows/ci.yml`)**:
  - Automated CI on macOS executing `cargo fmt`, `cargo clippy`, `cargo test`, `rust-swift diff-test`, and formal project bootstrap integrity checks.
