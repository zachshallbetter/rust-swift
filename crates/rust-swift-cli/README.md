# rust-swift-cli

Unified command-line interface for the `rust-swift` compiler, verification suites, and developer tools.

## Subcommands

- **`rust-swift compile <input.rs> [OPTIONS]`**: Compiles admitted Rust files into standalone Swift source files or complete Swift Package Manager packages.
- **`rust-swift proof <input.rs> [-o receipt.json]`**: Evaluates formal invariants and emits a cryptographically signed `proof-receipt.json` dossier.
- **`rust-swift sync [--input-dir <dir>] [--output-dir <dir>] [--watch] [--interval-ms <ms>]`**: Incremental CSG delta synchronizer and live watcher daemon.
- **`rust-swift diff-test [OPTIONS]`**: Runs the 16-suite differential qualification harness comparing `rustc` and `swiftc` outputs.
- **`rust-swift eval [--mutations] [--adversarial]`**: Runs failure injection mutations and aggressive adversarial stress tests.
- **`rust-swift bench [OPTIONS]`**: Runs compiler throughput benchmarks and microsecond runtime parity benchmarks.
- **`rust-swift info`**: Displays compiler metadata, version, toolchain baselines, and architectural attribution.
