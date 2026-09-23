# PlayerApp

A native macOS SwiftUI application demonstrating reactive state management powered directly by a Rust Finite State Machine (FSM) compiled with `rust-swift` (Zero FFI, pure value semantics).

## Architecture

- **Source Logic**: `examples/fsm_example.rs` in Rust
- **Compiled Module**: `examples/GeneratedFSM` (`fsm_example` Swift package)
- **App Layer**: `App.swift` (`import SwiftUI`, `import fsm_example`)

## Building & Running

1. Build the generated Swift package:
```bash
cd ../GeneratedFSM
swift build
```

2. Compile the SwiftUI app:
```bash
swiftc -parse-as-library App.swift \
  -I ../GeneratedFSM/.build/out/Products/Debug \
  -L ../GeneratedFSM/.build/out/Products/Debug \
  -lfsm_example \
  -o PlayerApp
```

3. Launch the native app:
```bash
./PlayerApp
```
