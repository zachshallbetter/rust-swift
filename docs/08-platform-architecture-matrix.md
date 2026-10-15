# Multi-Platform Architecture Decision Matrix: Rust Across iOS, macOS, tvOS, watchOS, and visionOS

**Architectural Analysis & Comparative Evaluation**  
*Lead Architect: [Zach Shallbetter](https://github.com/zachshallbetter) · [Reflective Rust Ecosystem](https://github.com/zachshallbetter/reflective-rust)*

---

## Executive Summary

When taking Rust-based domain logic and engines across the modern Apple ecosystem—**iOS/iPadOS, macOS, tvOS, watchOS, and visionOS**—as well as Android, engineering teams face a fundamental architectural choice: **how should core business logic, finite state machines, data models, and UI layers be structured across disparate form factors?**

Historically, teams explore four competing paths:
1. **Immediate-Mode GPU Canvas**: Running `egui` via Metal/wgpu and `winit`.
2. **Webview Container Packaging**: Wrapping web frontends with `Tauri Mobile` or Capacitor.
3. **Binary C-FFI / Bindings**: Compiling Rust crates to `.a` or `.xcframework` bundles with `UniFFI` or `bindgen`.
4. **Ahead-of-Time Zero-FFI Source Compilation**: Compiling Rust domain models, state machines, and algorithmic logic directly into native Swift 5.9+ source packages via `rust-swift`.

This document evaluates these paths across **all Apple platform targets**, comparing battery consumption, touch/focus physics, accessibility, memory constraints, and build toolchain ergonomics.

---

## Cross-Platform Architectural Matrix

| Platform Target | 1. `egui` Direct Canvas | 2. `Tauri` Webview | 3. `UniFFI` Binary FFI | 4. `rust-swift` Source Compilation |
|---|---|---|---|---|
| **iOS / iPadOS** (Touch & Glass) | **Friction**: Continuous 120Hz repaints burn battery; non-native touch physics; virtual keyboard occlusion | **Moderate**: Webview process overhead; synthetic momentum scrolling; memory pressure | **Functional**: Native SwiftUI frontend, but requires complex `.xcframework` slicing and C-ABI marshaling | **Optimal**: 100% native SwiftUI, native momentum scrolling, 0% idle GPU, `@Observable` synthesis |
| **macOS** (Desktop Citizen) | **Functional for tools**: Game-like canvas; lacks native menu bars, drag-and-drop, and window tabs | **Functional**: Desktop webview; heavy RAM footprint per window | **Functional**: Native AppKit/SwiftUI, but requires dylib codesigning & notarization toolchains | **Optimal**: First-class macOS citizen with native menu bar commands, multi-windowing, and zero FFI glue |
| **tvOS** (10-Foot UI & Siri Remote) | **Unsupported**: No mouse/touch; cannot integrate with the Apple **Focus Engine** | **Unsupported**: WebKit is not available on tvOS | **Complex**: Must build and sign dedicated `tvos` and `tvsimulator` binary slices | **Optimal**: Seamlessly binds to SwiftUI tvOS Focus Engine (`focused()`), Siri Remote swipes, and 4K HDR |
| **watchOS** (Wearable / Micro-UI) | **Unsupported**: No winit/Metal event loop; memory budget exceeds watchOS limits | **Unsupported**: Webview is not available on watchOS | **High Failure Risk**: Binary footprint frequently violates watchOS memory ceilings (<50–100 MB) | **Optimal**: Lightweight value-semantic Swift structs fit into ultra-low memory budgets; Digital Crown support |
| **visionOS** (Spatial Computing) | **Trapped in 2D**: Restricted to flat 2D projection planes; cannot bind to RealityKit 3D volumes | **Trapped in 2D**: Flat webview window; no spatial depth or gaze integration | **Complex**: Requires dedicated `xros` and `xrsimulator` slices; manual bridging to RealityKit | **Optimal**: Native SwiftUI 3D window groups, RealityView attachments, and privacy-preserving Gaze+Pinch |

---

## Evaluation Across Apple Platform Form Factors

### 1. tvOS: The 10-Foot Living Room Experience
- **Interaction Model**: tvOS relies entirely on the **Apple Focus Engine** (`UIFocusEnvironment`). Navigation is driven by Siri Remote directional swipes, micro-rotations, and haptic clicks—there are no mouse pointers or touch coordinates.
- **Why Canvas/Webview Fails**: `egui` and `Tauri` assume mouse clicks or touch taps. They cannot participate in the focus navigation graph or render tvOS parallax focus enlargement effects.
- **The `rust-swift` Advantage**: Because `rust-swift` emits native Swift 5.9+ packages declaring `tvOS(.v17)` support, the domain models bind directly to SwiftUI tvOS views with native `@FocusState` handling.

### 2. watchOS: Ultra-Low Power & Memory Budgets
- **Constraints**: watchOS enforces aggressive memory watchdog ceilings. Applications that exceed 50–100 MB of RAM or block the main thread for more than a few milliseconds are terminated immediately by the OS.
- **Why Binary FFI Fails**: Rust static libraries bundled into `.xcframework` archives carry significant metadata, allocator overhead, and runtime scaffolding. Stripping symbols across watchOS architectures is error-prone.
- **The `rust-swift` Advantage**: Emitted Swift code consists of pure Swift structs with Copy-on-Write (COW) semantics and zero runtime overhead. It compiles directly with `swiftc -O` and integrates with watchOS Complications and Digital Crown rotary inputs.

### 3. visionOS: Spatial Computing & RealityKit
- **Interaction Model**: visionOS uses a privacy-preserving **Gaze + Pinch** model. For user privacy, Apple does not allow applications to track eye gaze directly; the system manages focus internally and delivers events only when the user pinches.
- **Why Immediate-Mode Fails**: An immediate-mode canvas cannot intercept spatial gaze highlights. It remains a flat, hovering 2D rectangular surface inside the user's room.
- **The `rust-swift` Advantage**: Emitted models integrate directly into Spatial SwiftUI window groups, volumetric scenes (`Volume`), and `RealityView` component systems with Spatial Audio.

### 4. macOS: First-Class Desktop Citizenship
- **Desktop Expectations**: A true Mac app requires system menu bar menus (`Commands`), AppKit drag-and-drop, native window tabs, Services integration, macOS Settings sheets, and Spotlight indexing.
- **Why egui Feels Out of Place**: `egui` renders a monolithic, non-native canvas where text selection does not use Mac keybindings (`Cmd+Left`, `Option+Right`), context menus lack system services, and window resizing does not adhere to macOS AppKit window tiling.

---

## Detailed Comparison: `rust-swift` vs. `UniFFI`

`UniFFI` and `rust-swift` represent two legitimate but fundamentally different architectural solutions for Rust/Apple interoperability:

| Attribute | `UniFFI` (Binary FFI) | `rust-swift` (Source Compilation) |
|---|---|---|
| **Delivery Artifact** | Precompiled `.xcframework` containing binary static/dynamic libraries (`.a`, `.dylib`) | Pure Swift 5.9+ source package managed by Swift Package Manager (`Package.swift`) |
| **Toolchain Dependencies** | `cargo`, `cargo-xcode`, `lipo`, Python binding generators, Xcode run scripts | **Zero external build dependencies**: Runs directly inside Xcode via SPM Build Tool Plugin |
| **Multi-Architecture Slices** | Must cross-compile for up to 8 targets: `ios-arm64`, `ios-sim`, `macos-arm64`, `macos-x86_64`, `tvos`, `tvos-sim`, `watchos`, `watchos-sim`, `visionos`, `visionos-sim` | **1 Compilation Pass**: `swiftc` compiles the emitted Swift source for whichever active target Xcode is building |
| **Data Boundary Cost** | Serialization into shared buffers across C-ABI boundary on every function call | **Zero Marshaling Cost**: Direct register passing and native Swift struct field access |
| **Memory Management** | Rust RAII objects wrapped in heap-allocated Swift wrapper classes with opaque pointers | Native Swift value semantics (structs) and native Swift ARC (`final class`) |
| **Reactivity** | Manual callback streams or polling to update SwiftUI views | Automatic `@Observable` macro synthesis with `@Bindable` properties |
| **Ideal Use Case** | Massive legacy Rust engines with heavy async I/O (`Tokio`), custom C dependencies, or raw socket networking | Algorithmic domain logic, business rules, document models, calculation engines, and state machines |

---

## When to Choose What

```text
Do you have a massive Rust engine requiring async I/O (Tokio), low-level audio DSP, or raw sockets?
 ├── YES ──► Use UniFFI with an .xcframework binary bundle.
 └── NO
      │
      Are you building client applications for iOS, macOS, tvOS, watchOS, or visionOS?
       ├── YES ──► Use rust-swift: single-source Rust domain logic compiled into a universal SPM Swift package.
       └── NO  ──► (Internal desktop engineer tool / quick hardware test harness) ──► egui is acceptable.
```

---

## The Recommended Clean-Room Architecture

For multi-platform products delivering client software across Apple platforms and Android:

```text
       ┌────────────────────────────────────────────────────────┐
       │             Canonical Rust Core Logic                  │
       │   - Finite State Machines    - Algorithmic Models      │
       │   - Mathematical Engines     - Validation Rules        │
       └───────────────┬────────────────────────┬───────────────┘
                       │                        │
       Source-to-Source│ Lowering               │ (Targeted FFI / JNI)
                       ▼                        ▼
       ┌────────────────────────┐      ┌────────────────────────┐
       │   Native Swift Package │      │   Android Core (Kotlin)│
       │   - Pure Swift 5.9+    │      │   - Coroutines         │
       │   - @Observable Models │      │   - Compose State      │
       │   - Zero C-FFI / SPM   │      └───────────┬────────────┘
       └───────────────┬────────┘                  │
                       │                           ▼
      ┌────────────────┴────────────────┐ ┌─────────────────────┐
      │     Universal Apple Frontends   │ │   Jetpack Compose   │
      │  - iOS / iPadOS (SwiftUI Touch) │ │   (Android Phones,  │
      │  - macOS (SwiftUI Desktop)      │ │    Tablets, Wear)   │
      │  - tvOS (Focus Engine 10-foot)  │ └─────────────────────┘
      │  - watchOS (Crown & Complication│
      │  - visionOS (RealityKit Spatial)│
      └─────────────────────────────────┘
```

1. **Rust Remains the Canonical Source of Truth**: Core calculation logic, document schemas, and state machines are maintained once in Rust.
2. **`rust-swift` Provides Zero-FFI Portability**: The Rust models compile automatically into an SPM package targeting `macOS(.v14)`, `iOS(.v17)`, `tvOS(.v17)`, `watchOS(.v10)`, and `visionOS(.v1)`.
3. **Each Platform Gets Full Citizenship**: Every device form factor receives 100% native UI, optimal battery life, and complete accessibility without compromising architectural consistency.
