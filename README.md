# spell-lab

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.90%2B-orange.svg)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-19-61dafb.svg)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.7-blue.svg)](https://www.typescriptlang.org)

High-performance, confidence-ranked spellcheck and correction REST API built in **Rust** with an interactive **React** web testing interface.

Features native per-user isolated custom dictionaries, multi-layered spellcheck algorithmic pipelines (Bloom Filter, Compact Prefix Trie, BK-Tree metric space, Damerau-Levenshtein transposition awareness, and QWERTY proximity weighting), with sub-millisecond query responses.

![Spell-Lab Web Interface](docs/screenshot.png)

---

## Table of Contents

- [Key Highlights](#key-highlights)
- [Architecture & Design](#architecture--design)
- [Real-World API Usage & Parameters](#real-world-api-usage--parameters)
  - [1. Authentication](#1-authentication)
  - [2. Multi-Token Spellcheck (`/api/v1/spell/check`)](#2-multi-token-spellcheck-apiv1spellcheck)
  - [3. Single-Word Suggestions (`/api/v1/spell/suggest`)](#3-single-word-suggestions-apiv1spellsuggest)
  - [4. Prefix Autocompletion (`/api/v1/spell/complete`)](#4-prefix-autocompletion-apiv1spellcomplete)
  - [5. One-Click Auto-Correction (`/api/v1/spell/correct`)](#5-one-click-auto-correction-apiv1spellcorrect)
  - [6. User Dictionary & Bulk Operations (`/api/v1/dictionary/*`)](#6-user-dictionary--bulk-operations-apiv1dictionary)
  - [7. Telemetry & Observability (`/api/v1/metrics`, `/api/v1/stats`)](#7-telemetry--observability-apiv1metrics-apiv1stats)
- [Deployment Guide](#deployment-guide)
  - [Native Systemd Service (Local)](#native-systemd-service-local)
  - [Docker Compose (Local or Production)](#docker-compose-local-or-production)
  - [Traefik Edge Routing & TLS Certificates](#traefik-edge-routing--tls-certificates)
- [Testing & Quality Assurance](#testing--quality-assurance)
- [License](#license)

---

## Key Highlights

- **Multi-Algorithm Pipeline**:
  - **Bloom Filter**: Instant $O(1)$ rejection of non-words without traversing large tree structures.
  - **Compact Prefix Trie**: Fast prefix search, autocompletion, and Levenshtein automaton matching.
  - **BK-Tree (Burkhard-Keller Metric Tree)**: Discrete metric tree partitioned by Levenshtein distance for fuzzy search.
  - **Misordered Word & Transposition Awareness**: Detects adjacent and distant transpositions (e.g. `ehad` $\rightarrow$ `head`).
  - **QWERTY Spatial Distance**: Keyboard neighbor distance penalties (e.g. typing `w` instead of `e` incurs a much lower penalty than `p`).
  - **Frequency-Weighted Confidence Ranking**: Scores suggestions from $0$ to $100\%$ using a combination of edit distance, QWERTY geometry, source priority, and Google Web Trillion corpus unigram frequencies.
- **Isolated Per-User Dictionaries**:
  - User custom jargon, acronyms, and names are encrypted and indexed separately using embedded transactional storage ([`redb`](https://github.com/cberner/redb)).
  - Fast in-memory user lexicon caching with atomic invalidation on updates.
  - Bulk word import (newline, CSV, JSON) and multi-format export (`.txt`, `.csv`, `.json`).
- **Production Observability & Metrics**:
  - Real-time telemetry via `GET /api/v1/metrics`: Resident Set Size (RSS), Peak HWM, Virtual memory, thread count, system load averages, and suggestion cache hit rates.
- **Local-First & Cloud Ready**:
  - Runs locally with zero dependencies or containerized via Docker on x86_64 and ARM64 (aarch64).

---

## Architecture & Design

For an in-depth exploration of algorithms, data structures, and database schema, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

```mermaid
flowchart TD
    Client["Client Request (REST / JSON)"] --> Traefik["Traefik Edge Proxy\n(HTTPS SNI: spell.lab / spell.rajlabs.in)"]
    Traefik --> Nginx["Nginx 1.27 Web Proxy\n(Port 80)"]
    Nginx -->|Static UI Assets| Browser["React 19 SPA"]
    Nginx -->|/api/v1/*| Axum["Rust Axum 0.8 Service\n(Port 8080)"]

    subgraph Engine["Spell-Lab Engine Core"]
        Axum --> Tokenizer["Unicode Tokenizer"]
        Tokenizer --> Bloom{"Bloom Filter\nScreening"}
        Bloom -- Hit --> GlobalTrie["Global Trie (381K words)"]
        Bloom -- Miss --> CandidateGen["Fuzzy Candidate Generator"]
        CandidateGen --> LevAutomaton["Trie Levenshtein Automaton"]
        CandidateGen --> BKTree["BK-Tree Metric Space"]
        CandidateGen --> Anagram["Misordered Anagram Solver"]
        CandidateGen --> Scorer["Confidence Scorer (0-100%)\nQWERTY + Damerau + Frequency"]
    end

    Axum --> Auth["API Key & Tenant Manager"]
    Auth --> Redb[("Redb Transactional DB\n(spell.db)")]
```

---

## Real-World API Usage & Parameters

All endpoints accept and return UTF-8 JSON. All responses include an `elapsedMs` timing parameter measuring execution latency.

### 1. Authentication

Include your API key via the HTTP request headers:
- Header option 1: `X-API-Key: <your_api_key>`
- Header option 2: `Authorization: Bearer <your_api_key>`

> **Note**: Public requests without an API key use the global 381,705-word lexicon. Including an API key unlocks your personal vocabulary, which boosts matching priority.

---

### 2. Multi-Token Spellcheck (`/api/v1/spell/check`)

Scans a multi-word paragraph or sentence, tokenizes words, detects typos, and provides ranked suggestions for each token.

#### Endpoint
`POST /api/v1/spell/check`

#### Request Parameters (JSON Body)

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `text` | `string` | **Yes** | — | Raw input string to spellcheck (up to 20,000 characters). |
| `maxSuggestions` | `integer` | No | `5` | Maximum number of candidate suggestions returned per misspelled token (clamped 1 to 20). |
| `maxDistance` | `integer` | No | `2` | Maximum allowable Damerau-Levenshtein edit distance for candidate matching (clamped 1 to 3). |
| `minConfidence` | `float` | No | `0.0` | Minimum score threshold ($0.0$ to $100.0$) a candidate must reach to be included in suggestions. |
| `includeUserDictionary` | `boolean` | No | `true` | When `true` and an API key is provided, searches your personal dictionary before the global corpus. |
| `ignoreAcronyms` | `boolean` | No | `true` | When `true`, tokens consisting purely of uppercase letters (e.g., `NASA`, `REST`, `JSON`) are marked valid. |

#### Real-World Example
```bash
curl -s -X POST https://spell.lab/api/v1/spell/check \
  -H "X-API-Key: sl_b5835ea79e..." \
  -H "Content-Type: application/json" \
  -d '{
    "text": "Teh kubernetes clustr is runing smoothly.",
    "maxSuggestions": 3,
    "maxDistance": 2,
    "minConfidence": 70.0,
    "includeUserDictionary": true,
    "ignoreAcronyms": true
  }'
```

#### Response Example
```json
{
  "text": "Teh kubernetes clustr is runing smoothly.",
  "tokens": [
    {
      "raw": "Teh",
      "normalized": "teh",
      "start": 0,
      "end": 3,
      "byteStart": 0,
      "byteEnd": 3,
      "correct": false,
      "ignored": false,
      "inUserDictionary": false,
      "suggestions": [
        {
          "word": "The",
          "distance": 1,
          "confidence": 92,
          "frequency": 22761659,
          "source": "global",
          "reason": "transposition"
        }
      ]
    },
    {
      "raw": "kubernetes",
      "normalized": "kubernetes",
      "start": 4,
      "end": 14,
      "byteStart": 4,
      "byteEnd": 14,
      "correct": true,
      "ignored": false,
      "inUserDictionary": true,
      "suggestions": []
    }
  ],
  "stats": {
    "totalTokens": 6,
    "correctWords": 4,
    "misspelledWords": 2,
    "ignoredWords": 0,
    "suggestionCount": 5,
    "textChars": 41,
    "elapsedMs": 1.25
  },
  "dictionary": {
    "globalWords": 381705,
    "userWords": 13,
    "userDictionaryIncluded": true
  }
}
```

---

### 3. Single-Word Suggestions (`/api/v1/spell/suggest`)

Used for fast, targeted candidate lookup on a single token (e.g. typing or right-click context menus).

#### Endpoint
`POST /api/v1/spell/suggest`

#### Request Parameters (JSON Body)

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `word` | `string` | **Yes** | — | Word token to evaluate (1 to 64 ASCII/Unicode characters). |
| `maxSuggestions` | `integer` | No | `5` | Maximum number of candidates returned. |
| `maxDistance` | `integer` | No | `2` | Maximum allowable edit distance. |
| `minConfidence` | `float` | No | `0.0` | Minimum confidence score ($0.0$ to $100.0$). |
| `includeUserDictionary` | `boolean` | No | `true` | Include authenticated user words. |

#### Real-World Example
```bash
curl -s -X POST https://spell.lab/api/v1/spell/suggest \
  -H "Content-Type: application/json" \
  -d '{
    "word": "odre",
    "maxSuggestions": 3,
    "minConfidence": 75.0
  }'
```

#### Response Example
```json
{
  "word": "odre",
  "correct": false,
  "userDictionaryIncluded": false,
  "suggestions": [
    {
      "word": "order",
      "distance": 1,
      "confidence": 91,
      "frequency": 2420184,
      "source": "global",
      "reason": "transposition"
    },
    {
      "word": "ore",
      "distance": 1,
      "confidence": 82,
      "frequency": 61203,
      "source": "global",
      "reason": "deletion"
    }
  ],
  "elapsedMs": 0.82
}
```

---

### 4. Prefix Autocompletion (`/api/v1/spell/complete`)

Instant prefix-based completion powered by the Compact Prefix Trie.

#### Endpoint
`GET /api/v1/spell/complete?prefix=<str>&limit=<int>&includeUserDictionary=<bool>`

#### Query Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `prefix` | `string` | **Yes** | — | The prefix string to autocomplete (minimum 1 character). |
| `limit` | `integer` | No | `10` | Max completions to return (clamped 1 to 50). |
| `includeUserDictionary` | `boolean` | No | `true` | Prioritize and search personal vocabulary. |

#### Real-World Example
```bash
curl -s "https://spell.lab/api/v1/spell/complete?prefix=kube&limit=3"
```

#### Response Example
```json
{
  "prefix": "kube",
  "completions": [
    {
      "word": "kubernetes",
      "frequency": 85040,
      "source": "user"
    },
    {
      "word": "kuber",
      "frequency": 1204,
      "source": "global"
    }
  ]
}
```

---

### 5. One-Click Auto-Correction (`/api/v1/spell/correct`)

Applies top-ranked corrections across the entire text block, returning both the clean corrected text and a structured list of exact character-range diffs.

#### Endpoint
`POST /api/v1/spell/correct`

#### Request Parameters (JSON Body)

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `text` | `string` | **Yes** | — | Input text to auto-correct. |
| `maxDistance` | `integer` | No | `2` | Max edit distance allowed for replacements. |
| `minConfidence` | `float` | No | `80.0` | Minimum confidence required to accept an auto-replacement. |
| `includeUserDictionary` | `boolean` | No | `true` | Consult user lexicon. |
| `ignoreAcronyms` | `boolean` | No | `true` | Skip all-caps tokens. |

#### Real-World Example
```bash
curl -s -X POST https://spell.lab/api/v1/spell/correct \
  -H "Content-Type: application/json" \
  -d '{
    "text": "Teh fastpai server is runing.",
    "minConfidence": 80.0
  }'
```

#### Response Example
```json
{
  "original": "Teh fastpai server is runing.",
  "corrected": "The fastapi server is running.",
  "changes": [
    {
      "start": 0,
      "end": 3,
      "original": "Teh",
      "replacement": "The",
      "confidence": 92
    },
    {
      "start": 4,
      "end": 11,
      "original": "fastpai",
      "replacement": "fastapi",
      "confidence": 90
    },
    {
      "start": 22,
      "end": 28,
      "original": "runing",
      "replacement": "running",
      "confidence": 88
    }
  ],
  "elapsedMs": 1.64
}
```

---

### 6. User Dictionary & Bulk Operations (`/api/v1/dictionary/*`)

#### Create Identity (`POST /api/v1/users`)
Creates a new tenant identity with an isolated dictionary:
```bash
curl -s -X POST https://spell.lab/api/v1/users \
  -H "Content-Type: application/json" \
  -d '{"name": "Engineering Team"}'
```

#### Bulk Import (`POST /api/v1/dictionary/import` or `POST /api/v1/dictionary/words`)
Supports importing up to **50,000 words** in a single transaction via an array or raw text payload:
```bash
curl -s -X POST https://spell.lab/api/v1/dictionary/import \
  -H "X-API-Key: <your_key>" \
  -H "Content-Type: application/json" \
  -d '{
    "text": "kubernetes, grafana\ntailscale\nprometheus, argocd"
  }'
```

#### Bulk Export (`GET /api/v1/dictionary/export`)
Exports the entire personal dictionary in `.txt`, `.csv`, or `.json` format:
```bash
# Export as plain newline-delimited text
curl -s "https://spell.lab/api/v1/dictionary/export?format=txt" -H "X-API-Key: <key>" -o dictionary.txt

# Export as CSV
curl -s "https://spell.lab/api/v1/dictionary/export?format=csv" -H "X-API-Key: <key>" -o dictionary.csv

# Export as JSON with user metadata
curl -s "https://spell.lab/api/v1/dictionary/export?format=json" -H "X-API-Key: <key>" -o dictionary.json
```

#### Delete Single Word (`DELETE /api/v1/dictionary/words/{word}`)
```bash
curl -s -X DELETE https://spell.lab/api/v1/dictionary/words/argocd -H "X-API-Key: <key>"
```

#### Clear Entire Dictionary (`DELETE /api/v1/dictionary`)
```bash
curl -s -X DELETE https://spell.lab/api/v1/dictionary -H "X-API-Key: <key>"
```

---

### 7. Telemetry & Observability (`/api/v1/metrics`, `/api/v1/stats`)

`GET /api/v1/metrics` provides sub-millisecond process insights without heavy scrapers:
```bash
curl -s https://spell.lab/api/v1/metrics
```
```json
{
  "version": "0.1.0",
  "apiVersion": "v1",
  "uptimeSeconds": 1420,
  "memory": {
    "residentMb": 79.7,
    "peakResidentMb": 90.68,
    "virtualMb": 420.89,
    "threads": 6
  },
  "systemLoad": [0.45, 0.72, 0.88],
  "globalWords": 381705,
  "users": 7,
  "userWords": 16,
  "suggestionCache": {
    "entries": 128,
    "capacity": 10000,
    "hits": 1450,
    "misses": 128,
    "hitRate": 0.918
  }
}
```

---

## Deployment Guide

### Native Systemd Service (Local)

1. Build the release binary:
   ```bash
   cargo build --release
   ```
2. Enable and start the user systemd service:
   ```bash
   systemctl --user enable --now spell-lab.service
   systemctl --user status spell-lab.service
   ```

### Docker Compose (Local or Production)

Run the full multi-stage stack (API + React web frontend):
```bash
# Production deployment on Prod1 (Server1)
docker compose -f deploy/docker-compose.prod1.yml up -d --build
```

### Traefik Edge Routing & TLS Certificates

Spell-Lab is integrated with Traefik 3.6 across both environments:

1. **Local Homelab (`spell.lab`, `spell.lab.lan`)**:
   - Traefik dynamic config: [`config/traefik/dynamic/spell-lab.yml`](file:///home/raj/server-setups/config/traefik/dynamic/spell-lab.yml).
   - TLS Certificates signed by the internal **RajLab int-server CA**:
     - Certificate: [`config/traefik/tls/certs/spell.lab.crt`](file:///home/raj/server-setups/config/traefik/tls/certs/spell.lab.crt)
     - Key: `/home/raj/server-setups/config/traefik/tls/certs/spell.lab.key`
     - SANs: `spell.lab`, `spell.lab.lan`, `spell.lab.rajlabs.in`, `spell.rajlabs.in`.
   - Renewal automated via [`scripts/renew-service-cert.py spell`](file:///home/raj/server-setups/scripts/renew-service-cert.py).

2. **Cloud / Prod 1 (`spell.rajlabs.in`)**:
   - Routed through Traefik container attached to the `coolify` Docker network.
   - Public TLS termination via Let's Encrypt automated challenge resolver.

---

## Testing & Quality Assurance

Run test suites:
```bash
# Rust core engine tests (Bloom filter, Trie, BK-Tree, Levenshtein, Redb)
cargo test

# Web TypeScript compilation & bundle verification
cd web && npm run build
```

---

## License

MIT License. See [LICENSE](LICENSE) for details.
