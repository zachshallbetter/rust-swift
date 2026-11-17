---
description: Code style, lint and formatting requirements, safety boundaries, and concurrency safety
globs: ["**/*"]
---

# Code Style & Engineering Standards

This document defines the coding conventions and safety boundaries for projects operating under the Formal Project Bootstrap doctrine. The principles apply to every stack; the commands are the project's own.

---

## 1. Toolchain

Fill this table when the project's stack is chosen, and keep it current. CI and pre-commit run exactly these commands.

| Purpose | Command |
|---|---|
| Formatter | `<project formatter, check mode>` |
| Linter | `<project linter, warnings as errors>` |
| Type checker | `<project type checker, or "none: language is untyped">` |
| Test runner | `<project test command>` |

Use the project's own linter, type checker and test runner. Do not add a second toolchain for the same purpose.

---

## 2. Formatting

- One formatter, run by the project's tooling and checked in CI; formatting is never argued in review.
- **Imports**: group them consistently: standard library, third-party, internal modules, local items.
- **Trailing commas / diff churn**: prefer the formatter setting that keeps diffs to the lines that changed.

---

## 3. Lint Policy

- Warnings are treated as errors in CI and pre-commit.
- Enable the linter's strictest ruleset the project can pass, and record each suppression with a reason beside it.
- Production code must not crash on bad input: no unchecked assertions, forced unwraps, uncaught exceptions or `panic`-style calls on paths reachable from foreign input. Tests may use them for brevity.

---

## 4. Unsafe Code Boundary

- Memory-unsafe or type-system-escaping constructs (unsafe blocks, FFI, native addons, `any` casts, unchecked casts) are forbidden by default.
- Where hardware, memory mapping or native platform APIs require them, they must be:
  1. Isolated in one low-level module.
  2. Scoped to the smallest block that needs them.
  3. Documented with the invariant that makes them sound (pointer provenance, thread affinity, lifetime).
  4. Tested against edge conditions and boundary violations.

---

## 5. Concurrency & Resource Safety

- Never crash while acquiring a lock; recover from a poisoned or failed lock where the language has that failure.
- Real-time or latency-sensitive paths must never block indefinitely or allocate unbounded memory sized by foreign input.
