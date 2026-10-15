---
name: rust-to-swift
description: Guides autonomous coding agents and LLMs (Claude, Cursor, Codex, Antigravity) on compiling, checking, verifying, and auto-remediating Rust-to-Swift codebases using the native rust-swift compiler, MCP tools, and CLI diagnostics.
---

# Rust-to-Swift Agent Integration Skill

## Overview

The `rust-swift` compiler translates algorithmic Rust core logic into native, standalone Swift packages (with zero-cost Swift code and SPM manifests) without C-FFI overhead.

This skill equips autonomous AI coding agents (Claude, Codex, Cursor, Antigravity) to:
1. Perform pre-flight syntax and policy checks before committing changes.
2. Interpret machine-readable diagnostic codes (`E0001` through `E0015`).
3. Auto-remediate rejected patterns into compliant, high-assurance Rust.
4. Execute compilations and verify formal invariant proof receipts.

**Architectural Credits**:
- Lead Architect: **Zach Shallbetter**
- Formal Foundations & Semantic Graph: **Reflective Rust** (`reflective-rust-csg`, `reflective-rust-protocol`)

---

## 1. Quick Agent Cheatsheet

### Fast Diagnostics via CLI
```bash
# Pre-flight check returning structured JSON diagnostics
rust-swift check path/to/source.rs --format json

# Compile directly into a standalone Swift package
rust-swift compile path/to/source.rs --output-dir ./generated-swift --spm --format json

# Generate formal epistemic proof dossier
rust-swift proof path/to/source.rs --format json
```

### Pre-flight Diagnostic Schema (`--format json`)
```json
{
  "status": "admitted | rejected",
  "file": "path/to/source.rs",
  "module_name": "fsm_module",
  "type_count": 4,
  "function_count": 6,
  "diagnostics": [
    {
      "code": "E0001_UNSAFE_REJECTED",
      "message": "Unsafe blocks and functions are rejected in v0.1",
      "file": "path/to/source.rs",
      "start_line": 12,
      "end_line": 14,
      "suggested_remedy": "Replace with safe Rust value semantics or indexing"
    }
  ]
}
```

---

## 2. Language Matrix & Diagnostic Code Reference

When `rust-swift check` or `rust_swift_check` returns `"status": "rejected"`, the agent should inspect the diagnostic code and apply the standard remediation pattern:

| Diagnostic Code | Rejected Construct | Rationale | Agent Remediation Pattern |
|---|---|---|---|
| **`E0001_UNSAFE_REJECTED`** | `unsafe { ... }`, `unsafe fn` | Violates deterministic memory safety guarantee | Replace pointer manipulation with slice indexing, safe arrays, or value types. |
| **`E0002_ASYNC_REJECTED`** | `async fn`, `.await` | Async runtimes differ fundamentally between Rust (Poll/Waker) and Swift (cooperative thread pool) | Refactor into synchronous finite state machines (FSM) or explicit step/tick functions. |
| **`E0003_RAW_POINTER_REJECTED`** | `*const T`, `*mut T` | Unbounded memory access cannot be verified across compilers | Use standard references (`&T`, `&mut T`), indices, or owned collections. |
| **`E0004_MACRO_REJECTED`** | Custom procedural/declarative macros | Macros obscure the semantic graph | Pre-expand macros or declare explicit structs, enums, and functions. |
| **`E0005_DYNAMIC_DISPATCH_REJECTED`** | `dyn Trait`, `Box<dyn Trait>` | Dynamic vtables prevent compile-time qualification | Use monomorphic generics (`<T: Trait>`) or algebraic tagged enums with pattern matching. |
| **`E0006_COMPLEX_TRAIT_REJECTED`** | Higher-Ranked Trait Bounds (`for<'a>`), complex associated types | Trait complexity exceeds Swift protocol synthesis boundary | Flatten trait bounds or define simple Swift-friendly protocols. |
| **`E0007_CLOSURE_BORROW_REJECTED`** | Escaping closures capturing mutable environment | Differing capture semantics between Rust and Swift ARC | Pass captured state explicitly as arguments or use value-capturing closures. |
| **`E0008_UNSUPPORTED_TYPE_REJECTED`** | Unsupported stdlib types | Type does not have 1:1 Swift runtime semantic mapping | Stick to supported primitives: integers (`i8`..`i64`, `u8`..`u64`), `bool`, `String`, `Vec<T>`, `Option<T>`, `Result<T, E>`, tuples, structs, and enums. |
| **`E0009_SYNTAX_ERROR`** | Malformed Rust syntax | Parse failure in `syn` | Fix Rust grammar syntax error. |
| **`E0010_ALIASED_MUTATION_REJECTED`** | Multiple overlapping mutable borrows | Violates Swift Law of Exclusivity (`inout` access) | Sequence mutations linearly; return updated state instead of simultaneous multi-aliasing. |
| **`E0011_UNSPECIALIZABLE_GENERIC_REJECTED`** | Unbounded generic parameters | Cannot monomorphize during code generation | Constrain generic types or specialize with concrete implementations. |
| **`E0012_INTERIOR_MUTABILITY_REJECTED`** | `RefCell<T>`, `Cell<T>`, `Mutex<T>` | Bypasses borrow checker at runtime | Use pure value semantics with `&mut self` on structs. |
| **`E0013_SHARED_REFERENCE_REJECTED`** | Escaping or long-lived shared references across struct fields | Rust lifetimes cannot be checked by Swift compiler | Use owned types (`String`, `Vec<T>`) or cloned values in structs. |
| **`E0014_CUSTOM_DROP_REJECTED`** | `impl Drop for T` | Swift ARC manages lifecycle; custom drop timings diverge | Provide explicit `.close()`, `.reset()`, or `.cleanup()` methods on the type. |
| **`E0015_INT128_UNSUPPORTED_REJECTED`** | `i128`, `u128` | Swift has no native 128-bit integer type | Use `i64`, `u64`, or a pair `(u64, u64)` representation. |

---

## 3. Recommended Agent Workflow

### Pattern: Test-Driven Autonomous Translation
```
1. Write/Review Rust Source
   ↓
2. Pre-flight Check (MCP rust_swift_check or CLI check --format json)
   ├─ If Rejected: Read diagnostics[].suggested_remedy, patch Rust in-memory, re-check
   ↓
3. Compile (MCP rust_swift_compile or CLI compile --spm --format json)
   ↓
4. Prove (MCP rust_swift_proof or CLI proof --format json)
   ↓
5. Inspect Certified Proof Dossier (INV-MEM-001, INV-ARITH-001, INV-FSM-001)
```

---

## 4. MCP Tools Integration

When running as an MCP client (Claude Desktop, Claude Code, Cursor), configure the server:

```json
{
  "mcpServers": {
    "rust-swift": {
      "command": "rust-swift",
      "args": ["mcp"]
    }
  }
}
```

### Available MCP Tools:

1. **`rust_swift_check`**:
   - **Arguments**: `source: string`, `codable?: bool`, `observable?: bool`
   - **Returns**: Fast admission check and diagnostic list without touching disk.

2. **`rust_swift_compile`**:
   - **Arguments**: `source: string`, `module_name?: string`, `codable?: bool`, `observable?: bool`
   - **Returns**: Formatted Swift source code, generated symbols, and CSG hash.

3. **`rust_swift_proof`**:
   - **Arguments**: `source: string`
   - **Returns**: Full epistemic proof dossier certifying memory safety, arithmetic overflow modes, and state totality.

4. **`rust_swift_explain`**:
   - **Arguments**: `code: string` (e.g., `"E0001_UNSAFE_REJECTED"`)
   - **Returns**: Human and agent explanations with canonical auto-remediation examples.
