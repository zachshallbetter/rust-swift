Rust → Swift Compiler \| Proposal v0.1 \| 13 October 2026

## Status

Proposed. Architecture and acceptance criteria only; no implementation is asserted.

## Decision

Adopt a source compiler with a target-neutral typed semantic IR, a compatibility-first Swift backend, explicit rejection of unsupported constructs, and differential qualification.

## Rationale

Direct syntax rewriting cannot reliably preserve integer, Unicode, ownership, destruction, and concurrency behavior. An IR makes translation choices and their obligations inspectable.

## Alternatives

UniFFI and C ABI preserve Rust execution but do not produce independent Swift implementations. Direct transpilation is simpler initially but accumulates untracked semantic exceptions. Full LLVM-based recompilation does not yield maintainable Swift source.

## Consequences

Requires a pinned frontend, compiler-specific diagnostics, compatibility runtime, and a large fixture corpus. Native Swift output becomes possible for admitted programs.

## Open decisions

Choose rustc integration mechanism; define IR serialization and stability policy; select initial Swift deployment baseline; determine whether panic is observable in v0.1.

## First experiment

Compile a four-state transition function and wrapping Int32 addition into standalone Swift, compare all state/event pairs and integer edge vectors, and publish receipts.
