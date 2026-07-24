# APIWatch Phase 1 Lock-Size Report

- Report schema: 1
- APIWatch: 0.7.0
- Ceiling: 5242880 bytes

| Corpus | Commit | Source bytes | Status | Operations | Expanded YAML | Canonical JSON | Deduplicated YAML |
|---|---|---:|---|---:|---:|---:|---:|
| github | `5c88ff6bc3c36a12ccd69b8e0fee479b7202188a` | 12816309 | passing | 1209 | 39730491 (over) | 13885612 (over) | 2327580 (fits) |
| asana | `56796a67a3c093eedf55fd9682357957a2ebfd85` | 3066750 | passing | 249 | 10846401 (over) | 3394194 (fits) | 806691 (fits) |
| box | `f28eec5d49b9597d7df82f3a0c75bd92478b699a` | 1765788 | passing | 296 | 2082776 (fits) | 1044360 (fits) | 485332 (fits) |
| stripe | `86b6ae4db114ff06968dcc191ff4a898e9b5db7c` | 7866866 | known_failing | — | — | — | — |

Expected `stripe` failure: `circular schema reference detected: #/components/schemas/file`

| digitalocean | `7667351a0c8a1a526343160e1778cb5e97b2c9da` | 110982 | known_failing | — | — | — | — |

Expected `digitalocean` failure: `tags[0].description: invalid type: map, expected a string`


- Privacy sentinels: passed across 3 candidates
- Recommendation: `deduplicated_yaml`
