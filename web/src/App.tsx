import { useCallback, useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  ArrowUpRight,
  KeyRound,
  Loader2,
  RefreshCw,
  Sparkles,
} from "lucide-react";
import { api, ApiError } from "./api";
import { useLocalStorage } from "./hooks";
import type { EngineSettings, UserView } from "./types";
import { Proofreader } from "./components/Proofreader";
import { SuggestPanel } from "./components/SuggestPanel";
import { DictionaryPanel } from "./components/DictionaryPanel";
import { ApiPanel } from "./components/ApiPanel";
import { IdentityModal } from "./components/IdentityModal";
import "./styles.css";

type Tab = "proof" | "suggest" | "dictionary" | "api";

const DEFAULT_SETTINGS: EngineSettings = {
  maxSuggestions: 5,
  maxDistance: 2,
  minConfidence: 0,
  includeUserDictionary: true,
  ignoreAcronyms: true,
};

export default function App() {
  const [tab, setTab] = useState<Tab>("proof");
  const [settings, setSettings] = useLocalStorage<EngineSettings>(
    "spelllab.settings",
    DEFAULT_SETTINGS
  );
  const [apiKey, setApiKeyState] = useState<string | null>(() => api.getApiKey());
  const [identity, setIdentity] = useState<UserView | null>(null);
  const [userWords, setUserWords] = useState(0);
  const [identityState, setIdentityState] = useState<"loading" | "ready" | "error">(
    "loading"
  );
  const [identityError, setIdentityError] = useState("");
  const [identityNonce, setIdentityNonce] = useState(0);
  const [identityOpen, setIdentityOpen] = useState(false);
  const [rotating, setRotating] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [lastLatency, setLastLatency] = useState<number | null>(null);
  const [toast, setToast] = useState<{ message: string; error: boolean } | null>(null);
  const toastTimer = useRef<number | null>(null);

  const notify = useCallback((message: string, error = false) => {
    setToast({ message, error });
    if (toastTimer.current) window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 3600);
  }, []);

  useEffect(() => {
    return () => {
      if (toastTimer.current) window.clearTimeout(toastTimer.current);
    };
  }, []);

  const healthQuery = useQuery({
    queryKey: ["health"],
    queryFn: api.health,
    refetchInterval: 30_000,
    staleTime: 20_000,
    retry: 1,
  });

  const statsQuery = useQuery({
    queryKey: ["stats"],
    queryFn: api.stats,
    refetchInterval: 30_000,
    staleTime: 20_000,
    retry: 1,
  });

  useEffect(() => {
    let cancelled = false;

    async function bootstrap() {
      setIdentityState("loading");
      setIdentityError("");
      try {
        if (apiKey) {
          const me = await api.me();
          if (cancelled) return;
          setIdentity(me.user);
          setUserWords(me.userWords);
          setIdentityState("ready");
          return;
        }
        const created = await api.createUser("Browser demo");
        if (cancelled) return;
        api.setApiKey(created.apiKey);
        setApiKeyState(created.apiKey);
        setIdentity(created.user);
        setUserWords(0);
        setIdentityState("ready");
      } catch (error) {
        if (cancelled) return;
        if (error instanceof ApiError && error.status === 401 && apiKey) {
          api.setApiKey(null);
          setApiKeyState(null);
          return;
        }
        setIdentityState("error");
        setIdentityError(error instanceof Error ? error.message : "Identity unavailable");
      }
    }

    bootstrap();
    return () => {
      cancelled = true;
    };
  }, [apiKey, identityNonce]);

  const retryIdentity = () => {
    setIdentityNonce((value) => value + 1);
  };

  const rotateKey = async () => {
    setRotating(true);
    try {
      const rotated = await api.rotateKey();
      api.setApiKey(rotated.apiKey);
      setApiKeyState(rotated.apiKey);
      setIdentity((current) =>
        current ? { ...current, apiKeyPrefix: rotated.apiKeyPrefix } : current
      );
      notify("API key rotated. The previous key is now invalid.");
    } catch (error) {
      notify(error instanceof Error ? error.message : "Could not rotate the key", true);
    } finally {
      setRotating(false);
    }
  };

  const deleteIdentity = async () => {
    setDeleting(true);
    try {
      await api.deleteUser();
    } catch (error) {
      if (!(error instanceof ApiError && error.status === 401)) {
        notify(error instanceof Error ? error.message : "Could not delete identity", true);
        setDeleting(false);
        return;
      }
    }
    api.setApiKey(null);
    setApiKeyState(null);
    setIdentity(null);
    setUserWords(0);
    setIdentityOpen(false);
    setIdentityNonce((value) => value + 1);
    setDeleting(false);
    notify("Identity deleted. Creating a fresh one…");
  };

  const online = healthQuery.isSuccess;
  const statusText = healthQuery.isLoading
    ? "connecting"
    : online
      ? "API online"
      : "API offline";

  return (
    <>
      <div className="ambient" aria-hidden="true">
        <div className="orb orb-a" />
        <div className="orb orb-b" />
        <div className="grid-fade" />
      </div>

      <header className="topbar">
        <a className="brand" href="/" aria-label="Spell.lab home">
          <span className="brand-mark">
            S<span>.</span>
          </span>
          <span className="brand-copy">
            <strong>
              spell<span>.lab</span>
            </strong>
            <small>confidence-ranked spelling</small>
          </span>
        </a>

        <div className="status" title="API health">
          <span className={`dot ${online ? "ok" : healthQuery.isLoading ? "pending" : "bad"}`} />
          <span>{statusText}</span>
        </div>

        <div className="top-stats" aria-label="Live statistics">
          <div>
            <b>{healthQuery.data ? healthQuery.data.globalWords.toLocaleString() : "—"}</b>
            <span>global words</span>
          </div>
          <div>
            <b>{userWords.toLocaleString()}</b>
            <span>your words</span>
          </div>
          <div>
            <b>{lastLatency === null ? "—" : `${Math.round(lastLatency)} ms`}</b>
            <span>last check</span>
          </div>
        </div>

        {identityState === "error" ? (
          <button className="btn ghost small" type="button" onClick={retryIdentity}>
            <RefreshCw size={13} /> Retry identity
          </button>
        ) : (
          <button
            id="identityButton"
            className="btn ghost small"
            type="button"
            onClick={() => setIdentityOpen(true)}
          >
            <KeyRound size={13} />
            {identityState === "loading" ? "Connecting…" : "Identity"}
          </button>
        )}
      </header>

      <main>
        <section className="hero">
          <div className="hero-copy">
            <p className="eyebrow">
              <span className="spark" /> Rust · Axum · React · per-user dictionaries
            </p>
            <h1>
              Spellcheck that <em>explains</em> itself.
            </h1>
            <p className="lede">
              A fast REST API that finds the likely intent behind a typo and returns a
              calibrated confidence score. Built for English, tuned for real keyboards,
              and personalisable with your own vocabulary.
            </p>
            <div className="hero-actions">
              <button className="btn primary" type="button" onClick={() => setTab("proof")}>
                Try the proofreader
              </button>
              <a className="btn ghost" href="/api/v1" target="_blank" rel="noreferrer">
                API index <ArrowUpRight size={14} />
              </a>
            </div>
            <div className="algo-row" aria-label="Algorithms used">
              <span>Bloom filter</span>
              <span>compact trie</span>
              <span>BK-tree</span>
              <span>Damerau–Levenshtein</span>
              <span>QWERTY ranking</span>
              <span>React Query cache</span>
            </div>
          </div>

          <aside className="hero-card" aria-label="Example API response">
            <div className="mini-head">
              <span className="traffic" />
              <span>POST /api/v1/spell/suggest</span>
            </div>
            <pre>
              <code>
                {"{\n"}
                {'  "word": '}
                <span className="s">"teh"</span>
                {",\n"}
                {'  "correct": '}
                <span className="b">false</span>
                {",\n"}
                {'  "suggestions": [{\n'}
                {'    "word": '}
                <span className="s">"the"</span>
                {",\n"}
                {'    "distance": '}
                <span className="n">1</span>
                {",\n"}
                {'    "confidence": '}
                <span className="n">93</span>
                {",\n"}
                {'    "source": '}
                <span className="s">"global"</span>
                {",\n"}
                {'    "reason": '}
                <span className="s">"transposition"</span>
                {"\n  }]\n}"}
              </code>
            </pre>
            <div className="mini-foot">
              <span className="pulse" /> ranked, not just returned
            </div>
          </aside>
        </section>

        <nav className="tabs" aria-label="Sections">
          {(
            [
              ["proof", "Proofreader"],
              ["suggest", "Suggest & complete"],
              ["dictionary", "My dictionary"],
              ["api", "API & docs"],
            ] as const
          ).map(([value, label]) => (
            <button
              key={value}
              className={`tab ${tab === value ? "active" : ""}`}
              data-tab={value}
              type="button"
              onClick={() => setTab(value)}
            >
              {label}
            </button>
          ))}
        </nav>

        <section id="tab-proof" className={`tab-panel ${tab === "proof" ? "active" : ""}`}>
          <Proofreader
            settings={settings}
            onSettingsChange={setSettings}
            apiKey={apiKey}
            notify={notify}
            onLatency={setLastLatency}
          />
        </section>

        <section
          id="tab-suggest"
          className={`tab-panel ${tab === "suggest" ? "active" : ""}`}
        >
          <SuggestPanel settings={settings} apiKey={apiKey} />
        </section>

        <section
          id="tab-dictionary"
          className={`tab-panel ${tab === "dictionary" ? "active" : ""}`}
        >
          <DictionaryPanel
            apiKey={apiKey}
            identity={identity}
            notify={notify}
            onCountChange={setUserWords}
            onOpenIdentity={() => setIdentityOpen(true)}
          />
        </section>

        <section id="tab-api" className={`tab-panel ${tab === "api" ? "active" : ""}`}>
          <ApiPanel stats={statsQuery.data} statsLoading={statsQuery.isLoading} />
        </section>
      </main>

      <footer>
        <span>Spell.lab · Rust + Axum API · React + Vite tester</span>
        <span className="footer-links">
          <a href="https://github.com/authoritydmc/spell-lab" target="_blank" rel="noreferrer">
            GitHub
          </a>
          <a href="/api/v1/stats" target="_blank" rel="noreferrer">
            Runtime stats
          </a>
          <a href="/api/v1/health" target="_blank" rel="noreferrer">
            Health
          </a>
        </span>
      </footer>

      {identityState === "error" && (
        <div className="identity-warning">
          <span>{identityError}</span>
          <button className="btn ghost small" type="button" onClick={retryIdentity}>
            <Loader2 size={13} /> Retry
          </button>
        </div>
      )}

      <IdentityModal
        open={identityOpen}
        apiKey={apiKey}
        identity={identity}
        rotating={rotating}
        deleting={deleting}
        onClose={() => setIdentityOpen(false)}
        onRotate={rotateKey}
        onDelete={deleteIdentity}
        notify={notify}
      />

      <div className={`toast ${toast ? "show" : ""} ${toast?.error ? "error" : ""}`} role="status">
        {toast?.message}
      </div>

      <div className="sr-only" aria-live="polite">
        <Sparkles size={1} /> Spell.lab
      </div>
    </>
  );
}
