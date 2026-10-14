# PeelInspectorApp

A native macOS SwiftUI application powered directly by Rust models and logic from `peel-core`, compiled into native Swift using the `rust-swift` compiler with **Zero FFI, zero C headers, and zero unsafe pointers**.

## Architecture Comparison

### Before (Traditional C FFI):
- `peel-core` (Rust)
- $\to$ `peel-ffi` (C ABI via `*const c_char`, `peel_string_free`, `CString`, manual memory management)
- $\to$ `PeelKit` (Swift wrapper decoding JSON strings over C pointers)
- $\to$ SwiftUI / AppKit

### After (`rust-swift` Direct Compilation):
- `peel_core.rs` (Rust)
- $\to$ **`rust-swift` compiler**
- $\to$ `PeelCoreKit` (Pure Native Swift value types & functions, zero FFI)
- $\to$ `PeelInspectorApp` (Direct import: `import PeelCoreKit`, direct switch/struct access)

## Building & Running

1. Compile the Rust domain logic into the `PeelCoreKit` Swift package:
```bash
cargo run -p rust-swift-cli -- compile examples/peel_core.rs \
  --output-dir examples/PeelCoreKit \
  --package-name PeelCoreKit \
  --spm \
  --codable
```

2. Build the generated Swift package:
```bash
cd examples/PeelCoreKit
swift build
cd ../PeelInspectorApp
```

3. Compile the SwiftUI app:
```bash
swiftc -parse-as-library App.swift \
  -I ../PeelCoreKit/.build/out/Products/Debug \
  -L ../PeelCoreKit/.build/out/Products/Debug \
  -lPeelCoreKit \
  -o PeelInspectorApp
```

4. Launch the native app:
```bash
./PeelInspectorApp
```
