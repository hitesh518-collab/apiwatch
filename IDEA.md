# apiwatch Idea

`apiwatch` prevents third-party API changes from silently breaking applications.

Most API monitoring answers whether an API is up. `apiwatch` focuses on whether the API contract still matches what a repository expects.

The first version starts with OpenAPI because it gives structured contracts:

1. Import two OpenAPI 3.x files.
2. Normalize them into contract snapshots.
3. Compare the snapshots.
4. Report breaking, warning, and non-breaking changes.

Longer term, `apiwatch` can support lockfiles, CI verification, remote specs, runtime JSON samples, and language scanners.
