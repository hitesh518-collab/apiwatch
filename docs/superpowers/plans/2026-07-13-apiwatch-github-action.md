# APIWatch GitHub Action Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish a reusable composite GitHub Action that builds `apiwatch` from its own source and runs one Verify invocation in a consumer repository workspace.

**Architecture:** A root `action.yml` installs stable Rust, builds the action checkout through `GITHUB_ACTION_PATH`, and invokes the resulting binary from the caller-selected directory. The repository CI gains an Ubuntu `action-smoke` job using `uses: ./`, while existing Rust CI remains unchanged.

**Tech Stack:** GitHub composite actions, GitHub Actions workflows, Bash, `dtolnay/rust-toolchain@stable`, Cargo.

## Constraints

- The action runs `apiwatch verify <OPENAPI_OR_URL> --name <NAME> --lock <PATH>`.
- Inputs are exactly `openapi` and `name` (required), plus `lock` (default `api.lock`) and `working-directory` (default `.`).
- Build the action source through `$GITHUB_ACTION_PATH/Cargo.toml`; run Verify in the consumer-selected working directory.
- Pass inputs through step environment variables and quote every shell expansion.
- Preserve the CLI's existing exit behavior: `0` for a match, `1` for detected drift, and `2` for operational or validation errors.
- This initial slice targets Ubuntu GitHub-hosted runners.
- Do not add outputs, caching, release-binary downloads, authentication, custom headers, configuration files, JSON, or SARIF support.
- Keep implementation notes in ignored `implementation-log/` files.

## Task 1: Add the Composite Action and CI Smoke Coverage

**Files:**
- Create: `action.yml`
- Modify: `.github/workflows/ci.yml`

- [ ] Add an `action-smoke` job to `.github/workflows/ci.yml` that runs on `ubuntu-latest`, checks out the repository, and invokes the local action with `uses: ./`.

  ```yaml
  action-smoke:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4
      - uses: ./
        with:
          openapi: testdata/openapi/verify_matching.yaml
          name: users
          lock: testdata/lock/verify_users.lock
  ```

  Keep the existing `rust` job unchanged. Do not commit a workflow that invokes the local action before `action.yml` exists.

- [ ] Create root `action.yml` with the following complete composite-action metadata:

  ```yaml
  name: apiwatch verify
  description: Verify a local or live OpenAPI contract against a named api.lock entry.

  inputs:
    openapi:
      description: Local OpenAPI YAML/JSON path or HTTP(S) URL to verify.
      required: true
    name:
      description: Named api.lock entry to verify.
      required: true
    lock:
      description: api.lock path relative to the working directory.
      required: false
      default: api.lock
    working-directory:
      description: Consumer repository directory in which Verify runs.
      required: false
      default: .

  runs:
    using: composite
    steps:
      - name: Install stable Rust
        uses: dtolnay/rust-toolchain@stable
      - name: Build apiwatch
        shell: bash
        env:
          ACTION_PATH: ${{ github.action_path }}
        run: cargo build --release --manifest-path "$ACTION_PATH/Cargo.toml"
      - name: Verify API contract
        shell: bash
        working-directory: ${{ inputs.working-directory }}
        env:
          ACTION_PATH: ${{ github.action_path }}
          OPENAPI: ${{ inputs.openapi }}
          API_NAME: ${{ inputs.name }}
          LOCK: ${{ inputs.lock }}
        run: '"$ACTION_PATH/target/release/apiwatch" verify "$OPENAPI" --name "$API_NAME" --lock "$LOCK"'
  ```

  The runner toolchain step intentionally matches the repository's existing Rust CI setup. Build and executable paths refer to the action checkout, while OpenAPI and lock-file paths remain relative to the caller's working directory.

- [ ] Run the local Rust quality gate:

  ```powershell
  cargo fmt --all -- --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
  git diff --check
  ```

- [ ] Commit the implementation with:

  ```text
  Add verify GitHub Action
  ```

  After pushing, confirm the `action-smoke` job runs successfully on GitHub Actions.

## Task 2: Document the Consumer Contract and Record the Work

**Files:**
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Create (ignored): `implementation-log/2026-07-13-apiwatch-github-action.md`

- [ ] Add a `## GitHub Action` section near the existing CLI introduction in `README.md`:

  ~~~markdown
  ## GitHub Action

  Use the reusable action from an Ubuntu workflow after checking out the consumer repository:

  ```yaml
  steps:
    - uses: actions/checkout@v4
    - uses: hitesh518-collab/apiwatch@<commit-sha>
      with:
        openapi: https://api.example.com/openapi.yaml
        name: users
        lock: api.lock
  ```

  Pin the action to a commit SHA or release tag. The action builds `apiwatch` from source with Cargo, propagates Verify's `0`/`1`/`2` exit codes, and supports the `working-directory` input. It does not provide caching, action outputs, authentication, custom headers, or configuration files.
  ~~~

  Preserve the repository's Markdown style and surrounding CLI documentation.

- [ ] Add this unreleased changelog entry:

  ```markdown
  - Reusable `apiwatch verify` composite GitHub Action that builds from source and propagates Verify exit codes.
  ```

- [ ] Write the ignored implementation log with the interface, Ubuntu-only source-build decision, smoke coverage, local verification, and deliberately deferred features.

- [ ] Re-run the local quality gate and inspect the repository state:

  ```powershell
  cargo fmt --all -- --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
  git diff --check
  git status --short --branch
  ```

- [ ] Inspect the completed GitHub Actions run, including `action-smoke`, after pushing.

- [ ] Commit the documentation with:

  ```text
  Document verify GitHub Action
  ```

## Final Verification

- [ ] Confirm the composite action's four inputs exactly match the documented contract.
- [ ] Confirm all caller-controlled values flow through environment variables and remain quoted in the Bash command.
- [ ] Confirm the action builds under the action directory but verifies under the consumer directory.
- [ ] Confirm the local CI quality gate passes and the GitHub `action-smoke` job is green.
- [ ] Confirm only intended tracked files are committed and the implementation log remains ignored.
