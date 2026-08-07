# Dogfood: GitHub REST API

APIWatch dogfoods itself by monitoring the GitHub REST API spec for changes
that could break the project's compatibility corpus.

## Provider

**GitHub REST API** — the public GitHub API spec published at
`github/rest-api-description`. This is an external dependency for APIWatch's
compatibility corpus tests.

## Scope

Three pull request operations, chosen as a narrow, meaningful subset:

| Operation | Description |
|-----------|-------------|
| `GET /repos/{owner}/{repo}/pulls` | List pull requests |
| `GET /repos/{owner}/{repo}/pulls/{pull_number}` | Get a pull request |
| `GET /repos/{owner}/{repo}/pulls/{pull_number}/comments` | List review comments |

## Lock

`api.lock` is a v4 declared lock containing the normalized contract for these
three operations. It was created from the pinned corpus spec (`compat/specs.json`)
and verified against the latest `main` branch spec.

Lock size: 88 KB (well under the 5 MB default ceiling).

## CI

`.github/workflows/dogfood.yml` runs weekly (Monday 06:00 UTC) and on every
push that touches `ci/dogfood/` or the dogfood workflow. It fetches the latest
GitHub API spec and verifies it against the committed lock.

## Intent

If GitHub changes the response shape of pull request operations, the dogfood
CI fails and the team can:
1. Review the breaking change in the CI output
2. Decide whether it affects the corpus tests
3. Update the lock intentionally via `apiwatch lock --update`

## Updating the lock

```bash
apiwatch lock .compat-cache/github.json --name github-pull-requests \
  --output ci/dogfood/api.lock --update \
  --include-operation "GET /repos/{owner}/{repo}/pulls" \
  --include-operation "GET /repos/{owner}/{repo}/pulls/{pull_number}" \
  --include-operation "GET /repos/{owner}/{repo}/pulls/{pull_number}/comments"
```

The update is explicit, reviewable in git, and never happens automatically.
