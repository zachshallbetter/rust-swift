# PluginDemoApp

An example Swift executable package demonstrating the **`RustSwiftPlugin`** Swift Package Manager Build Tool Plugin.

## How It Works

Instead of manually running code generation scripts before building your Swift project:
1. `models.rs` lives directly inside `Sources/PluginDemoApp/models.rs`.
2. `Package.swift` attaches `.plugin(name: "RustSwiftPlugin", package: "rust-swift")` to the target.
3. When you run `swift build` (or build in Xcode), the plugin:
   - Scans the target directory for all `.rs` files.
   - Invokes `rust-swift compile` to generate native Swift sources into the build cache directory.
   - Passes the generated Swift files into the `swiftc` compilation pass.
4. `main.swift` seamlessly consumes `MachineStatus` and `advance(...)` without any intermediate bridging or manual steps.

## Running the Example

```bash
# 1. Build the rust-swift compiler CLI
cargo build

# 2. Build and run PluginDemoApp via Swift Package Manager
cd examples/PluginDemoApp
swift run
```

Expected output:
```text
PLUGIN_RESULT:Starting->Running
```
