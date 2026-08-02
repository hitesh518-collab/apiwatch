# APIWatch Phase 1 Lock-Size Report

- Report schema: 1
- APIWatch: 0.9.0
- Ceiling: 5242880 bytes

| Corpus | Commit | Source bytes | Status | Operations | Expanded YAML | Canonical JSON | Deduplicated YAML |
|---|---|---:|---|---:|---:|---:|---:|
| github | `5c88ff6bc3c36a12ccd69b8e0fee479b7202188a` | 12816309 | passing | 1209 | 39730491 (over) | 13885612 (over) | 2327580 (fits) |
| asana | `56796a67a3c093eedf55fd9682357957a2ebfd85` | 3066750 | passing | 249 | 10846401 (over) | 3394194 (fits) | 806691 (fits) |
| box | `f28eec5d49b9597d7df82f3a0c75bd92478b699a` | 1765788 | passing | 296 | 2082776 (fits) | 1044360 (fits) | 485332 (fits) |
| digitalocean | `7667351a0c8a1a526343160e1778cb5e97b2c9da` | 110982 | known_failing | — | — | — | — |

Expected `digitalocean` failure: `missing field `responses``

| stripe | `86b6ae4db114ff06968dcc191ff4a898e9b5db7c` | 7866866 | known_failing | — | — | — | — |

Expected `stripe` failure: `circular schema reference detected: #/components/schemas/file`

| paystack | `c35994da6dddf521794ec7f8b730a1efda10c565` | 127804 | known_failing | — | — | — | — |

Expected `paystack` failure: `unsupported schema reference: #/paths/~1transaction~1initialize/post/requestBody/content/application~1json/schema`

| deutsche-bahn | `4d66b23dc5948016b50e79b944a0b084c7000da7` | 8279 | known_failing | — | — | — | — |

Expected `deutsche-bahn` failure: `failed to parse cleaned OpenAPI YAML`

| mercadopago | `ab4604b671b78c015f66fab1a59c95ed8fe95275` | 235336 | passing | 142 | 2360354 (fits) | 1125700 (fits) | 295669 (fits) |
| line | `de8bd9e2872ed9d9800e23b86136695fcd370a0c` | 183329 | passing | 73 | 416921 (fits) | 206401 (fits) | 92560 (fits) |
| humanitas-fhir | `ff0ce058595ac0ade0d64e0737dffb858517a97b` | 68679 | passing | 3 | 85443 (fits) | 40815 (fits) | 34221 (fits) |

- Privacy sentinels: passed across 3 candidates
- Recommendation: `deduplicated_yaml`
