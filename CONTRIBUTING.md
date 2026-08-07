# Contributing

Thanks for helping build `apiwatch` — a tool that catches third-party API
changes before they break your app.

## Local Development

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo +1.88.0 check --workspace --exclude apiwatch-lock-size-report
```

## Example Demos

```bash
cargo build --release
python scripts/check_examples.py --binary target/release/apiwatch
```

## Project Direction

APIWatch locks, diffs, and verifies external API contracts — both declared
(OpenAPI) and observed (JSON/HAR). The product validates that the APIs your
code depends on haven't changed in breaking ways.

See [ROADMAP.md](ROADMAP.md) for the delivery sequence and phase exit criteria.

## Pull Requests

- Keep changes focused. One coherent change per PR.
- Add regression fixtures for behavior fixes.
- Run `cargo fmt`, `cargo clippy`, and `cargo test` before pushing.
- Update documentation when changing CLI behavior or rule classification.
- Never weaken a test merely to make it pass.
- Preserve deterministic output, value-free locks, and read-only verify.

## Issue Reporting

- For bugs, include a minimal reproduction and the full CLI output.
- For feature requests, describe the API breakage scenario the feature would
  protect against, not just the feature mechanics.
- Check [docs/compat-corpus.md](docs/compat-corpus.md) before reporting
  parser/format compatibility issues.

## Privacy

Observed locks must never retain captured scalar values, credentials,
authorization headers, cookies, dynamic map keys, or user secrets. All
contributions must preserve this invariant.
