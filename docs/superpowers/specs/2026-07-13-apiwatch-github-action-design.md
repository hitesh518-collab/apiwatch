# APIWatch GitHub Action Design

## Goal

Ship the first reusable GitHub Action for `apiwatch verify`. It must build the action's own Rust source with Cargo, run one verification against a consumer repository's workspace, and preserve the CLI's existing exit behavior.

## Action Interface

The action lives at the repository root in `action.yml` and is invoked from another repository after checkout:

```yaml
- uses: actions/checkout@v4
- uses: hitesh518-collab/apiwatch@<pinned-ref>
  with:
    openapi: https://api.example.com/openapi.yaml
    name: users
    lock: api.lock
```

Inputs:

- `openapi` (required): local OpenAPI YAML/JSON path or HTTP/HTTPS URL.
- `name` (required): named v1 `api.lock` entry.
- `lock` (optional, default `api.lock`): lockfile path relative to the working directory.
- `working-directory` (optional, default `.`): consumer repository directory in which Verify runs.

The action exposes no outputs. It invokes `apiwatch verify` directly, so exit `0` remains a match, exit `1` remains drift and fails the workflow step, and exit `2` remains an input/fetch/lockfile error.

## Architecture

Use a root composite action rather than Docker or a JavaScript wrapper. Composite steps may invoke actions and shell commands, so the action:

1. Installs the stable Rust toolchain through the same `dtolnay/rust-toolchain@stable` convention already used by this repository's CI.
2. Builds the action's checked-out source in release mode with `cargo build --release --manifest-path "$GITHUB_ACTION_PATH/Cargo.toml"`.
3. Runs `"$GITHUB_ACTION_PATH/target/release/apiwatch" verify` in the caller-selected working directory.

Build commands always resolve from `GITHUB_ACTION_PATH`; Verify inputs always resolve from the consumer workflow directory. Input values are passed through step-scoped environment variables and quoted in Bash, rather than interpolated into shell source.

## Security And Failure Handling

The action does not accept tokens, headers, secrets, arbitrary shell fragments, or configuration files. It does not capture or reinterpret the `apiwatch` exit status. Cargo/build/action failures naturally fail the workflow step.

Documentation tells consumers to pin the action to an immutable commit SHA or release tag. This first slice does not cache Cargo artifacts or download release binaries.

## Verification

Extend the existing repository CI workflow with a dedicated Ubuntu action-smoke job:

1. Check out this repository.
2. Invoke `uses: ./` with `testdata/openapi/verify_matching.yaml`, `users`, and `testdata/lock/verify_users.lock`.
3. Require the action to build its own source and exit successfully.

The existing Rust CI job remains unchanged. Local verification checks action metadata structure and the normal Cargo formatting, Clippy, and test gates; GitHub Actions executes the end-to-end composite smoke job on push and pull request.

## Scope

This is the v0.5 CI foundation only: one reusable Verify composite action. JSON output, SARIF output, caching, release-binary distribution, setup-only actions, matrix runners, authentication, custom headers, and action outputs are intentionally deferred.
