# apiwatch Lock Single-File Design

## Purpose

The first `apiwatch lock` slice creates a deterministic `api.lock` file from one local OpenAPI 3.x YAML or JSON file. This gives the project a concrete lockfile command without introducing config files, remote fetching, or multi-API orchestration.

## Approved CLI

```bash
apiwatch lock <OPENAPI> --name <NAME> --output <PATH>
```

- `<OPENAPI>` is a local OpenAPI 3.x YAML or JSON file.
- `--name <NAME>` is the key used under `apis` in the lockfile. Empty names are invalid.
- `--output <PATH>` is the lockfile path to write. The command overwrites an existing file, but the parent directory must already exist.
- Success exits with code `0` and prints `Wrote <PATH>`.
- Input, parse, normalization, validation, and write failures exit with code `2`, matching current CLI error behavior.

## Lockfile Shape

The first lockfile version stores normalized operation metadata only:

```yaml
version: 1
apis:
  users:
    source: openapi
    operations:
      - method: GET
        path: /users
      - method: POST
        path: /users
```

Operations are sorted by the existing normalized `OperationKey` ordering, so output is deterministic. The first slice intentionally does not store raw OpenAPI fragments, secrets, headers, examples, request or response schemas, or hashes. Those can be added in later lockfile-version-compatible steps.

## Architecture

Add a new `lockfile` module that converts `ApiContract` into serializable lockfile structs and renders YAML. The module depends on the normalized contract model, not raw OpenAPI, so future input sources can reuse it.

Add a `Lock` CLI subcommand in `src/cli.rs`, then route it from `src/main.rs`: load the OpenAPI contract through `openapi::load_contract`, convert it with `lockfile::from_contract(name, &contract)`, serialize it, write it to `--output`, and print the success line.

## Data Flow

1. Clap parses `apiwatch lock <OPENAPI> --name <NAME> --output <PATH>`.
2. `openapi::load_contract` reads and normalizes the OpenAPI file.
3. The lockfile module validates `NAME`, extracts sorted operations, and builds `ApiLock`.
4. The lockfile module serializes deterministic YAML.
5. `main` writes the YAML to `PATH` and prints `Wrote <PATH>`.

## Error Handling

- Empty `--name` returns `api name cannot be empty`.
- Unsupported OpenAPI versions keep the current `unsupported OpenAPI version ...` error.
- Malformed YAML/JSON keeps the current parse context.
- Unsupported or circular references keep the existing normalization errors.
- File write failures include `failed to write lockfile <PATH>`.

## Testing

Add CLI integration tests for:

- A successful lockfile write from an existing fixture.
- Deterministic operation ordering in the generated YAML.
- Empty `--name` exits with code `2`.
- Invalid OpenAPI input exits with code `2`.

The full verification gate remains:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Non-Goals

- No `apiwatch lock --config apiwatch.yaml`.
- No remote URL fetching.
- No schema hashing.
- No multi-API merge behavior.
- No compatibility check between an existing lockfile and a new OpenAPI file.
