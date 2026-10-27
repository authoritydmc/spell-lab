import { useQuery } from "@tanstack/react-query";
import {
  Activity,
  Cpu,
  Database,
  ExternalLink,
  HardDrive,
  Layers,
  LineChart,
  RefreshCw,
  Server,
  Zap,
} from "lucide-react";
import { api } from "../api";
import type { MetricsResponse, StatsResponse } from "../types";
import { ApiCallViewer } from "./ApiCallViewer";

interface Props {
  stats?: StatsResponse;
  statsLoading: boolean;
}

export function ApiPanel({ stats, statsLoading }: Props) {
  const base = api.base.startsWith("http")
    ? api.base
    : `${window.location.origin}${api.base}`;

  const metricsQuery = useQuery<MetricsResponse>({
    queryKey: ["metrics"],
    queryFn: api.metrics,
    refetchInterval: 5_000,
  });

  const m = metricsQuery.data;

  return (
    <>
      {/* =====================================================================
          1. LIVE PERFORMANCE & RESOURCE TELEMETRY
          ===================================================================== */}
      <article className="card perf-card">
        <div className="card-head">
          <div>
            <p className="kicker">REAL-TIME TELEMETRY</p>
            <h2>Performance & System Vitals</h2>
          </div>
          <div className="card-actions-row">
            <span className="live-badge">
              <span className="live-dot" /> Polling (5s)
            </span>
            <button
              className="btn ghost small"
              type="button"
              onClick={() => {
                metricsQuery.refetch();
              }}
              disabled={metricsQuery.isFetching}
            >
              <RefreshCw size={12} className={metricsQuery.isFetching ? "spin" : ""} /> Refresh
            </button>
          </div>
        </div>

        <div className="perf-grid">
          {/* Memory Vital */}
          <div className="perf-tile">
            <div className="perf-tile-header">
              <span className="perf-tile-label">
                <HardDrive size={14} /> Resident Memory (RSS)
              </span>
              <span className="perf-pill ok">Active</span>
            </div>
            <div className="perf-tile-value">
              {m ? `${m.memory.residentMb.toFixed(1)} MB` : <span className="skeleton w-45" />}
            </div>
            <div className="perf-progress-wrap">
              <div
                className="perf-progress-bar"
                style={{
                  width: m
                    ? `${Math.min(100, (m.memory.residentMb / m.memory.peakResidentMb) * 100)}%`
                    : "0%",
                }}
              />
            </div>
            <div className="perf-tile-foot">
              <span>Peak: {m ? `${m.memory.peakResidentMb.toFixed(1)} MB` : "—"}</span>
              <span>Virtual: {m ? `${m.memory.virtualMb.toFixed(1)} MB` : "—"}</span>
            </div>
          </div>

          {/* System Load & Threads */}
          <div className="perf-tile">
            <div className="perf-tile-header">
              <span className="perf-tile-label">
                <Cpu size={14} /> CPU & Concurrency
              </span>
              <span className="perf-pill accent">
                {m ? `${m.memory.threads} OS Threads` : "Threads"}
              </span>
            </div>
            <div className="perf-tile-value">
              {m ? m.systemLoad.map((l) => l.toFixed(2)).join(" · ") : <span className="skeleton w-55" />}
            </div>
            <p className="perf-tile-subtitle">System load averages (1m, 5m, 15m)</p>
            <div className="perf-tile-foot">
              <span>Rayon + Tokio runtime</span>
              <span>Non-blocking async</span>
            </div>
          </div>

          {/* LRU Cache Hit Rate */}
          <div className="perf-tile">
            <div className="perf-tile-header">
              <span className="perf-tile-label">
                <Zap size={14} /> Suggestion Cache Hit Rate
              </span>
              <span className="perf-pill neutral">LRU 10k</span>
            </div>
            <div className="perf-tile-value">
              {m ? (
                `${(m.suggestionCache.hitRate * 100).toFixed(1)}%`
              ) : (
                <span className="skeleton w-35" />
              )}
            </div>
            <div className="perf-progress-wrap">
              <div
                className="perf-progress-bar accent"
                style={{
                  width: m ? `${m.suggestionCache.hitRate * 100}%` : "0%",
                }}
              />
            </div>
            <div className="perf-tile-foot">
              <span>{m ? `${m.suggestionCache.hits.toLocaleString()} hits` : "—"}</span>
              <span>{m ? `${m.suggestionCache.misses.toLocaleString()} misses` : "—"}</span>
            </div>
          </div>

          {/* Lexicon & Storage Scale */}
          <div className="perf-tile">
            <div className="perf-tile-header">
              <span className="perf-tile-label">
                <Database size={14} /> Corpus & Tenant Scale
              </span>
              <span className="perf-pill purple">Redb Zero-Copy</span>
            </div>
            <div className="perf-tile-value">
              {m ? (
                `${m.globalWords.toLocaleString()}`
              ) : (
                <span className="skeleton w-45" />
              )}
              <small> words</small>
            </div>
            <p className="perf-tile-subtitle">
              Embedded English Unigrams + Bigrams
            </p>
            <div className="perf-tile-foot">
              <span>{m ? `${m.users} tenant users` : "—"}</span>
              <span>{m ? `${m.userWords} custom words` : "—"}</span>
            </div>
          </div>
        </div>
      </article>

      {/* =====================================================================
          2. API CALLS & QUICK START
          ===================================================================== */}
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
              ["POST", "/api/v1/dictionary/words", "Add up to 50,000 words."],
              ["GET", "/api/v1/dictionary/export", "Export in txt, csv, or json format."],
              ["DEL", "/api/v1/dictionary/words/{word}", "Remove one word."],
              ["GET", "/api/v1/health", "Liveness and corpus size."],
              ["GET", "/api/v1/metrics", "Real-time RAM, CPU, load & cache telemetry."],
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

      {/* =====================================================================
          3. ARCHITECTURE & MULTI-LAYER ALGORITHMS
          ===================================================================== */}
      <article className="card doc-card">
        <div className="card-head">
          <div>
            <p className="kicker">UNDER THE HOOD</p>
            <h2>Storage, caching and algorithms</h2>
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
            <h3>Bloom + compact trie</h3>
            <p>
              A Bloom filter rejects unknown words in $O(1)$ time before traversing
              the compact trie and hash indexes.
            </p>
          </div>
          <div>
            <h3>BK-tree metric space</h3>
            <p>
              Discrete metric space partitioned by Levenshtein distance provides an independent
              neighbourhood when the trie automaton is pruned.
            </p>
          </div>
          <div>
            <h3>LRU suggestion cache</h3>
            <p>
              Global candidate lists are memoised by word and settings; user dictionary terms are
              dynamically blended in after cache retrieval.
            </p>
          </div>
          <div>
            <h3>Multi-tenant Redb storage</h3>
            <p>
              Each user lexicon is isolated under `{`{user_id}:{word}`}` compound keys with transactional ACID
              safety in embedded redb.
            </p>
          </div>
          <div>
            <h3>QWERTY & spatial geometry</h3>
            <p>
              Calculates Euclidean keyboard neighbor penalties so adjacent typo keys (e.g. `w` instead of `e`) score higher confidence.
            </p>
          </div>
          <div>
            <h3>Frequency priors</h3>
            <p>
              Unigram occurrence stats from Google Web Trillion ensure everyday words always outrank obscure dictionary entries.
            </p>
          </div>
        </div>
      </article>

      {/* =====================================================================
          4. INTERACTIVE LIVE BACKEND API TESTERS
          ===================================================================== */}
      <div className="split">
        <ApiCallViewer
          title="Health & Liveness Endpoint"
          method="GET"
          path="/api/v1/health"
          description="Instant liveness, version, and global lexicon word counts."
        />

        <ApiCallViewer
          title="Observability & Process Metrics"
          method="GET"
          path="/api/v1/metrics"
          response={m}
          description="Real-time telemetry showing RSS, Peak HWM, Virtual memory, load averages, and LRU suggestion cache stats."
        />
      </div>
    </>
  );
}

function formatUptime(seconds: number) {
  if (seconds < 60) return `${Math.round(seconds)}s`;
  if (seconds < 3600) return `${Math.round(seconds / 60)}m`;
  if (seconds < 86400) return `${Math.round(seconds / 3600)}h`;
  return `${Math.round(seconds / 86400)}d`;
}
