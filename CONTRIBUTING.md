# Contributing to rust-swift

`rust-swift` is a source-to-source compiler from a constrained Rust subset to standalone Swift. This guide covers setup, the checks a change must pass, and the conventions the repository uses.

Agent and bootstrap operating rules live in [`AGENTS.md`](AGENTS.md) and the compiler-specific contract in [`.agents/AGENTS.md`](.agents/AGENTS.md). Read them before changing compiler behavior. They apply to people and agents alike.

## Setup

You need a Rust toolchain (stable, with `clippy` and `rustfmt`) and Swift 5.9 or later for the Swift-side qualification suites. CI runs on macOS.

```sh
git clone https://github.com/zachshallbetter/rust-swift.git
cd rust-swift
cargo build --workspace
```

## Checks

CI runs these in this order (`.github/workflows/ci.yml`). Run the first four before opening a pull request, and the rest when you change compiler behavior:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
cargo run -p rust-swift-cli -- diff-test    # differential qualification suites
cargo run -p rust-swift-cli -- eval         # failure injection and mutation gates
cargo run -p rust-swift-cli -- bench        # differential performance benchmarks
python3 .agents/bootstrap/scripts/validate-bootstrap.py
```

## Conventions

- **Crate ownership.** Parsing and admission live in `rust-swift-frontend`, the IR in `rust-swift-ir`, Swift generation in `rust-swift-codegen`, qualification in `rust-swift-qualification`, the MCP server in `rust-swift-mcp`, and the CLI in `rust-swift-cli`. Downstream code depends on the `rust-swift` facade crate.
- **Reject, do not guess.** A Rust construct that is not admitted must produce a diagnostic (`E0001` to `E0015`), not partial output. Adding a construct means adding its lowering contract and a differential qualification case.
- **Deterministic output.** The same Rust input must produce byte-identical Swift.
- **Negative results are kept.** Log a disproven approach in `records/negative-results.jsonl`; do not delete entries.
- **Claim tiers.** Documented, implemented, tested and empirically validated are different claims. Do not state a higher tier than the evidence supports.
- **Commits.** Conventional style with a scope where it helps: `feat(codegen): ...`, `fix(frontend): ...`, `docs: ...`, `ci: ...`, `chore(bootstrap): ...`.
- **Changelog.** Note user-visible changes under `## [Unreleased]` in `CHANGELOG.md`.

## Pull requests

Describe what changed and why, list the checks you ran, and keep the diff to one concern. Use the pull request template.

Report vulnerabilities through [SECURITY.md](SECURITY.md), not a public issue.
