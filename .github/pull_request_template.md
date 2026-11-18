## What and why

<!-- What does this change do, and what problem does it solve? A conventional-commit title is appreciated, e.g. `feat(codegen): lower X`. -->

## How it was verified

<!-- Tick what you ran. -->

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `cargo run -p rust-swift-cli -- diff-test` (compiler behavior changes)
- [ ] `cargo run -p rust-swift-cli -- eval` (compiler behavior changes)
- [ ] `python3 .agents/bootstrap/scripts/validate-bootstrap.py`

## Notes

<!-- Trade-offs, follow-ups, or a CHANGELOG entry under [Unreleased]. -->
