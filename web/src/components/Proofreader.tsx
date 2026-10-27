import { useEffect, useMemo, useState } from "react";
import { keepPreviousData, useMutation, useQuery } from "@tanstack/react-query";
import { Eraser, Loader2, RotateCcw, Sparkles, Wand2 } from "lucide-react";
import { api } from "../api";
import { useDebouncedValue } from "../hooks";
import type { EngineSettings, TokenView } from "../types";
import { ApiCallViewer } from "./ApiCallViewer";

const DEMO_TEXT =
  "Teh quick brown fox jumps over teh lazy dog. I recieve alot of email, " +
  "and my faourite word is kubernets. Please put the files in odre before lunch.";

interface Props {
  settings: EngineSettings;
  onSettingsChange: (settings: EngineSettings) => void;
  apiKey: string | null;
  notify: (message: string, error?: boolean) => void;
  onLatency: (ms: number) => void;
}

export function Proofreader({ settings, onSettingsChange, apiKey, notify, onLatency }: Props) {
  const [text, setText] = useState(DEMO_TEXT);
  const [selectedToken, setSelectedToken] = useState<number | null>(null);
  const debouncedText = useDebouncedValue(text, 300);

  const checkQuery = useQuery({
    queryKey: ["check", debouncedText, settings, Boolean(apiKey)],
    queryFn: ({ signal }) =>
      api.check(
        {
          text: debouncedText,
          maxSuggestions: settings.maxSuggestions,
          maxDistance: settings.maxDistance,
          minConfidence: settings.minConfidence,
          includeUserDictionary: settings.includeUserDictionary && Boolean(apiKey),
          ignoreAcronyms: settings.ignoreAcronyms,
        },
        signal
      ),
    enabled: debouncedText.trim().length > 0,
    placeholderData: keepPreviousData,
    staleTime: 15_000,
    retry: false,
  });

  const data = checkQuery.data;
  const isStale = Boolean(data && data.text !== text);
  const checking = checkQuery.isFetching || isStale || (text !== debouncedText && Boolean(text.trim()));

  useEffect(() => {
    if (data) onLatency(data.stats.elapsedMs);
  }, [data, onLatency]);

  const correctMutation = useMutation({
    mutationFn: () =>
      api.correct({
        text,
        maxDistance: settings.maxDistance,
        minConfidence: Math.max(85, settings.minConfidence),
        includeUserDictionary: settings.includeUserDictionary && Boolean(apiKey),
        ignoreAcronyms: settings.ignoreAcronyms,
      }),
    onSuccess: (result) => {
      if (result.changes.length === 0) {
        notify("No corrections met the safety threshold.");
        return;
      }
      setText(result.corrected);
      setSelectedToken(null);
      notify(`${result.changes.length} correction${result.changes.length === 1 ? "" : "s"} applied.`);
    },
    onError: (error) => notify(error.message, true),
  });

  const issues = useMemo(() => (data?.tokens ?? []).filter((token) => !token.correct), [data]);

  const applySuggestion = (index: number, replacement: string) => {
    if (!data) return;
    if (data.text !== text) {
      notify("Still checking the latest text. Try again in a moment.", true);
      return;
    }
    const token = data.tokens[index];
    if (!token) return;
    const chars = Array.from(text);
    chars.splice(token.start, token.end - token.start, ...Array.from(replacement));
    setText(chars.join(""));
    setSelectedToken(null);
    notify(`Replaced “${token.raw}” with “${replacement}”`);
  };

  const updateSetting = <K extends keyof EngineSettings>(key: K, value: EngineSettings[K]) => {
    onSettingsChange({ ...settings, [key]: value });
  };

  return (
    <>
      <div className="split">
        <article className="card editor-card">
          <div className="card-head">
            <div>
              <p className="kicker">LIVE</p>
              <h2>Paste or type text</h2>
            </div>
            <div className="inline-actions">
              <button className="btn ghost small" type="button" onClick={() => setText(DEMO_TEXT)}>
                <Sparkles size={12} /> Load demo
              </button>
              <button className="btn ghost small" type="button" onClick={() => setText("")}>
                <Eraser size={12} /> Clear
              </button>
            </div>
          </div>
          <textarea
            value={text}
            spellCheck={false}
            placeholder='Start typing… e.g. "Teh quick brown fox jumped over the odre dog."'
            onChange={(event) => setText(event.target.value)}
            onKeyDown={(event) => {
              if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
                event.preventDefault();
                checkQuery.refetch();
              }
            }}
          />
          <div className="editor-foot">
            <span>{Array.from(text).length.toLocaleString()} characters</span>
            <span>{checking ? "checking…" : "checks run automatically"}</span>
          </div>
        </article>

        <article className="card preview-card">
          <div className="card-head">
            <div>
              <p className="kicker">ANALYSIS</p>
              <h2>Annotated text</h2>
            </div>
            <SummaryPill
              checking={checking}
              issues={issues.length}
              correct={data?.stats.correctWords ?? 0}
              latency={data?.stats.elapsedMs}
            />
          </div>

          {checkQuery.isError ? (
            <div className="error-box">
              <strong>Check failed.</strong>
              <span>{checkQuery.error.message}</span>
              <button className="btn ghost small" type="button" onClick={() => checkQuery.refetch()}>
                Retry
              </button>
            </div>
          ) : !text.trim() ? (
            <div className="preview empty">Your annotated text will appear here.</div>
          ) : !data ? (
            <PreviewSkeleton />
          ) : (
            <AnnotatedText
              text={data.text}
              tokens={data.tokens}
              selected={selectedToken}
              onSelect={setSelectedToken}
            />
          )}

          <div className="legend">
            <span>
              <i className="legend-dot bad" /> needs attention
            </span>
            <span>
              <i className="legend-dot ok" /> known word
            </span>
            <span>
              <i className="legend-dot muted" /> ignored
            </span>
            {data && (
              <span>
                {data.stats.misspelledWords} misspelled · {data.stats.ignoredWords} ignored
              </span>
            )}
          </div>
        </article>
      </div>

      <div className="settings-row">
        <article className="card settings-card">
          <div className="card-head compact">
            <div>
              <p className="kicker">TUNING</p>
              <h2>Engine settings</h2>
            </div>
            <button
              className="btn ghost small"
              type="button"
              onClick={() =>
                onSettingsChange({
                  maxSuggestions: 5,
                  maxDistance: 2,
                  minConfidence: 0,
                  includeUserDictionary: true,
                  ignoreAcronyms: true,
                })
              }
            >
              <RotateCcw size={12} /> Reset
            </button>
          </div>
          <div className="settings-grid">
            <label className="field">
              <span>
                maxSuggestions <output>{settings.maxSuggestions}</output>
              </span>
              <input
                type="range"
                min="0"
                max="20"
                step="1"
                value={settings.maxSuggestions}
                onChange={(event) => updateSetting("maxSuggestions", Number(event.target.value))}
              />
            </label>
            <label className="field">
              <span>
                maxDistance <output>{settings.maxDistance}</output>
              </span>
              <input
                type="range"
                min="1"
                max="3"
                step="1"
                value={settings.maxDistance}
                onChange={(event) => updateSetting("maxDistance", Number(event.target.value))}
              />
            </label>
            <label className="field">
              <span>
                minConfidence <output>{settings.minConfidence}%</output>
              </span>
              <input
                type="range"
                min="0"
                max="100"
                step="1"
                value={settings.minConfidence}
                onChange={(event) => updateSetting("minConfidence", Number(event.target.value))}
              />
            </label>
            <div className="toggle-stack">
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={settings.includeUserDictionary}
                  onChange={(event) =>
                    updateSetting("includeUserDictionary", event.target.checked)
                  }
                />
                <span /> Use my dictionary
              </label>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={settings.ignoreAcronyms}
                  onChange={(event) => updateSetting("ignoreAcronyms", event.target.checked)}
                />
                <span /> Ignore acronyms
              </label>
            </div>
          </div>
        </article>

        <article className="card issue-card">
          <div className="card-head compact">
            <div>
              <p className="kicker">SUGGESTIONS</p>
              <h2>Misspellings</h2>
            </div>
            <button
              className="btn primary small"
              type="button"
              disabled={!text.trim() || correctMutation.isPending}
              onClick={() => correctMutation.mutate()}
            >
              {correctMutation.isPending ? <Loader2 className="spin" size={13} /> : <Wand2 size={13} />}
              {correctMutation.isPending ? "Applying…" : "Apply safe fixes"}
            </button>
          </div>

          {!data ? (
            <div className="issue-list empty-state">Suggestions appear after the first check.</div>
          ) : issues.length === 0 ? (
            <div className="issue-list empty-state">No misspellings found. Try adding a typo.</div>
          ) : (
            <div className="issue-list">
              {issues.map((issue) => {
                const index = data.tokens.indexOf(issue);
                return (
                  <IssueCard
                    key={`${issue.start}-${issue.raw}`}
                    issue={issue}
                    index={index}
                    selected={selectedToken === index}
                    disabled={isStale || correctMutation.isPending}
                    onSelect={() => setSelectedToken(index)}
                    onApply={applySuggestion}
                  />
                );
              })}
            </div>
          )}
        </article>
      </div>

      <ApiCallViewer
        title="Check Text API Call"
        method="POST"
        path="/api/v1/spell/check"
        headers={apiKey ? { "X-API-Key": apiKey } : {}}
        body={{
          text: debouncedText || text,
          maxSuggestions: settings.maxSuggestions,
          maxDistance: settings.maxDistance,
          minConfidence: settings.minConfidence,
          includeUserDictionary: settings.includeUserDictionary && Boolean(apiKey),
          ignoreAcronyms: settings.ignoreAcronyms,
        }}
        response={data}
        description="Inspect and replay the exact REST API payload submitted to the spellchecker for the text above."
      />
    </>
  );
}

function SummaryPill({
  checking,
  issues,
  correct,
  latency,
}: {
  checking: boolean;
  issues: number;
  correct: number;
  latency?: number;
}) {
  if (checking) {
    return (
      <span className="pill neutral">
        <Loader2 className="spin" size={12} /> checking
      </span>
    );
  }
  if (issues === 0) {
    return (
      <span className="pill good">
        {correct} words · clean
      </span>
    );
  }
  return (
    <span className="pill warn">
      {issues} issue{issues === 1 ? "" : "s"} · {latency ? `${Math.round(latency)} ms` : "ready"}
    </span>
  );
}

function PreviewSkeleton() {
  return (
    <div className="preview skeleton-preview" aria-label="Analysing text">
      <div className="skeleton w-90" />
      <div className="skeleton w-75" />
      <div className="skeleton w-84" />
      <div className="skeleton w-60" />
    </div>
  );
}

function AnnotatedText({
  text,
  tokens,
  selected,
  onSelect,
}: {
  text: string;
  tokens: TokenView[];
  selected: number | null;
  onSelect: (index: number) => void;
}) {
  const chars = Array.from(text);
  const nodes: React.ReactNode[] = [];
  let cursor = 0;

  tokens.forEach((token, index) => {
    if (token.start > cursor) {
      nodes.push(chars.slice(cursor, token.start).join(""));
    }
    if (token.correct) {
      nodes.push(
        <span
          key={`${token.start}-${index}`}
          className={`token ${token.ignored ? "ignored" : "correct"}`}
          title={token.ignored ? "ignored" : "known word"}
        >
          {token.raw}
        </span>
      );
    } else {
      const top = token.suggestions[0];
      nodes.push(
        <button
          key={`${token.start}-${index}`}
          type="button"
          className={`token typo ${selected === index ? "selected" : ""}`}
          data-token-index={index}
          title={top ? `${top.confidence}% · ${top.word} · ${top.reason}` : "no suggestion"}
          onClick={() => {
            onSelect(index);
            document
              .querySelector(`[data-issue="${index}"]`)
              ?.scrollIntoView({ block: "nearest", behavior: "smooth" });
          }}
        >
          {token.raw}
        </button>
      );
    }
    cursor = token.end;
  });

  if (cursor < chars.length) nodes.push(chars.slice(cursor).join(""));

  return <div className="preview">{nodes}</div>;
}

function IssueCard({
  issue,
  index,
  selected,
  disabled,
  onSelect,
  onApply,
}: {
  issue: TokenView;
  index: number;
  selected: boolean;
  disabled: boolean;
  onSelect: () => void;
  onApply: (index: number, replacement: string) => void;
}) {
  const top = issue.suggestions[0];
  return (
    <div
      className={`issue ${selected ? "selected" : ""}`}
      data-issue={index}
      onClick={onSelect}
    >
      <div>
        <div className="issue-word">
          <strong>{issue.raw}</strong>
          <span className="badge">
            {top
              ? `${top.reason} · ${top.distance} edit${top.distance === 1 ? "" : "s"}`
              : "unresolved"}
          </span>
          {issue.inUserDictionary && <span className="source user">user</span>}
        </div>
        <div className="result-meta">
          {issue.normalized}
          {top ? ` · top confidence ${top.confidence}%` : ""}
        </div>
      </div>
      <div className="suggestions">
        {issue.suggestions.length === 0 ? (
          <span className="badge">no candidate inside limits</span>
        ) : (
          issue.suggestions.map((suggestion) => (
            <button
              key={suggestion.word}
              type="button"
              className="suggestion-btn"
              title={`${suggestion.reason} · distance ${suggestion.distance}`}
              disabled={disabled}
              onClick={(event) => {
                event.stopPropagation();
                onApply(index, suggestion.word);
              }}
            >
              {suggestion.word}
              <b>{suggestion.confidence}%</b>
              <span className={`source ${suggestion.source}`}>{suggestion.source}</span>
            </button>
          ))
        )}
      </div>
    </div>
  );
}
