Rust → Swift Compiler \| Proposal v0.1 \| 13 October 2026

## v0.1 admitted

Pure synchronous functions; fixed-width integers and bool; local bindings; if, match, loops; simple structs and fieldless enums; direct calls; explicit wrapping operations.

## v0.1 conditional

Simple borrowed parameters and strings only when lifetime and byte semantics are explicitly supported. No silent coercion.

## v0.1 rejected

Unsafe blocks, raw pointers, closures capturing borrows, complex traits, dynamic dispatch, macros without supported expansion, async, threads, FFI, interior mutability, and custom Drop.

## v0.2 candidates

Option, Result, UTF-8 strings, Vec, nested enums, generic specialization, and deterministic parsing.

## v0.3 candidates

Traits, associated types, owned resources, deterministic drop elaboration, and selected filesystem interfaces.

## Later research

Async state machines, cancellation, atomics, channels, shared mutable state, unsafe code, and external crate compatibility.

## Diagnostics policy

Unsupported constructs produce stable diagnostic codes, source locations, semantic reason, and suggested alternatives; never emit misleading approximate Swift.
