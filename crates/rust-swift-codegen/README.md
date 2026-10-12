# rust-swift-codegen

Native Swift code generator, Swift Package Manager manifest synthesizer, and bidirectional debugger source mapping emitter.

## Overview

`rust-swift-codegen` lowers `rust_swift_ir::Module` instances into clean, idiomatic, standalone Swift source code and buildable Swift packages with zero external C bridging or unsafe pointer dereferencing.

## Key Features

- **Zero-FFI Swift Emission**: Translates Rust types directly into Swift value types (`struct`, `enum`, `Array`) with copy-on-write semantics.
- **Arithmetic Invariant Lowering**: Preserves two's complement integer wrapping semantics by emitting Swift overflow operators (`&+`, `&-`, `&*`).
- **Traits to Protocols**: Lowers Rust `trait` definitions to Swift `protocol` definitions and trait method defaults to protocol extension default implementations.
- **SwiftUI Integration**:
  - Emits `@Observable public final class` for models annotated with `#[observable]`.
  - Synthesizes `struct Name: View { var body: some View { ... } }` for views annotated with `#[view]`, wiring state models with `@Bindable` and `@Binding`.
- **Codable Synthesis**: Emits `: Codable` conformances matching `serde_json` format byte-for-byte.
- **Bidirectional Source Mapping**: Emits `#sourceLocation(file:line:)` directives so Xcode, LLDB, and crash symbolication map back to the original `.rs` file.
- **SPM Package Emission**: Generates complete Swift 5.9+ package hierarchies (`Package.swift`, `Sources/`, `Tests/`) buildable with `swift build` and testable with `swift test`.
