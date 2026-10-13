# Project Intent

**Project ID:** `rust-swift`  
**Project Name:** `Rust to Swift Compiler`  
**Status:** Active

## Originating intent

The `rust-swift` project builds a high-assurance, source-to-source compiler from a constrained, admitted subset of Rust to independently executable, standalone Swift. Unlike FFI binding generators (e.g. UniFFI) or C ABI wrappers, this compiler emits native, self-contained Swift packages that compile without linking the original Rust library or runtime, while guaranteeing semantic fidelity across integer overflow modes, finite-state transitions, struct representations, and control flow. Unsupported constructs are rejected with stable diagnostic error codes.

## Desired state

```text
CONSTRAINED RUST SOURCE (v0.1)
      ↓
FRONTEND (syn parse + Language Matrix policy enforcement)
      ↓
TARGET-NEUTRAL SEMANTIC IR + COMPILER SEMANTIC GRAPH (CSG)
      ↓
SWIFT CODEGEN (Swift AST + Native Overflow & Structural Lowering)
      ↓
STANDALONE SWIFT PACKAGE + DIFFERENTIAL QUALIFICATION
```

## Constraints

```text
toolchain: rustc 1.88+ / Swift 5.9+ / macOS v13+ baseline
reproducibility: deterministic code generation, stable symbol maps, provenance digests
semantics: explicit wrapping (&+, &-, &*) and checked arithmetic; fieldless enums; pure functions
epistemic integrity: Documented != Implemented != Tested != Empirically Validated
licensing: MIT OR Apache-2.0
```

## Non-goals

```text
v0.1 non-goals:
- Arbitrary unconstrained Rust translation
- Full standard library emulation
- Async / await runtimes
- Unsafe blocks and raw pointer operations
- Dynamic dispatch (dyn Trait) and complex trait bounds
- FFI and C ABI linking
```

## Success questions

At maturity, canonical state answers:
- What subset of Rust is admitted vs rejected? (See `docs/04-language-matrix.md`)
- How are integer overflow and wrapping operations lowered to Swift? (Via native `&+`, `&-`, `&*`)
- What evidence supports semantic equivalence? (Differential test execution receipts in `records/evidence.jsonl`)
- What provenance is emitted? (Compiler Semantic Graph `csg.json` conforming to `reflective-rust-csg`)

## Termination / dormancy

The v0.1 milestone terminates upon passing differential qualification across finite-state transition tables, integer boundary vectors, negative rejection diagnostics, and successful standalone `swift build` of generated packages.
