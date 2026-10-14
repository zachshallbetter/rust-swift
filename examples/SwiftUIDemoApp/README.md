# SwiftUIDemoApp

A native macOS SwiftUI application demonstrating declarative view synthesis and `@Observable` state binding from Rust domain models via the **`RustSwiftPlugin`** SPM Build Tool Plugin.

## Architecture

1. **Rust Source (`Sources/SwiftUIDemoApp/Model.rs`)**:
   - `#[observable] pub struct CounterModel`: Observation-tracked state model with methods.
   - `#[view] pub struct CounterView`: Declarative UI component tree written in Rust syntax (`VStack`, `HStack`, `Button`, `Spacer`).
2. **Automated SPM Plugin (`RustSwiftPlugin`)**:
   - When building with `swift build` or Xcode, the plugin automatically invokes `rust-swift compile` to lower `Model.rs` into native Swift:
     * `CounterModel` $\to$ `@Observable public final class CounterModel`
     * `CounterView` $\to$ `public struct CounterView: View { @Bindable var model: CounterModel ... }`
3. **Application Entry Point (`Sources/SwiftUIDemoApp/main.swift`)**:
   - Standard `@main struct SwiftUIDemoApp: App` window displaying `CounterView(model: model)`.

## Building and Running

```bash
# 1. Build the rust-swift compiler
cargo build

# 2. Build the SwiftUI application
cd examples/SwiftUIDemoApp
swift build

# 3. Launch the native app binary
.build/debug/SwiftUIDemoApp
```
