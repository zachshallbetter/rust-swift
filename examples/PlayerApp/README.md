# PlayerApp

A native macOS SwiftUI application demonstrating reactive state management powered directly by a Rust Finite State Machine (FSM) compiled with `rust-swift` (Zero FFI, pure value semantics).

## Architecture

- **Source Logic**: `examples/fsm_example.rs` in Rust
- **Compiled Module**: `examples/GeneratedFSM` (`fsm_example` Swift package)
- **App Layer**: `App.swift` (`import SwiftUI`, `import fsm_example`)

## Building & Running

1. Compile the Rust source into a standalone Swift package:
```bash
cargo run -p rust-swift-cli -- compile examples/fsm_example.rs \
  --output-dir examples/GeneratedFSM \
  --package-name GeneratedFSM \
  --spm
```

2. Build the generated Swift package:
```bash
cd examples/GeneratedFSM
swift build
cd ../PlayerApp
```

3. Compile the SwiftUI app:
```bash
swiftc -parse-as-library App.swift \
  -I ../GeneratedFSM/.build/out/Products/Debug \
  -L ../GeneratedFSM/.build/out/Products/Debug \
  -lfsm_example \
  -o PlayerApp
```

4. Launch the native app:
```bash
./PlayerApp
```
