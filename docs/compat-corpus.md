# APIWatch Compatibility Corpus

The compatibility corpus ensures APIWatch correctly normalizes real-world OpenAPI specifications and produces stable lock files. Every spec is pinned to an immutable GitHub commit with a SHA-256 hash.

## Passing Specs (14)

| Spec | Source | Operations | Stresses |
|------|--------|-----------|----------|
| github | `github/rest-api-description` | 1,209 | Very large JSON spec; massive schema nesting |
| asana | `Asana/openapi` | 249 | YAML spec with deeply nested schemas |
| box | `box/box-openapi` | 296 | JSON spec with moderate complexity |
| digitalocean | `digitalocean/openapi` | 0 usable after cleanup | Malformed path metadata with missing `responses` |
| mercadopago | `mercadopago/openapi` | 142 | YAML spec with i18n descriptions |
| line | `line/line-openapi` | 73 | Small YAML spec |
| humanitas-fhir | `copyleftdev/humanitas` | 3 | Tiny JSON spec; FHIR healthcare API |
| **petstore** | `openapitools/openapi-petstore` | 20 | Classic example spec; minimal baseline |
| **plaid** | `plaid/plaid-openapi` | 335 | Large YAML spec; financial API with complex schemas |
| **twilio** | `twilio/twilio-oai` | 197 | YAML spec with complex `$ref` and versioned APIs |
| **adyen** | `Adyen/adyen-openapi` | 28 | JSON with `anyOf`/`oneOf` discriminators; payment gateway |
| **kubernetes** | `kubernetes/kubernetes` | 248 | Massive expanded YAML (312 MB); Kubernetes core v1 API |
| **intercom** | `intercom/Intercom-OpenAPI` | 231 | YAML spec; customer messaging platform |
| **figma** | `figma/rest-api-spec` | 50 | YAML spec with duplicate wire-level authentication headers |

## Known Failing Specs (6)

| Spec | Source | Error |
|------|--------|-------|
| stripe | `stripe/openapi` | Schema expansion exceeds the resolution budget |
| paystack | `PaystackHQ/openapi` | Unsupported path-level `$ref` |
| deutsche-bahn | `APIs-guru/openapi-directory` | Swagger 2.0 spec fails YAML parsing |
| shopify | `allengrant/shopify_openapi` | Parameter name contains invalid characters |
| **slack** | `slackapi/slack-api-specs` | `ParameterSchemaOrContent` enum variant mismatch |
| **openai** | `openai/openai-openapi` | YAML parsing: i128 integer overflow in schema |

## Selection Rationale

Each spec was chosen to stress a different aspect of APIWatch's parser and normalizer:

- **petstore**: Minimal spec to verify basic parsing works.
- **digitalocean**: Malformed path metadata that is safely cleaned before normalization.
- **plaid**: Large YAML spec from a major fintech API; stresses schema resolution.
- **shopify**: Large e-commerce spec that retains an upstream malformed parameter name.
- **twilio**: Complex `$ref` structures with YAML-specific features.
- **adyen**: `anyOf`/`oneOf` discriminators common in payment gateways.
- **kubernetes**: Extremely large expanded YAML output (312 MB) despite modest source size; stresses memory.
- **intercom**: Medium complexity YAML from a popular customer platform.
- **figma**: Duplicate authentication wire identities that are safely deduplicated.

Known-failing specs document current parser limitations that may be addressed in future versions.
