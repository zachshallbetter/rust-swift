# Changelog

All notable changes to the Rust to Swift Compiler will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
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
