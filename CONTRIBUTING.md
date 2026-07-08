# Contributing

Thanks for helping build `apiwatch`.

## Local Development

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Project Direction

The first milestone is a trustworthy OpenAPI semantic diff engine. Prefer small, well-tested rules over broad noisy detection.

## Pull Requests

- Keep changes focused.
- Add fixtures for diff behavior.
- Update docs when changing CLI behavior or rule classification.
