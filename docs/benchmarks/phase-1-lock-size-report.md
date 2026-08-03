# APIWatch Phase 1 Lock-Size Report

- Report schema: 1
- APIWatch: 1.0.2 (b3629e2)
- Ceiling: 5242880 bytes

| Corpus | Commit | Source bytes | Status | Operations | Expanded YAML | Canonical JSON | Deduplicated YAML |
|---|---|---:|---|---:|---:|---:|---:|
| github | `5c88ff6bc3c36a12ccd69b8e0fee479b7202188a` | 12816309 | passing | 1209 | 39730491 (over) | 13885612 (over) | 2327580 (fits) |
| asana | `56796a67a3c093eedf55fd9682357957a2ebfd85` | 3066750 | passing | 249 | 10846401 (over) | 3394194 (fits) | 806691 (fits) |
| box | `f28eec5d49b9597d7df82f3a0c75bd92478b699a` | 1765788 | passing | 296 | 2082776 (fits) | 1044360 (fits) | 485332 (fits) |
| stripe | `86b6ae4db114ff06968dcc191ff4a898e9b5db7c` | 7866866 | known_failing | — | — | — | — |

Expected `stripe` failure: `schema expansion exceeded resolution budget`

| digitalocean | `7667351a0c8a1a526343160e1778cb5e97b2c9da` | 110982 | passing | 0 | 15 (fits) | 18 (fits) | 27 (fits) |
| paystack | `c35994da6dddf521794ec7f8b730a1efda10c565` | 127804 | known_failing | — | — | — | — |

Expected `paystack` failure: `unsupported schema reference: #/paths/~1transaction~1initialize/post/requestBody/content/application~1json/schema`

| deutsche-bahn | `4d66b23dc5948016b50e79b944a0b084c7000da7` | 8279 | known_failing | — | — | — | — |

Expected `deutsche-bahn` failure: `failed to parse cleaned OpenAPI YAML`

| mercadopago | `ab4604b671b78c015f66fab1a59c95ed8fe95275` | 235336 | passing | 142 | 2360354 (fits) | 1125700 (fits) | 295669 (fits) |
| line | `de8bd9e2872ed9d9800e23b86136695fcd370a0c` | 183329 | passing | 73 | 416921 (fits) | 206401 (fits) | 92560 (fits) |
| humanitas-fhir | `ff0ce058595ac0ade0d64e0737dffb858517a97b` | 68679 | passing | 3 | 85443 (fits) | 40815 (fits) | 34221 (fits) |
| petstore | `19662bbe120189f6740836961f9ee7e0d3effcbc` | 20436 | passing | 20 | 86315 (fits) | 47963 (fits) | 18628 (fits) |
| plaid | `bae08e213428e0260c3ac53d3f73b40b4d6ec113` | 3041896 | passing | 335 | 9655471 (over) | 3597553 (fits) | 1607557 (fits) |
| shopify | `030bf4edf884d07f0eb4a96c57d15f4f6f43ed89` | 3122904 | known_failing | — | — | — | — |

Expected `shopify` failure: `parameter name contains invalid characters`

| twilio | `bb6288e9f540d2d63540bbaadf6b73fd262c2df3` | 1496453 | passing | 197 | 1921795 (fits) | 1061970 (fits) | 508457 (fits) |
| adyen | `425d65d12163ebd4fe6fa6d2f859075817c81cba` | 937956 | passing | 28 | 2588380 (fits) | 1167851 (fits) | 393822 (fits) |
| kubernetes | `0f914a00561edadef5b6ef19c443cab232c39527` | 2135483 | passing | 248 | 312536737 (over) | 91147295 (over) | 761018 (fits) |
| intercom | `d04d4798a68a8b77aabdf7d117d71865b72ed571` | 1317483 | known_failing | — | — | — | — |

Expected `intercom` failure: `path template placeholder job_identifier is not bound to a path parameter`

| slack | `dfea73e06d146c368d7f94b52ac90796dc4e27e1` | 1237332 | known_failing | — | — | — | — |

Expected `slack` failure: `no variant of enum ParameterSchemaOrContent found in flattened data`

| figma | `e854a2c2dff3ff8cb743e9a06575fbbf225faa33` | 388252 | known_failing | — | — | — | — |

Expected `figma` failure: `duplicate authentication identity`

| openai | `117ce5680e4269f6656a4fd70d28f9755630d938` | 2845483 | known_failing | — | — | — | — |

Expected `openai` failure: `failed to parse OpenAPI YAML`


- Privacy sentinels: passed across 3 candidates
- Recommendation: `deduplicated_yaml`
