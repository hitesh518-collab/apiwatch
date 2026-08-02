# APIWatch Phase 2 v4 Lock-Size Report

- Report schema: 1
- APIWatch: 0.9.0
- Ceiling: 5242880 bytes

| Corpus | Commit | SHA-256 | Source bytes | Status | Operations | v4 contract payload |
|---|---|---|---:|---|---:|---:|
| github | `5c88ff6bc3c36a12ccd69b8e0fee479b7202188a` | `17d0cf71ec30e78bd1dc27085be8371504b98e9a9326cf2a0802ab88c37fbfb5` | 12816309 | passing | 1209 | 2569165 (fits) |
| asana | `56796a67a3c093eedf55fd9682357957a2ebfd85` | `cb3b90f4e0af56035eab0c648974f625b942a28a7144aa6c2326e38ca0bb3d56` | 3066750 | passing | 249 | 945824 (fits) |
| box | `f28eec5d49b9597d7df82f3a0c75bd92478b699a` | `0db1ffa51e52b9f1cb779bc4a37f200ac5f978630cab5178141687b2fed24e7a` | 1765788 | passing | 296 | 589237 (fits) |
| digitalocean | `7667351a0c8a1a526343160e1778cb5e97b2c9da` | `cda2db55fb97ceef551a3e35682dca49ad331b486f88f712f7c93f4ba05eefbc` | 110982 | known_failing | — | — |

Expected `digitalocean` failure: `missing field `responses``

| stripe | `86b6ae4db114ff06968dcc191ff4a898e9b5db7c` | `e24a26de4188fd64dec4c043d5d3726277fdcb07556a493ea481c305b0a223d8` | 7866866 | known_failing | — | — |

Expected `stripe` failure: `circular schema reference detected: #/components/schemas/file`

| paystack | `c35994da6dddf521794ec7f8b730a1efda10c565` | `253ed891f09da6f477c96d1b9173dd1c76e99f665dbf906611fd6f8560f87809` | 127804 | known_failing | — | — |

Expected `paystack` failure: `unsupported schema reference: #/paths/~1transaction~1initialize/post/requestBody/content/application~1json/schema`

| deutsche-bahn | `4d66b23dc5948016b50e79b944a0b084c7000da7` | `524483f96b1e91d78d75f6d8961831620b5cf2e718db24402772ca2e0a21cba3` | 8279 | known_failing | — | — |

Expected `deutsche-bahn` failure: `failed to parse cleaned OpenAPI YAML`

| mercadopago | `ab4604b671b78c015f66fab1a59c95ed8fe95275` | `27122365615dfd4a28dea42b717283c41797b5dd3f2cc73a872855507c1b9437` | 235336 | passing | 142 | 299749 (fits) |
| line | `de8bd9e2872ed9d9800e23b86136695fcd370a0c` | `0227978ce1b3133e20da034fc33a9241000619ae4fea2fda7b61983abf79577a` | 183329 | passing | 73 | 95192 (fits) |
| humanitas-fhir | `ff0ce058595ac0ade0d64e0737dffb858517a97b` | `fa674d561cca3dc504ed0672a2c38bda1b94e579cd2f5adbcbe0d0f6ab0593ec` | 68679 | passing | 3 | 33787 (fits) |
