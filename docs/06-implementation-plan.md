Rust → Swift Compiler | Milestone Status & Implementation Plan | 14 October 2026

## Milestone Status

| Milestone | Scope | Status | Evidence / Verification |
|---|---|---|---|
| **M0 - Feasibility** | Toolchain pinning, syn AST extraction, diagnostic engine | **COMPLETE** | Diagnostics test suite (`E0001`..`E0012`), reproducibility manifest |
| **M1 - Vertical Slice** | Finite state machines, wrapping arithmetic, standalone Swift packages | **COMPLETE** | 16-case FSM differential, 36-vector arithmetic vectors, zero-FFI emission |
| **M2 - Structural Language** | Structs, payload enums, pattern matching, inherent methods | **COMPLETE** | Mutating struct qualification, associated values pattern matching |
| **M3 - Data Semantics** | `Option`, `Result`, dynamic `Vec<T>`, UTF-8 string slicing, Codable | **COMPLETE** | Data semantics differential, JSON serialization parity (`serde_json` vs `JSONEncoder`) |
| **M4 - Ownership & Regions** | Borrow exclusivity (`&mut` $\to$ `inout`), drop checks, rejection gates | **COMPLETE** | Ownership regions differential, negative rejection matrix |
| **M5 - Protocols & SPM** | Traits to protocols, default method extensions, SPM `Package.swift` | **COMPLETE** | Protocol qualification, clean-room `swift build` and `swift test` runs |
| **M6 - Hardening & Evals** | Failure injection ("Gates That Cannot Fail"), performance benchmarks, adversarial stress | **COMPLETE** | 6/6 defect mutations caught, 24/24 adversarial assertions, `benchmarks.jsonl` |
| **M7 - Distribution & Apps** | SwiftUI `@Observable`, App Store Connect API, automated notarization | **COMPLETE** | `PlayerApp`, `PeelCoreKit`, `asc-token.py`, `apple-notarize.sh` |
| **M8 - Companion Systems** | SPM Build Tool Plugin, declarative SwiftUI view synthesis, proof dossiers, CSG delta synchronizer | **COMPLETE** | `RustSwiftPlugin` fixture, SwiftUI differential, `proof-receipt.json`, CSG delta skipping |

## Next Phase Horizons & Phased Roadmap

Following the completion of Milestones M0 through M8, the forward compiler roadmap proceeds across five sequential phases (detailed in [`09-apple-ecosystem-capabilities-and-kits.md`](09-apple-ecosystem-capabilities-and-kits.md)):

### Phase 1: Trait Associated Types & Type Projections (`v0.3.0`)
- **Associated Types**: Elaborate multi-parameter trait associated types (`type Item;`) and where-clause projections into native Swift `associatedtype` declarations.
- **Protocol Conformance**: Generate explicit `typealias` bindings in conforming structs and classes.
- **Verification**: Extend protocol qualification suite with differential conformance checks.

### Phase 2: Structured Concurrency & Event Loops (`v0.3.5`)
- **Admitted Async Grammar**: Relax `E0002_ASYNC_REJECTED` for admitted, cooperative asynchronous functions.
- **Native Swift Concurrency**: Lower Rust futures directly into Swift `async/await`, structured `TaskGroup`, and native actor isolation without intermediate C-ABI wrappers.
- **Verification**: Differential async execution tests comparing Rust poll state machines with Swift concurrency tasks.

### Phase 3: Apple Framework ("Kit") Adapters & Synthesizers (`v0.4.0`)
- **AppKit & Document Models**: Attribute-driven synthesis of `FileDocument` / `ReferenceFileDocument` for macOS multi-window document apps.
- **FSKit & FileProvider**: Turnkey delegate adapters (`FSVolumeOperations`) allowing safe Rust file system engines to run zero-FFI inside macOS 15+ FSKit user-space daemons and FileProvider extensions.
- **RealityKit & WidgetKit**: Automatic `RealityKit.Component` protocol conformance and `WidgetKit.TimelineProvider` scheduling.

### Phase 4: Security, Entitlements & Packaging Verification (`v0.5.0`)
- **Automated Entitlements**: Synthesize `.entitlements` property lists directly from Rust capabilities and CSG permissions.
- **Pre-Flight Packaging Verifier**: Add `rust-swift verify-packaging` CLI subcommand to audit Hardened Runtime flags, codesigning identities, and notarization readiness.

### Phase 5: CloudKit & Distributed CSG State Synchronization (`v0.6.0`)
- **Reflective Rust CSG to CloudKit**: Map Compiler Semantic Graphs and Operational State Graphs (OSG) directly into `CKRecord` hierarchies.
- **Deterministic Peer Replication**: Multi-device state replication for collaborative creative and document applications.

