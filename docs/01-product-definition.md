Rust → Swift Compiler \| Proposal v0.1 \| 13 October 2026

## Purpose

Build a Rust-to-Swift source compiler that produces independently executable Swift for a constrained Rust subset. This is not an FFI generator, and it must not claim equivalence without evidence.

## Success criteria

Generated Swift compiles without linking the source Rust library; accepted fixtures agree on specified outputs and observable traces; unsupported constructs fail with actionable diagnostics.

## Non-goals

Arbitrary Rust translation, full standard-library parity, async runtimes, unsafe Rust, and immediate multi-language backends are outside v0.1.

## Modes

Compatibility mode is authoritative. Idiomatic Swift mode is optional and must discharge the same semantic obligations.

## Acceptance

A reproducible CLI builds a Swift package, emits a manifest, and runs differential tests on the initial finite-state and arithmetic fixtures.
