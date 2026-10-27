import { ExternalLink } from "lucide-react";
import { api } from "../api";
import type { StatsResponse } from "../types";

interface Props {
  stats?: StatsResponse;
  statsLoading: boolean;
}

export function ApiPanel({ stats, statsLoading }: Props) {
  const base = api.base.startsWith("http")
    ? api.base
    : `${window.location.origin}${api.base}`;

  return (
    <>
      <div className="split">
        <article className="card">
          <div className="card-head">
            <div>
              <p className="kicker">QUICK START</p>
              <h2>Call the API</h2>
            </div>
            <a className="btn ghost small" href={base} target="_blank" rel="noreferrer">
              JSON index <ExternalLink size={12} />
            </a>
          </div>
          <p className="muted">
            Create a user once, then send the key with every user-dictionary request.
            Global spellcheck works without a key.
          </p>
          <pre className="code-block">
            <code>{`# create a user (returns an API key)
curl -sX POST ${base}/users \\
  -H 'content-type: application/json' \\
  -d '{"name":"my-app"}'

# check text
curl -sX POST ${base}/spell/check \\
  -H 'content-type: application/json' \\
  -H 'x-api-key: sl_…' \\
  -d '{"text":"Teh quick brown fox","maxSuggestions":5,"maxDistance":2}'

# add personal words
curl -sX POST ${base}/dictionary/words \\
  -H 'content-type: application/json' \\
  -H 'x-api-key: sl_…' \\
  -d '{"words":["kubernetes","grafana"]}'`}</code>
          </pre>
        </article>

        <article className="card">
          <div className="card-head">
            <div>
              <p className="kicker">REFERENCE</p>
              <h2>Endpoints</h2>
            </div>
          </div>
          <div className="endpoint-list">
            {[
              ["POST", "/api/v1/spell/check", "Tokenise and check a whole text."],
              ["POST", "/api/v1/spell/suggest", "Rank suggestions for one word."],
              ["POST", "/api/v1/spell/correct", "Apply high-confidence corrections."],
              ["GET", "/api/v1/spell/complete", "Prefix completion from the trie."],
              ["GET", "/api/v1/dictionary", "List your dictionary (paginated)."],
              ["POST", "/api/v1/dictionary/words", "Add up to 5,000 words."],
              ["DEL", "/api/v1/dictionary/words/{word}", "Remove one word."],
              ["GET", "/api/v1/health", "Liveness and corpus size."],
              ["GET", "/api/v1/stats", "Cache, storage and runtime stats."],
            ].map(([method, path, description]) => (
              <div key={path}>
                <span className={`method ${method === "DEL" ? "delete" : method.toLowerCase()}`}>
                  {method}
                </span>
                <code>{path}</code>
                <p>{description}</p>
              </div>
            ))}
          </div>
        </article>
      </div>

      <article className="card doc-card">
        <div className="card-head">
          <div>
            <p className="kicker">UNDER THE HOOD</p>
            <h2>Storage, caching and deployment</h2>
          </div>
          <a
            className="btn ghost small"
            href="https://github.com/authoritydmc/spell-lab"
            target="_blank"
            rel="noreferrer"
          >
            GitHub <ExternalLink size={12} />
          </a>
        </div>

        {statsLoading ? (
          <div className="stat-strip">
            {[0, 1, 2, 3].map((item) => (
              <div className="stat-box" key={item}>
                <span className="skeleton w-45" />
                <span className="skeleton w-70" />
              </div>
            ))}
          </div>
        ) : stats ? (
          <div className="stat-strip">
            <div className="stat-box">
              <b>{(stats.suggestionCache.hitRate * 100).toFixed(1)}%</b>
              <span>suggestion cache hit rate</span>
            </div>
            <div className="stat-box">
              <b>
                {stats.suggestionCache.entries.toLocaleString()}
                <small> / {stats.suggestionCache.capacity.toLocaleString()}</small>
              </b>
              <span>cached suggestion sets</span>
            </div>
            <div className="stat-box">
              <b>{stats.userLexiconCacheEntries}</b>
              <span>hot user lexicons</span>
            </div>
            <div className="stat-box">
              <b>{formatUptime(stats.uptimeSeconds)}</b>
              <span>uptime</span>
            </div>
          </div>
        ) : (
          <div className="empty-state">Runtime stats are temporarily unavailable.</div>
        )}

        <div className="feature-grid">
          <div>
            <h3>Bloom + trie</h3>
            <p>
              A Bloom filter rejects most unknown words before the compact trie and hash
              index are touched.
            </p>
          </div>
          <div>
            <h3>BK-tree fallback</h3>
            <p>
              Levenshtein pruning gives an independent neighbourhood when the trie
              automaton is truncated.
            </p>
          </div>
          <div>
            <h3>LRU suggestion cache</h3>
            <p>
              Global candidates are memoised by word + settings; user dictionaries are
              merged after the cache.
            </p>
          </div>
          <div>
            <h3>Per-user cache</h3>
            <p>
              Each user lexicon is built once and invalidated on dictionary writes.
              Embedded redb stores the data.
            </p>
          </div>
          <div>
            <h3>Docker-first</h3>
            <p>
              Multi-stage API and web images, non-root runtime, healthchecks, persistent
              volume and Traefik labels.
            </p>
          </div>
          <div>
            <h3>Observable</h3>
            <p>
              Structured tracing, JSON access logs at the edge, health/stats endpoints and
              cache hit rates.
            </p>
          </div>
        </div>
      </article>
    </>
  );
}

function formatUptime(seconds: number) {
  if (seconds < 60) return `${Math.round(seconds)}s`;
  if (seconds < 3600) return `${Math.round(seconds / 60)}m`;
  if (seconds < 86400) return `${Math.round(seconds / 3600)}h`;
  return `${Math.round(seconds / 86400)}d`;
}
