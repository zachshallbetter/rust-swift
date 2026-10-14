# `rust-swift` Agent Operating Contract & Execution Guide

This document defines the specialized execution contract, architectural doctrine, toolchain interfaces, and verification requirements for autonomous coding agents (Claude Code, Cursor, Codex, Antigravity) working within the `rust-swift` repository.

For root project governance and authority precedence, see [`AGENTS.md`](../AGENTS.md) at the repository root.

---

## 1. Architectural Philosophy & Core Invariants

The `rust-swift` compiler is a **high-assurance, zero-FFI source-to-source compiler**. It translates algorithmic Rust domain logic into clean, idiomatic, standalone Swift 5.9+ packages without intermediate C bridging layers or runtime FFI wrappers.

### Lead Architect & Credits
- **Lead Architect**: [Zach Shallbetter](https://github.com/zachshallbetter)
- **Foundational Technology**: [Reflective Rust](https://github.com/zachshallbetter/reflective-rust) (`reflective-rust-csg`, `reflective-rust-protocol`)
- **Governance**: Formal Project Bootstrap (Epistemic Integrity & Four-Tier Claim Taxonomy)

### Non-Negotiable Invariants
1. **Zero C-FFI / Zero Bridging**: The compiler emits 100% native Swift code (`struct`, `enum`, `@Observable class`, `protocol`). It never emits C headers, `extern "C"`, or unsafe pointer-bridging shims.
2. **Deterministic Code Generation**: Identical Rust inputs must generate byte-for-byte identical Swift outputs and identical Compiler Semantic Graph (CSG) digests.
3. **No-Op Protection**: CI workflows and differential test runners must verify assertion counts ($\ge 20$ executed assertions) to guarantee skipped or category-error jobs cannot produce false green checks.
4. **SPM Absolute Path Resolution**: Swift Package Manager build tool plugins require canonical absolute URL paths for all executable commands. Never supply bare or relative paths to `PluginContext.buildCommand`.
5. **Swift Executable Entry Points**: In Swift, a file named `main.swift` is an executable top-level script. Do **not** decorate structs with `@main` inside `main.swift` without `-parse-as-library`; use idiomatic top-level code.

---

## 2. Workspace Crate Topography

Agents modifying compiler behavior must understand which crate owns each stage of the compilation pipeline:

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

| Crate / Directory | Architectural Role | Key Responsibilities |
|---|---|---|
| [`crates/rust-swift-frontend/`](../crates/rust-swift-frontend/) | **Frontend & Parser** | `syn` AST lowering, Language Matrix policy enforcement, stable diagnostic emission (`E0001`..`E0015`). |
| [`crates/rust-swift-ir/`](../crates/rust-swift-ir/) | **Semantic IR & CSG** | Target-neutral typed intermediate representation (`Module`, `Item`, `Type`, `Expr`), `to_csg()` projection. |
| [`crates/rust-swift-codegen/`](../crates/rust-swift-codegen/) | **Swift Code Generator** | AST lowering to Swift 5.9+ source, `@Observable` synthesis, `Codable` synthesis, `#sourceLocation` directives, `Package.swift`. |
| [`crates/rust-swift-qualification/`](../crates/rust-swift-qualification/) | **Qualification & Verification** | 16 differential test suites, failure mutation injection, runtime parity benchmarks, CSG delta watcher sync engine, and epistemic proof verifier. |
| [`crates/rust-swift-mcp/`](../crates/rust-swift-mcp/) | **Model Context Protocol** | Native stdio JSON-RPC 2.0 server exposing `rust_swift_check`, `rust_swift_compile`, `rust_swift_proof`, and `rust_swift_explain`. |
| [`crates/rust-swift-cli/`](../crates/rust-swift-cli/) | **Unified CLI** | Subcommands: `compile`, `check`, `diff-test`, `eval`, `bench`, `proof`, `sync`, `mcp`, `manifest`. |
| [`Plugins/RustSwiftPlugin/`](../Plugins/RustSwiftPlugin/) | **SPM Build Tool Plugin** | Native Swift Package Manager build tool plugin for automatic compilation of `.rs` files during `swift build` passes. |
| [`.agents/skills/rust-to-swift/`](skills/rust-to-swift/) | **Portable Agent Skill** | Canonical skill definition for autonomous assistants. |

---

## 3. Language Matrix Boundary & Diagnostic Remediation

Constructs outside the admitted boundary are rejected at parse time with stable, deterministic error codes. When an agent receives a rejection, it must apply the standard remediation pattern:

| Diagnostic Code | Rejected Construct | Rationale | Autonomous Remediation Pattern |
|---|---|---|---|
| **`E0001_UNSAFE_REJECTED`** | `unsafe { ... }`, `unsafe fn` | Violates deterministic memory safety guarantee | Replace pointer manipulation with slice indexing, safe arrays, or value types. |
| **`E0002_ASYNC_REJECTED`** | `async fn`, `.await` | Async runtimes differ fundamentally between Rust (Poll/Waker) and Swift (cooperative thread pool) | Refactor into synchronous finite state machines (FSM) or explicit step/tick functions. |
| **`E0003_RAW_POINTER_REJECTED`** | `*const T`, `*mut T` | Unbounded memory access cannot be verified across compilers | Use standard references (`&T`, `&mut T`), indices, or owned collections. |
| **`E0004_MACRO_REJECTED`** | Custom procedural/declarative macros | Macros obscure the semantic graph | Pre-expand macros or declare explicit structs, enums, and functions. |
| **`E0005_DYNAMIC_DISPATCH_REJECTED`** | `dyn Trait`, `Box<dyn Trait>` | Dynamic vtables prevent compile-time qualification | Use monomorphic generics (`<T: Trait>`) or algebraic tagged enums with pattern matching. |
| **`E0006_COMPLEX_TRAIT_REJECTED`** | Higher-Ranked Trait Bounds (`for<'a>`), complex associated types | Trait complexity exceeds Swift protocol synthesis boundary | Flatten trait bounds or define simple Swift-friendly protocols. |
| **`E0007_CLOSURE_BORROW_REJECTED`** | Escaping closures capturing mutable environment | Differing capture semantics between Rust and Swift ARC | Pass captured state explicitly as arguments or use value-capturing closures. |
| **`E0008_UNSUPPORTED_TYPE_REJECTED`** | Unsupported stdlib types | Type does not have 1:1 Swift runtime semantic mapping | Stick to supported primitives: integers (`i8`..`i64`, `u8`..`u64`), `bool`, `String`, `Vec<T>`, `Option<T>`, `Result<T, E>`, tuples, structs, and enums. |
| **`E0009_SYNTAX_ERROR`** | Malformed Rust syntax | Parse failure in `syn` | Fix Rust grammar syntax error. |
| **`E0010_ALIASED_MUTATION_REJECTED`** | Multiple overlapping mutable borrows | Violates Swift Law of Exclusivity (`inout` access) | Sequence mutations linearly; return updated state instead of simultaneous multi-aliasing. |
| **`E0011_UNSPECIALIZABLE_GENERIC_REJECTED`** | Unbounded generic parameters | Cannot monomorphize during code generation | Constrain generic types or specialize with concrete implementations. |
| **`E0012_INTERIOR_MUTABILITY_REJECTED`** | `RefCell<T>`, `Cell<T>`, `Mutex<T>` | Bypasses borrow checker at runtime | Use pure value semantics with `&mut self` on structs. |
| **`E0013_SHARED_REFERENCE_REJECTED`** | Escaping or long-lived shared references across struct fields | Rust lifetimes cannot be checked by Swift compiler | Use owned types (`String`, `Vec<T>`) or cloned values in structs. |
| **`E0014_CUSTOM_DROP_REJECTED`** | `impl Drop for T` | Swift ARC manages lifecycle; custom drop timings diverge | Provide explicit `.close()`, `.reset()`, or `.cleanup()` methods on the type. |
| **`E0015_INT128_UNSUPPORTED_REJECTED`** | `i128`, `u128` | Swift has no native 128-bit integer type | Use `i64`, `u64`, or a pair `(u64, u64)` representation. |

---

## 4. Agent Tooling Interfaces

### A. Model Context Protocol (MCP) Server
The native MCP server runs over stdio via `cargo run -p rust-swift-cli -- mcp` or the installed `rust-swift mcp` binary. Agents can call:
- **`rust_swift_check`**: Pre-flight validation in memory (`{ source: string }`). Returns `status: "admitted" | "rejected"` and structured diagnostics.
- **`rust_swift_compile`**: Compiles snippet to Swift code (`{ source: string, module_name?: string }`).
- **`rust_swift_proof`**: Verifies invariants and returns formal epistemic proof receipt.
- **`rust_swift_explain`**: Returns remediation guidance for diagnostic codes (`E0001`..`E0015`).

### B. Machine-Readable CLI Diagnostics (`--format json`)
Agents running terminal commands should pass `--format json`:
```bash
# Pre-flight check
rust-swift check path/to/source.rs --format json

# Compile with JSON result summary
rust-swift compile path/to/source.rs --output-dir ./generated-swift --spm --format json

# Epistemic proof dossier
rust-swift proof path/to/source.rs --format json
```

---

## 5. Required Verification Sequence

Before declaring any coding task complete or landing a branch, an agent MUST execute and verify the following sequence:

```bash
# 1. Format Check (0 diffs allowed)
cargo fmt --all --check

# 2. Clippy Linter (0 warnings allowed)
cargo clippy --workspace --all-targets -- -D warnings

# 3. Workspace Build
cargo build --workspace

# 4. Workspace Tests (all 68+ tests must pass)
cargo test --workspace

# 5. Differential Qualification Suite (all 16 suites must pass)
cargo run -p rust-swift-cli -- diff-test

# 6. Failure Injection & Mutation Qualification (proves gates detect defects)
cargo run -p rust-swift-cli -- eval

# 7. Differential Performance Benchmarks (throughput & runtime parity)
cargo run -p rust-swift-cli -- bench

# 8. Bootstrap & Protocol Integrity
python3 scripts/validate-bootstrap.py

# 9. Compiled Context Regeneration
python3 scripts/gen-context.py
```

---

## 6. Status Honesty & Epistemic Governance

Agents must adhere to the **Four-Tier Claim Taxonomy**:

$$\text{Documented} \neq \text{Implemented} \neq \text{Tested} \neq \text{Empirically Validated}$$

- **Documented**: Specified in PRD, RFC, or README.
- **Implemented**: Code exists in workspace crates.
- **Tested**: Covered by unit tests or qualification runners.
- **Empirically Validated**: Proven via differential execution against `rustc` and `swiftc`, mutation testing, and cryptographic proof receipts (`proof-receipt.json`).

Never conflate these tiers. If a construct compiles but hasn't run differentials, it is *tested*, not *empirically validated*.

### Negative Result Retention
When an architectural direction, lowering strategy, or optimization is disproven or rejected, log it permanently in [`records/negative-results.jsonl`](../records/negative-results.jsonl). Never delete negative results.
