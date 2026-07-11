# apiwatch Verify Local Design

## Goal

Add a local, CI-friendly command that verifies one OpenAPI contract against one named v1 `api.lock` entry.

## Command

```bash
apiwatch verify <OPENAPI> --name <NAME> --lock <PATH>
```

- `<OPENAPI>` is a local OpenAPI 3.x YAML or JSON file.
- `--name <NAME>` selects the API entry under `apis` in the lockfile. The trimmed name must not be empty.
- `--lock <PATH>` is a local v1 `api.lock` YAML file.

## Data Flow

1. Load and validate the v1 lockfile.
2. Select the trimmed API name from `apis`.
3. Require its `source` to be `openapi`.
4. Normalize the supplied OpenAPI file through the existing `openapi::load_contract` pipeline.
5. Compare the locked and current operation sets, where an operation is its uppercase HTTP method and normalized path.
6. Render the result through a focused output helper.

The lockfile module owns YAML deserialization, v1 validation, entry lookup, and operation-set comparison. The CLI router only orchestrates loading, comparison, rendering, and exit status.

## Results

Matching operation sets exit `0` and print:

```text
Verified users
```

Any drift exits `1`. Output is deterministic: all removed operations appear first, followed by all added operations; each group is ordered lexicographically by method and path.

```text
REMOVED GET /users
ADDED POST /users
```

Input, lockfile, validation, and OpenAPI parsing failures exit `2` through the existing top-level error handler. These include an empty API name, unreadable or invalid lockfile YAML, unsupported lockfile version, unsupported lockfile source, and an absent named API entry.

## Scope

This first verify slice compares only the normalized operation set available in lockfile version 1. It does not fetch remote contracts, read a config file, mutate a lockfile, compare schemas or authentication details, or verify multiple named entries in a single command.

## Testing

CLI integration coverage will prove:

- a matching current contract exits `0` with the success message;
- an added operation exits `1` with one `ADDED` line;
- a removed operation exits `1` with one `REMOVED` line;
- combined drift is rendered in the required deterministic order;
- an empty name, missing API entry, malformed lockfile, unsupported lockfile version, and unsupported lockfile source exit `2` with no successful verification output.

The existing format, Clippy, and full Rust test gates remain required. README and CHANGELOG will document the implemented command.
