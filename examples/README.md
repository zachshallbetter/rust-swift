# Rust to Swift Compiler Examples

This directory provides working demonstration projects, source files, and consumer applications illustrating zero-FFI compilation from Rust to native Swift.

## Directory Index

| Example | Type | Description |
|---|---|---|
| **[`fsm_example.rs`](fsm_example.rs)** | Rust Source | Core Finite State Machine (FSM) model demonstrating fieldless enums, pattern matching, state transitions, and score calculation with integer wrapping arithmetic. |
| **[`PlayerApp/`](PlayerApp/)** | macOS SwiftUI App | Interactive macOS application directly consuming the `GeneratedFSM` package with reactive SwiftUI state binding. |
| **[`peel_core.rs`](peel_core.rs)** | Rust Source | Audio processing and track inspection models illustrating payload enums (`TrackFormat`), structs, `Vec<T>`, inherent methods, and `#[derive(Codable)]`. |
| **[`PeelInspectorApp/`](PeelInspectorApp/)** | macOS SwiftUI App | Native macOS inspection application demonstrating pure value types, associated value extraction, and zero C bridging vs. traditional C-FFI wrappers. |
| **[`PluginDemoApp/`](PluginDemoApp/)** | SPM Package Target | Demonstrates the **`RustSwiftPlugin`** Swift Package Manager Build Tool Plugin. Automatically discovers and compiles `.rs` files during `swift build` without manual intervention. |
| **[`GeneratedFSM/`](GeneratedFSM/)** | Swift Package | Generated Swift package layout emitted from `fsm_example.rs` by `rust-swift compile`. |
| **[`PeelCoreKit/`](PeelCoreKit/)** | Swift Package | Generated Swift package layout emitted from `peel_core.rs` by `rust-swift compile`. |

## Quick Start

### 1. Compile and Run the SPM Build Tool Plugin Demo
```bash
cargo build
cd examples/PluginDemoApp
swift run
```

### 2. Compile and Launch the PlayerApp
```bash
cargo run -p rust-swift-cli -- compile examples/fsm_example.rs \
  --output-dir examples/GeneratedFSM \
  --package-name GeneratedFSM \
  --spm
cd examples/GeneratedFSM && swift build
cd ../PlayerApp
swiftc -parse-as-library App.swift \
  -I ../GeneratedFSM/.build/out/Products/Debug \
  -L ../GeneratedFSM/.build/out/Products/Debug \
  -lfsm_example \
  -o PlayerApp
./PlayerApp
```
