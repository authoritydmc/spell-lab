# spell-lab

High-performance, confidence-ranked spellcheck and correction REST API built in **Rust** with an interactive **React** web testing interface.

Features native per-user isolated custom dictionaries, multi-layered spellcheck algorithmic pipelines (Bloom Filter, Compact Prefix Trie, BK-Tree metric space, Damerau-Levenshtein transposition awareness, and QWERTY proximity weighting), with sub-millisecond query responses.

---

## Key Highlights

- **Multi-Algorithm Pipeline**:
  - **Bloom Filter**: Instant $O(1)$ rejection of non-words without traversing large tree structures.
  - **Compact Prefix Trie**: Fast prefix search, autocompletion, and Levenshtein automaton matching.
  - **BK-Tree (Burkhard-Keller Metric Tree)**: Discrete metric tree partitioned by Levenshtein distance for fuzzy search.
  - **Misordered Word & Transposition Awareness**: Detects anagrams, adjacent transpositions, and typoglycemia variations (e.g. `ehad` $\rightarrow$ `head`).
  - **QWERTY Spatial Distance**: Keyboard neighbor distance penalties (e.g. typing `w` instead of `e` incurs lower penalty than `p`).
  - **Frequency-Weighted Confidence Ranking**: Scores suggestions from $0$ to $100\%$ using a combination of edit distance, QWERTY geometry, source priority, and Google Web Trillion corpus unigram frequencies.
- **Isolated Per-User Dictionaries**:
  - Each user receives an API key. Custom jargon, acronyms, and names are encrypted and indexed separately using embedded transactional storage ([`redb`](https://github.com/cberner/redb)).
  - Fast in-memory user lexicon caching with LRU invalidation.
- **Production Observability & Metrics**:
  - Live resource telemetry via `GET /api/v1/metrics`: reports process memory footprint (Resident Set Size, Peak HWM, Virtual memory), thread count, system load averages, and suggestion cache hit rates.
- **Local-First & Cloud Ready**:
  - Ships with Docker Compose, native Traefik integration, and automated systemd management.

---

## Architectural Workflow

```
                        [ Inbound HTTP Request ]
                                   │
                                   ▼
                       [ Axum Router + Security ]
                                   │
                                   ▼
                    [ User Auth & API Key Resolver ]
                       ├── Default: Global Corpus (381K words)
                       └── With Key: Global + Isolated User Lexicon
                                   │
                                   ▼
                       [ Tokenizer & Normalizer ]
                                   │
                                   ▼
                        [ Bloom Filter Screening ]
                       ├── Hit  -> Verified in Trie -> Correct (0ms)
                       └── Miss -> Fuzzy Ranking Pipeline
                                   │
                                   ▼
                         [ Candidate Generator ]
                       ├── Trie Levenshtein Automaton (dist <= 2)
                       ├── BK-Tree Metric Traversal
                       └── Misordered / Anagram Solver
                                   │
                                   ▼
                   [ Confidence Scoring & Ranker (0-100%) ]
                       ├── Damerau-Levenshtein Distance
                       ├── QWERTY Keyboard Proximity
                       ├── Corpus Frequency Prior
                       └── User Dictionary Source Boost
                                   │
                                   ▼
                         [ Output JSON Envelope ]
```

---

## API Reference

All requests accept and return JSON. Authenticate user requests with header `X-API-Key: <key>` or `Authorization: Bearer <key>`.

### 1. Health & Observability Metrics

#### `GET /api/v1/health`
Returns quick service health and dictionary size:
```json
{
  "status": "ok",
  "version": "0.1.0",
  "apiVersion": "v1",
  "uptimeSeconds": 120,
  "globalWords": 381705,
  "users": 3
}
```

#### `GET /api/v1/metrics`
Returns real-time memory usage, system load, and cache performance:
```json
{
  "version": "0.1.0",
  "apiVersion": "v1",
  "uptimeSeconds": 120,
  "memory": {
    "residentMb": 79.7,
    "peakResidentMb": 90.68,
    "virtualMb": 420.89,
    "threads": 6
  },
  "systemLoad": [0.45, 0.72, 0.88],
  "globalWords": 381705,
  "users": 3,
  "userWords": 1,
  "suggestionCache": {
    "entries": 42,
    "capacity": 10000,
    "hits": 312,
    "misses": 42,
    "hitRate": 0.881
  }
}
```

---

### 2. Spellcheck & Suggestions

#### `POST /api/v1/spell/suggest`
Suggests corrections for a single word:
```bash
curl -s -X POST https://spell.lab/api/v1/spell/suggest \
  -H "Content-Type: application/json" \
  -d '{"word": "ehad", "maxSuggestions": 5}'
```

Response:
```json
{
  "word": "ehad",
  "correct": false,
  "userDictionaryIncluded": false,
  "suggestions": [
    {
      "word": "head",
      "distance": 2,
      "confidence": 92,
      "frequency": 820311,
      "source": "global",
      "reason": "misordered"
    },
    {
      "word": "had",
      "distance": 1,
      "confidence": 88,
      "frequency": 12044810,
      "source": "global",
      "reason": "deletion"
    }
  ],
  "elapsedMs": 1.42
}
```

#### `POST /api/v1/spell/check`
Tokenizes a full block of text and returns offset positions, errors, and ranked suggestions for each token:
```bash
curl -s -X POST https://spell.lab/api/v1/spell/check \
  -H "Content-Type: application/json" \
  -d '{"text": "The quikc brown fox jumpd ovr the lazzy dog."}'
```

#### `GET /api/v1/spell/complete?prefix=progr`
Fast prefix completion backed by compact Trie traversal:
```bash
curl -s "https://spell.lab/api/v1/spell/complete?prefix=progr&limit=5"
```

---

### 3. User Dictionaries

#### `POST /api/v1/users`
Creates an isolated user account and issues an API key:
```bash
curl -s -X POST https://spell.lab/api/v1/users \
  -H "Content-Type: application/json" \
  -d '{"name": "Engineering Team"}'
```

#### `POST /api/v1/dictionary/words`
Adds custom terms to the authenticated user's dictionary:
```bash
curl -s -X POST https://spell.lab/api/v1/dictionary/words \
  -H "X-API-Key: <your-key>" \
  -H "Content-Type: application/json" \
  -d '{"words": ["RajLabs", "Kubernetes", "gRPC", "Traefik"]}'
```

---

## Deployment & Running

### Option A: Local / Native Service (with systemd)
```bash
# Build release binary
cargo build --release

# Environment configuration
export SPELL_BIND="127.0.0.1:8099"
export SPELL_DB_PATH="./data/spell.db"
export SPELL_PUBLIC_URL="https://spell.lab"

# Run binary
./target/release/spell-lab
```

Run as a systemd user service:
```bash
systemctl --user enable --now spell-lab.service
```

### Option B: Docker Compose
```bash
docker compose up -d --build
```

---

## Testing

Run unit and integration tests:
```bash
cargo test
```

Run end-to-end smoke verification:
```bash
PUBLIC_URL=https://spell.lab make smoke
```

---

## License

MIT License. See [LICENSE](LICENSE) for details.
