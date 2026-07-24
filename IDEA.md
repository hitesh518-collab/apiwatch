# apiwatch Idea

`apiwatch` prevents changes in third-party APIs from silently breaking the
applications that depend on them.

Uptime monitoring answers whether an API responds. It does not answer whether
the response, authentication, parameters, status codes, or schemas still match
what a consumer expects.

Provider specifications are useful evidence, but external APIs often publish
no OpenAPI document, publish an incomplete one, or change behavior without
updating it. APIWatch therefore supports two contract sources:

1. **Declared contracts** normalize a usable OpenAPI document.
2. **Observed contracts** infer response structure from explicit JSON samples.

Both produce deterministic, reviewable evidence that can be locked in a
repository and verified in CI. Their confidence is different: a declaration
states what a provider claims, while an observation proves only the structure
that has been sampled. APIWatch keeps that provenance visible.

Observed contracts store shape, paths, and type information—never captured
scalar values, credentials, or dynamic map keys. This structure-only boundary
is a core privacy property.

The product is designed for a global, cross-industry audience. The
correctness-first delivery sequence and explicit scope boundaries live in
[ROADMAP.md](ROADMAP.md).
