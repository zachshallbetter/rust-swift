# rust-swift-frontend

Syntactic parsing, language matrix admission checking, and semantic AST lowering for the `rust-swift` compiler.

## Overview

`rust-swift-frontend` ingests Rust source code (via `syn`), verifies it against the formally admitted language boundary, and lowers it into the target-neutral `rust_swift_ir::Module`.

## Responsibilities

1. **AST Parsing**: Preserves source spans, identifier names, and attributes.
2. **Language Matrix Policy Enforcement**: Detects unadmitted or unsafe Rust constructs and rejects them early with stable, deterministic diagnostic error codes:
   - `E0001_UNSAFE_REJECTED`: `unsafe` blocks and functions
   - `E0002_ASYNC_REJECTED`: `async` functions and awaits
   - `E0003_RAW_POINTER_REJECTED`: Raw pointer types (`*const T`, `*mut T`)
   - `E0004_MACRO_REJECTED`: Unadmitted macro invocations
   - `E0005_DYN_TRAIT_REJECTED`: Dynamic trait objects (`dyn Trait`)
   - `E0006_TRAIT_IMPL_REJECTED`: Complex or aliased trait implementations
   - `E0007_CUSTOM_DROP_REJECTED`: Custom `Drop` implementations
   - `E0008_UNION_REJECTED`: Untagged unions
   - `E0009_SYNTAX_ERROR`: Rust syntax errors
   - `E0010_INT128_REJECTED`: 128-bit integers without 64-bit target equivalents
   - `E0011_GENERICS_REJECTED`: Unbounded generics
   - `E0012_INTERIOR_MUTABILITY_REJECTED`: `Cell` and `RefCell` types
3. **Semantic Lowering**: Lowers structs, enums (fieldless and payload), trait declarations, trait implementations, pure functions, loops, contracts (`assert!`, `debug_assert!`, `panic!`), `@Observable` models, and `#[view]` component hierarchies into `rust-swift-ir`.
