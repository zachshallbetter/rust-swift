Rust → Swift Compiler | Language Admission Matrix | 14 October 2026

## Implemented & Empirically Validated

The following constructs are admitted, lowered to standalone native Swift, and qualified through differential testing suites:

- **Functions & Control Flow**: Pure synchronous functions, local immutable and mutable bindings, `if`/`else`, `match` expressions, and `for`-in sequence loops.
- **Fixed-Width Arithmetic**: Integer types (`i8`..`i64`, `u8`..`u64`), boolean, checked operations, and explicit wrapping arithmetic (`wrapping_add`, `wrapping_sub`, `wrapping_mul` $\to$ `&+`, `&-`, `&*`).
- **Data Structures**:
  - Fieldless enums (`enum State: String, CaseIterable`).
  - Payload enums with associated values (`enum Event: Equatable, Hashable, Sendable`).
  - Structs (immutable value types and mutating methods via `mutating func`).
  - Tuples (native Swift tuples and `.0`, `.1` member indexing).
- **Dynamic Collections & Strings**:
  - Dynamic arrays (`Vec<T>` $\to$ `[T]`, `push` $\to$ `append`, `pop`, `clear`, `len` $\to$ `count`).
  - `Option<T>` and `Result<T, E>` monadic unwrapping and pattern matching.
  - UTF-8 strings (`String` and string slices), scalar preservation, and grapheme cluster handling.
- **Traits & Protocols**:
  - Trait declarations (`trait Foo`) lowered to Swift `protocol Foo`.
  - Default trait methods lowered to protocol extensions.
  - Conformance implementations (`impl Foo for Bar`) lowered to `extension Bar: Foo`.
- **System & Framework Integrations**:
  - Codable synthesis (`#[derive(Codable)]` on structs and enums).
  - SwiftUI `@Observable` class emission (`#[observable]` $\to$ `@Observable public final class`).
  - Contract assertions (`assert!`, `debug_assert!`, `assert_eq!`, `panic!` $\to$ `precondition`, `assert`, `fatalError`).
  - Debugger source mapping directives (`#sourceLocation(file:line:)`).
  - Swift Package Manager manifest generation (`Package.swift`, library products, test targets).

## Conditional / Next Phase

- Trait associated types and multi-parameter generic constraints.
- Owned resource disposal elaboration and deterministic cleanup boundaries.
- Specialized Apple system APIs and async runtime adapter bindings.

## Rejected Boundary (Diagnostic Codes E0001..E0012)

Constructs outside the verified boundary are rejected at parse time with stable, actionable diagnostic codes:

- `E0001_UNSAFE_REJECTED`: `unsafe` blocks and functions are forbidden.
- `E0002_ASYNC_REJECTED`: Asynchronous execution requires explicit runtime adapters.
- `E0003_RAW_POINTER_REJECTED`: Raw pointers (`*const T`, `*mut T`) are rejected.
- `E0004_MACRO_REJECTED`: Unadmitted macro invocations.
- `E0005_DYN_TRAIT_REJECTED`: Dynamic trait objects (`dyn Trait`).
- `E0006_TRAIT_IMPL_REJECTED`: Complex or aliased trait implementations.
- `E0007_CUSTOM_DROP_REJECTED`: Custom `Drop` implementations.
- `E0008_UNION_REJECTED`: Untagged unions.
- `E0009_SYNTAX_ERROR`: Rust syntax errors.
- `E0010_INT128_REJECTED`: 128-bit integers without native 64-bit target equivalents.
- `E0011_GENERICS_REJECTED`: Unbounded generics.
- `E0012_INTERIOR_MUTABILITY_REJECTED`: `Cell` and `RefCell` types.

## Diagnostics Policy

Unsupported constructs produce stable diagnostic codes, source locations, semantic reason, and suggested alternatives; never emit misleading approximate Swift.
