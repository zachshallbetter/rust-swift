# Security Policy

## Reporting a vulnerability

Report security issues privately through GitHub private vulnerability reporting:
<https://github.com/zachshallbetter/rust-swift/security/advisories/new>.
Do not open a public issue for a vulnerability.

Include a description of the issue and its impact, steps to reproduce (a minimal example is ideal), and the affected version or commit.

This is a solo-maintained project. Handling is best-effort with no SLA: there is no guaranteed acknowledgement or fix time, and the maintainer will respond when next active.

## Scope

`rust-swift` compiles a constrained subset of Rust into standalone Swift source. In scope:

- The compiler (`crates/rust-swift-frontend`, `rust-swift-ir`, `rust-swift-codegen`): crashes, hangs or unbounded resource use on crafted Rust input, and generated Swift that does not preserve the semantics the lowering contracts claim.
- The MCP server (`crates/rust-swift-mcp`), a stdio JSON-RPC 2.0 server that accepts source text from an agent client.
- The SwiftPM build tool plugin (`Plugins/RustSwiftPlugin`), which scans target directories for `.rs` files and compiles them before the target builds.
- The release and signing scripts (`scripts/asc-token.py`, `scripts/apple-notarize.sh`) and `.github/workflows/release.yml`: anything that could expose or misuse signing material. No secrets belong in the repository.

Build-time use of the compiler on untrusted Rust source is the main exposure. Reports about the correctness of generated Swift for constructs the compiler admits are in scope; constructs it rejects with a diagnostic (`E0001` to `E0015`) are by design.

## Supported versions

The project is pre-1.0. Only the latest tagged release and `main` receive fixes.
