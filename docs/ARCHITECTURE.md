# Architecture Deep Dive — Spell-Lab

High-performance, confidence-ranked spellchecker and morphological correction engine built in **Rust** (Axum, Rayon, Redb) with an interactive **React** interface.

---

## 1. System Architecture

```mermaid
flowchart TD
    subgraph Clients["Client Layer"]
        Browser["Modern Browser / SPA\n(React + Vite + TanStack Query)"]
        CLI["External APIs / cURL / CI Scripts\n(Authorization: Bearer <key>)"]
    end

    subgraph Edge["Edge & Reverse Proxy Layer"]
        TraefikLocal["Local Traefik (rajlab)\nSNI: spell.lab, spell.lab.lan\nTLS: RajLabs Intermediate CA"]
        TraefikCloud["Cloud Traefik (server1)\nSNI: spell.rajlabs.in\nTLS: Let's Encrypt / Wildcard"]
    end

    subgraph Service["Spell-Lab Service Runtime"]
        subgraph FrontProxy["Frontend Web Gateway"]
            Nginx["Nginx 1.27 Static Server\n- Compression: Gzip\n- SPA Routing (/index.html)\n- Reverse Proxy (/api/ -> Core)"]
        end

        subgraph Core["Core Rust Engine (Axum 0.8)"]
            Router["Axum HTTP Router & Middleware\n- TraceLayer, CorsLayer, CompressionLayer\n- Request Body Limit (2MB)"]
            Auth["Auth & Context Layer\n- X-API-Key / Bearer Extractor\n- LRU Lexicon Cache"]
            Tokenizer["Unicode Tokenizer\n- Word boundary detection\n- Acronym & digit filtering"]
            Pipeline["Correction & Scoring Pipeline\n- Bloom Filter (O(1) screening)\n- Compact Trie (Levenshtein Automaton)\n- BK-Tree (Metric Levenshtein Space)\n- Misordered Anagram Finder\n- QWERTY Distance Penalty Matrix\n- Frequency Prior Weighting"]
        end

        subgraph Storage["Storage & Persistence"]
            Corpus["Global Read-Only Lexicon\n(381,705 words embedded via include_str!)"]
            Redb["Redb Transactional Database (spell.db)\n- users table (uuid -> profile)\n- api_keys table (sha256 -> uuid)\n- user_words table (user_id:word -> ())"]
        end
    end

    Browser --> TraefikLocal
    Browser --> TraefikCloud
    CLI --> TraefikLocal
    CLI --> TraefikCloud

    TraefikLocal --> Nginx
    TraefikCloud --> Nginx
    Nginx -->|/api/*| Router
    Router --> Auth
    Auth --> Redb
    Auth --> Tokenizer
    Tokenizer --> Pipeline
    Pipeline --> Corpus
    Pipeline --> Redb
```

---

## 2. Spellcheck & Candidate Scoring Pipeline

Every inbound spellcheck or suggest request executes through a deterministic, multi-stage heuristic pipeline:

```mermaid
flowchart TD
    Start(["Input Token e.g. 'ehad'"]) --> Normalizer["Normalize ASCII & Lowercase"]
    Normalizer --> AcronymCheck{"Ignore Acronyms enabled\n& all-uppercase?"}
    AcronymCheck -- Yes --> TokenIgnored(["Mark Ignored & Correct"])
    AcronymCheck -- No --> UserDictCheck{"User Dictionary\nIncluded?"}

    UserDictCheck -- Yes --> UserTrie{"In User Lexicon\n(Trie)?"}
    UserTrie -- Yes --> FoundUser(["Mark Correct (Source: User)"])
    UserTrie -- No --> BloomFilter{"Bloom Filter\nMatch?"}
    UserDictCheck -- No --> BloomFilter

    BloomFilter -- Match --> GlobalTrie{"In Global Corpus\n(Trie)?"}
    GlobalTrie -- Yes --> FoundGlobal(["Mark Correct (Source: Global)"])
    GlobalTrie -- No --> CandidateGen
    BloomFilter -- Miss --> CandidateGen["Candidate Generation Engine"]

    subgraph CandidateGen["Parallel Candidate Generation"]
        LevAutomaton["Levenshtein Automaton on Trie\n(Edit Distance <= maxDistance)"]
        BKTree["BK-Tree Metric Traversal\n(Burkhard-Keller d(u, v) <= 2)"]
        Anagram["Misordered / Anagram Matcher\n(Length match + char multiset eq)"]
    end

    CandidateGen --> MergePool["Deduplicated Candidate Pool"]
    MergePool --> Scoring["Confidence Scoring Engine (0 - 100%)"]

    subgraph Scoring["Confidence Scoring Engine"]
        DLDist["Damerau-Levenshtein Metric\n(Insertion, Deletion, Substitution, Transposition)"]
        Keyboard["QWERTY Spatial Matrix\n(Adjacent key distance penalty)"]
        FreqPrior["Log-Frequency Prior\n(Corpus unigram occurrence)"]
        UserBoost["User Dictionary Source Boost\n(+15% score priority)"]
    end

    Scoring --> ThresholdFilter{"Score >= minConfidence?"}
    ThresholdFilter -- Yes --> Rank["Sort by Confidence DESC\nFrequency DESC, Distance ASC"]
    ThresholdFilter -- No --> Discard["Drop Candidate"]
    Rank --> Limit["Clamp to maxSuggestions"]
    Limit --> Output(["Return Ranked JSON"])
```

---

## 3. Data Storage & Multi-Tenant Isolation

User dictionaries and authentication keys are kept isolated in transactional storage powered by [`redb`](https://github.com/cberner/redb):

```mermaid
erDiagram
    USERS ||--o{ API_KEYS : "has"
    USERS ||--o{ USER_WORDS : "owns"

    USERS {
        string id PK "UUID v4"
        string name "User / Org label"
        string api_key_hash "SHA256 hex digest"
        string api_key_prefix "sl_xxxxxxxx"
        string created_at "ISO-8601 UTC timestamp"
    }

    API_KEYS {
        string api_key_hash PK "SHA256 hex digest"
        string user_id FK "UUID v4"
    }

    USER_WORDS {
        string user_id_and_word PK "prefix: {user_id}:{normalized_word}"
        unit value "Zero-byte entry"
    }
```

- **Prefix-range lookups**: All user words are stored under the compound key `{user_id}:{word}`. Range queries scan only the user's specific partition (`range(user_id..)`).
- **Fast In-Memory Cache**: Each user has an in-memory `Arc<Lexicon>` constructed on first query and stored in a synchronized `RwLock<HashMap<String, Arc<Lexicon>>>`. Invalidation occurs immediately on word additions, removals, or bulk clears.

---

## 4. Edge Traefik & TLS Certificate Architecture

```mermaid
sequenceDiagram
    autonumber
    actor Client as User / Browser
    participant Traefik as Traefik 3.6 Reverse Proxy
    participant CertSigner as RajLabs PKI (cert-signer)
    participant SpellWeb as Nginx Frontend (spell-lab-web)
    participant SpellAPI as Rust Core (spell-lab-api)

    Note over Client,Traefik: 1. TLS Handshake with SNI: spell.lab or spell.rajlabs.in
    Traefik->>Traefik: Match SNI in dynamic TLS stores (spell.lab.crt)
    Traefik-->>Client: Serve TLS Certificate (Issued by RajLab int-server CA)
    Client->>Traefik: HTTP/2 GET / or POST /api/v1/spell/check
    Traefik->>SpellWeb: Proxy port 80
    alt Static SPA Asset Request
        SpellWeb-->>Client: Serve HTML / JS / CSS (Gzip cached)
    else API Request (/api/*)
        SpellWeb->>SpellAPI: Proxy pass http://api:8080/api/*
        SpellAPI-->>SpellWeb: JSON Response with stats
        SpellWeb-->>Client: Filtered & compressed JSON
    end
```
