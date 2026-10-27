import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Loader2, Search, Sparkles } from "lucide-react";
import { api } from "../api";
import type { EngineSettings } from "../types";
import { ApiCallViewer } from "./ApiCallViewer";

interface Props {
  settings: EngineSettings;
  apiKey: string | null;
}

export function SuggestPanel({ settings, apiKey }: Props) {
  const [word, setWord] = useState("recieve");
  const [submittedWord, setSubmittedWord] = useState("recieve");
  const [prefix, setPrefix] = useState("spel");
  const [submittedPrefix, setSubmittedPrefix] = useState("spel");

  const includeUser = settings.includeUserDictionary && Boolean(apiKey);

  const suggestQuery = useQuery({
    queryKey: ["suggest", submittedWord, settings.maxSuggestions, settings.maxDistance, settings.minConfidence, includeUser],
    queryFn: () =>
      api.suggest({
        word: submittedWord,
        maxSuggestions: settings.maxSuggestions,
        maxDistance: settings.maxDistance,
        minConfidence: settings.minConfidence,
        includeUserDictionary: includeUser,
      }),
    enabled: submittedWord.trim().length > 0,
    staleTime: 60_000,
    retry: false,
  });

  const completeQuery = useQuery({
    queryKey: ["complete", submittedPrefix, includeUser],
    queryFn: () => api.complete(submittedPrefix, 12, includeUser),
    enabled: submittedPrefix.trim().length > 0,
    staleTime: 60_000,
    retry: false,
  });

  const fillWord = (value: string) => {
    setWord(value);
    setSubmittedWord(value);
  };

  return (
    <>
      <div className="split">
        <article className="card">
          <div className="card-head">
            <div>
              <p className="kicker">SINGLE WORD</p>
              <h2>Ask for suggestions</h2>
            </div>
            <span className="pill neutral">
              <Sparkles size={11} /> ranked
            </span>
          </div>
          <form
            className="search-row"
            onSubmit={(event) => {
              event.preventDefault();
              if (word.trim()) setSubmittedWord(word.trim());
              else if (submittedWord) suggestQuery.refetch();
            }}
          >
            <input
              type="text"
              value={word}
              autoComplete="off"
              spellCheck={false}
              placeholder="Type a misspelled word"
              onChange={(event) => setWord(event.target.value)}
            />
            <button className="btn primary" type="submit" disabled={!word.trim()}>
              {suggestQuery.isFetching ? <Loader2 className="spin" size={14} /> : <Search size={14} />}
              Suggest
            </button>
          </form>

          {suggestQuery.isError ? (
            <div className="error-box">
              <strong>Could not suggest.</strong>
              <span>{suggestQuery.error.message}</span>
              <button className="btn ghost small" type="button" onClick={() => suggestQuery.refetch()}>
                Retry
              </button>
            </div>
          ) : suggestQuery.isPending ? (
            <ResultSkeleton />
          ) : suggestQuery.data ? (
            <>
              <div className="result-summary">
                <span>
                  <strong>{suggestQuery.data.word}</strong>{" "}
                  {suggestQuery.data.correct ? "is already correct" : "is not in the dictionary"}
                </span>
                <span className="badge">{Math.round(suggestQuery.data.elapsedMs)} ms</span>
              </div>
              <div className="result-stack">
                {suggestQuery.data.suggestions.length === 0 ? (
                  <div className="empty-state">No candidates matched the current limits.</div>
                ) : (
                  suggestQuery.data.suggestions.map((suggestion) => (
                    <div className="result-card" key={suggestion.word}>
                      <div className="result-top">
                        <button className="result-word linkish" type="button" onClick={() => fillWord(suggestion.word)}>
                          {suggestion.word}
                        </button>
                        <span className="confidence">
                          {suggestion.confidence}
                          <small>%</small>
                        </span>
                      </div>
                      <div className="bar">
                        <i style={{ width: `${suggestion.confidence}%` }} />
                      </div>
                      <div className="result-meta">
                        <span>distance {suggestion.distance}</span>
                        <span>{suggestion.reason}</span>
                        <span className={`source ${suggestion.source}`}>{suggestion.source}</span>
                        {suggestion.frequency > 0 && <span>freq {suggestion.frequency}</span>}
                      </div>
                    </div>
                  ))
                )}
              </div>
            </>
          ) : (
            <div className="result-stack empty-state">
              Try{" "}
              {["recieve", "kubernets", "odre"].map((example, index) => (
                <span key={example}>
                  {index > 0 && ", "}
                  <button className="linkish" type="button" onClick={() => fillWord(example)}>
                    {example}
                  </button>
                </span>
              ))}
              .
            </div>
          )}
          <p className="muted tiny-note">
            Confidence combines edit distance, corpus frequency, word shape, keyboard adjacency and source.
          </p>
        </article>

        <article className="card">
          <div className="card-head">
            <div>
              <p className="kicker">PREFIX</p>
              <h2>Complete a partial word</h2>
            </div>
            <span className="pill neutral">{completeQuery.data?.completions.length ?? 0} matches</span>
          </div>
          <form
            className="search-row"
            onSubmit={(event) => {
              event.preventDefault();
              if (prefix.trim()) setSubmittedPrefix(prefix.trim());
              else if (submittedPrefix) completeQuery.refetch();
            }}
          >
            <input
              type="text"
              value={prefix}
              autoComplete="off"
              spellCheck={false}
              placeholder="Start of a word"
              onChange={(event) => setPrefix(event.target.value)}
            />
            <button className="btn primary" type="submit" disabled={!prefix.trim()}>
              {completeQuery.isFetching ? <Loader2 className="spin" size={14} /> : <Search size={14} />}
              Complete
            </button>
          </form>

          {completeQuery.isError ? (
            <div className="error-box">
              <strong>Completion failed.</strong>
              <span>{completeQuery.error.message}</span>
            </div>
          ) : completeQuery.isPending ? (
            <div className="chip-cloud">
              {[0, 1, 2, 3].map((item) => (
                <span className="skeleton chip-skeleton" key={item} />
              ))}
            </div>
          ) : completeQuery.data && completeQuery.data.completions.length > 0 ? (
            <div className="chip-cloud">
              {completeQuery.data.completions.map((completion) => (
                <button
                  className="chip"
                  type="button"
                  key={completion.word}
                  onClick={() => fillWord(completion.word)}
                  title="Use this word in the suggestion box"
                >
                  {completion.word}
                  <b>{completion.source === "user" ? "user" : completion.frequency}</b>
                </button>
              ))}
            </div>
          ) : (
            <div className="chip-cloud empty-state">
              No completions for “{submittedPrefix}”. Prefix completion is powered by the compact trie.
            </div>
          )}
        </article>
      </div>

      <article className="card">
        <div className="card-head">
          <div>
            <p className="kicker">WHY IT RANKS</p>
            <h2>Confidence signals</h2>
          </div>
        </div>
        <div className="signal-grid">
          <div>
            <b>60%</b>
            <span>edit distance</span>
            <p>Damerau–Levenshtein, including adjacent transpositions.</p>
          </div>
          <div>
            <b>27%</b>
            <span>frequency prior</span>
            <p>Corpus frequency keeps likely English words above obscure ones.</p>
          </div>
          <div>
            <b>13%</b>
            <span>shape</span>
            <p>Length similarity prevents tiny words from hijacking the result.</p>
          </div>
          <div>
            <b>+</b>
            <span>adjustments</span>
            <p>QWERTY-adjacent keys and per-user dictionary words get a boost.</p>
          </div>
        </div>
      </article>

      <div className="split">
        <ApiCallViewer
          title="Single Word Suggest API"
          method="POST"
          path="/api/v1/spell/suggest"
          headers={apiKey ? { "X-API-Key": apiKey } : {}}
          body={{
            word: submittedWord || word,
            maxSuggestions: settings.maxSuggestions,
            maxDistance: settings.maxDistance,
            minConfidence: settings.minConfidence,
            includeUserDictionary: includeUser,
          }}
          response={suggestQuery.data}
          description="Inspect and replay the POST /api/v1/spell/suggest endpoint call for the single-word lookup above."
        />

        <ApiCallViewer
          title="Prefix Autocomplete API"
          method="GET"
          path={`/api/v1/spell/complete?prefix=${encodeURIComponent(submittedPrefix || prefix)}&limit=12&includeUserDictionary=${includeUser}`}
          headers={apiKey ? { "X-API-Key": apiKey } : {}}
          response={completeQuery.data}
          description="Inspect and replay the GET /api/v1/spell/complete endpoint call for the prefix autocompletion above."
        />
      </div>
    </>
  );
}

function ResultSkeleton() {
  return (
    <div className="result-stack">
      {[0, 1, 2].map((item) => (
        <div className="result-card" key={item}>
          <div className="skeleton w-35" />
          <div className="skeleton bar-skeleton" />
          <div className="skeleton w-55" />
        </div>
      ))}
    </div>
  );
}
