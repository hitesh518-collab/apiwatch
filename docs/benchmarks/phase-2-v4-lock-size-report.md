# APIWatch Phase 2 v4 Lock-Size Report

- Report schema: 1
- APIWatch: 0.9.0
- Ceiling: 5242880 bytes

| Corpus | Commit | SHA-256 | Source bytes | Status | Operations | v4 contract payload |
|---|---|---|---:|---|---:|---:|
| github | `5c88ff6bc3c36a12ccd69b8e0fee479b7202188a` | `17d0cf71ec30e78bd1dc27085be8371504b98e9a9326cf2a0802ab88c37fbfb5` | 12816309 | passing | 1209 | 2569165 (fits) |
| asana | `56796a67a3c093eedf55fd9682357957a2ebfd85` | `cb3b90f4e0af56035eab0c648974f625b942a28a7144aa6c2326e38ca0bb3d56` | 3066750 | passing | 249 | 946072 (fits) |
| box | `f28eec5d49b9597d7df82f3a0c75bd92478b699a` | `0db1ffa51e52b9f1cb779bc4a37f200ac5f978630cab5178141687b2fed24e7a` | 1765788 | passing | 296 | 589237 (fits) |
| stripe | `86b6ae4db114ff06968dcc191ff4a898e9b5db7c` | `e24a26de4188fd64dec4c043d5d3726277fdcb07556a493ea481c305b0a223d8` | 7866866 | known_failing | — | — |

Expected `stripe` failure: `circular schema reference detected: #/components/schemas/file`

| digitalocean | `7667351a0c8a1a526343160e1778cb5e97b2c9da` | `cda2db55fb97ceef551a3e35682dca49ad331b486f88f712f7c93f4ba05eefbc` | 110982 | known_failing | — | — |

Expected `digitalocean` failure: `tags[0].description: invalid type: map, expected a string`
