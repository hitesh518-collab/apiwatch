# APIWatch Phase 2 v4 Lock-Size Report

- Report schema: 1
- APIWatch: 1.0.3 (f52a5b4)
- Ceiling: 5242880 bytes

| Corpus | Commit | SHA-256 | Source bytes | Status | Operations | v4 contract payload |
|---|---|---|---:|---|---:|---:|
| github | `5c88ff6bc3c36a12ccd69b8e0fee479b7202188a` | `17d0cf71ec30e78bd1dc27085be8371504b98e9a9326cf2a0802ab88c37fbfb5` | 12816309 | passing | 1209 | 2569165 (fits) |
| asana | `56796a67a3c093eedf55fd9682357957a2ebfd85` | `cb3b90f4e0af56035eab0c648974f625b942a28a7144aa6c2326e38ca0bb3d56` | 3066750 | passing | 249 | 945824 (fits) |
| box | `f28eec5d49b9597d7df82f3a0c75bd92478b699a` | `0db1ffa51e52b9f1cb779bc4a37f200ac5f978630cab5178141687b2fed24e7a` | 1765788 | passing | 296 | 589237 (fits) |
| stripe | `86b6ae4db114ff06968dcc191ff4a898e9b5db7c` | `e24a26de4188fd64dec4c043d5d3726277fdcb07556a493ea481c305b0a223d8` | 7866866 | known_failing | — | — |

Expected `stripe` failure: `schema expansion exceeded resolution budget`

| digitalocean | `7667351a0c8a1a526343160e1778cb5e97b2c9da` | `cda2db55fb97ceef551a3e35682dca49ad331b486f88f712f7c93f4ba05eefbc` | 110982 | passing | 0 | 27 (fits) |
| paystack | `c35994da6dddf521794ec7f8b730a1efda10c565` | `253ed891f09da6f477c96d1b9173dd1c76e99f665dbf906611fd6f8560f87809` | 127804 | known_failing | — | — |

Expected `paystack` failure: `unsupported schema reference: #/paths/~1transaction~1initialize/post/requestBody/content/application~1json/schema`

| deutsche-bahn | `4d66b23dc5948016b50e79b944a0b084c7000da7` | `524483f96b1e91d78d75f6d8961831620b5cf2e718db24402772ca2e0a21cba3` | 8279 | known_failing | — | — |

Expected `deutsche-bahn` failure: `failed to parse cleaned OpenAPI YAML`

| mercadopago | `ab4604b671b78c015f66fab1a59c95ed8fe95275` | `27122365615dfd4a28dea42b717283c41797b5dd3f2cc73a872855507c1b9437` | 235336 | passing | 142 | 299749 (fits) |
| line | `de8bd9e2872ed9d9800e23b86136695fcd370a0c` | `0227978ce1b3133e20da034fc33a9241000619ae4fea2fda7b61983abf79577a` | 183329 | passing | 73 | 95192 (fits) |
| humanitas-fhir | `ff0ce058595ac0ade0d64e0737dffb858517a97b` | `fa674d561cca3dc504ed0672a2c38bda1b94e579cd2f5adbcbe0d0f6ab0593ec` | 68679 | passing | 3 | 33787 (fits) |
| petstore | `19662bbe120189f6740836961f9ee7e0d3effcbc` | `d8af387ab4d079976e4734833584e65e6b2d87cda6ed24067c0ab6342d97f952` | 20436 | passing | 20 | 19293 (fits) |
| plaid | `bae08e213428e0260c3ac53d3f73b40b4d6ec113` | `64c4514ea59b82526a1206024684ebc6e91cf2a3d73276772a0f818c13d828d9` | 3041896 | passing | 335 | 1602827 (fits) |
| shopify | `030bf4edf884d07f0eb4a96c57d15f4f6f43ed89` | `cbf89d87833a1c3e1dca35059adcb1b25959258daeea402d0ad4681d6257d841` | 3122904 | known_failing | — | — |

Expected `shopify` failure: `parameter name contains invalid characters`

| twilio | `bb6288e9f540d2d63540bbaadf6b73fd262c2df3` | `768720d76b47f1b75896c8fe092fac59b886bd3370278b27b6962b83cacae12e` | 1496453 | passing | 197 | 521155 (fits) |
| adyen | `425d65d12163ebd4fe6fa6d2f859075817c81cba` | `1498fbccd44b48a97328907bd1cbada2f6db9a92ae43ba4d0c68d45908356dc8` | 937956 | passing | 28 | 389292 (fits) |
| kubernetes | `0f914a00561edadef5b6ef19c443cab232c39527` | `d09ab224a98fb9c0e7fd128b6f395f66b23d43b6635272109dcaaafc4a3dd9c9` | 2135483 | passing | 248 | 776349 (fits) |
| intercom | `d04d4798a68a8b77aabdf7d117d71865b72ed571` | `9bd5a638adefc6a81f18a25905151889a64f8a16378decc03ed9cc4e75c44a2e` | 1317483 | passing | 231 | 681063 (fits) |
| slack | `dfea73e06d146c368d7f94b52ac90796dc4e27e1` | `742a5c977180a829df8767cf57bc417d99b3713583aee83741efb9c08ca731e7` | 1237332 | known_failing | — | — |

Expected `slack` failure: `no variant of enum ParameterSchemaOrContent found in flattened data`

| figma | `e854a2c2dff3ff8cb743e9a06575fbbf225faa33` | `06c46b4a12731d12ea51efaf01a946bb997d8103861cfa192e3e6df40883eba0` | 388252 | passing | 50 | 548202 (fits) |
| openai | `117ce5680e4269f6656a4fd70d28f9755630d938` | `e9cfcc3a325093a640af9e3b289dd4fa69f0c03e3a9af425fda47a5fe1238361` | 2845483 | known_failing | — | — |

Expected `openai` failure: `failed to parse OpenAPI YAML`
