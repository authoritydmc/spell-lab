import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Database, Download, FileUp, Loader2, Plus, RefreshCw, Trash2, Upload } from "lucide-react";
import { api } from "../api";
import type { UserView } from "../types";

interface Props {
  apiKey: string | null;
  identity: UserView | null;
  notify: (message: string, error?: boolean) => void;
  onCountChange: (count: number) => void;
  onOpenIdentity: () => void;
}

export function DictionaryPanel({
  apiKey,
  identity,
  notify,
  onCountChange,
  onOpenIdentity,
}: Props) {
  const queryClient = useQueryClient();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [newWord, setNewWord] = useState("");
  const [importText, setImportText] = useState("kubernetes\ngrafana\npostgresql");
  const [exporting, setExporting] = useState(false);

  const dictionaryQuery = useQuery({
    queryKey: ["dictionary", apiKey],
    queryFn: () => api.listWords(2000),
    enabled: Boolean(apiKey),
    staleTime: 30_000,
    retry: false,
  });

  useEffect(() => {
    if (dictionaryQuery.data) onCountChange(dictionaryQuery.data.total);
  }, [dictionaryQuery.data, onCountChange]);

  const invalidateAll = () => {
    queryClient.invalidateQueries({ queryKey: ["dictionary", apiKey] });
    queryClient.invalidateQueries({ queryKey: ["check"] });
    queryClient.invalidateQueries({ queryKey: ["suggest"] });
    queryClient.invalidateQueries({ queryKey: ["complete"] });
  };

  const addMutation = useMutation({
    mutationFn: (words: string[]) => api.addWords(words),
    onSuccess: (result, words) => {
      const pieces = [
        `${result.added} added`,
        result.duplicates ? `${result.duplicates} duplicate` : "",
        result.invalid ? `${result.invalid} invalid` : "",
      ].filter(Boolean);
      notify(`${pieces.join(" · ")}. Dictionary now has ${result.total} words.`);
      if (words.length === 1) setNewWord("");
      if (words.length > 1) setImportText("");
      invalidateAll();
    },
    onError: (error) => notify(error.message, true),
  });

  const removeMutation = useMutation({
    mutationFn: (word: string) => api.removeWord(word),
    onSuccess: (result) => {
      notify(result.removed ? `Removed “${result.word}”.` : `“${result.word}” was not present.`);
      invalidateAll();
    },
    onError: (error) => notify(error.message, true),
  });

  const clearMutation = useMutation({
    mutationFn: () => api.clearDictionary(),
    onSuccess: (result) => {
      notify(`Cleared ${result.removed} personal word${result.removed === 1 ? "" : "s"}.`);
      invalidateAll();
    },
    onError: (error) => notify(error.message, true),
  });

  const addOne = (event: React.FormEvent) => {
    event.preventDefault();
    const word = newWord.trim();
    if (!word) return;
    addMutation.mutate([word]);
  };

  const importWords = (event: React.FormEvent) => {
    event.preventDefault();
    const words = importText
      .split(/[\s,]+/)
      .map((word) => word.trim())
      .filter(Boolean);
    if (words.length === 0) {
      notify("Paste at least one word.", true);
      return;
    }
    addMutation.mutate(words);
  };

  const handleExport = async (format: "txt" | "csv" | "json" = "txt") => {
    try {
      setExporting(true);
      await api.exportDictionary(format);
      notify(`Exported dictionary as .${format}`);
    } catch (err) {
      notify(err instanceof Error ? err.message : "Failed to export", true);
    } finally {
      setExporting(false);
    }
  };

  const handleFileUpload = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = (e) => {
      const text = e.target?.result as string;
      if (!text) return;
      let words: string[] = [];
      if (file.name.endsWith(".json")) {
        try {
          const parsed = JSON.parse(text);
          words = Array.isArray(parsed) ? parsed : (parsed.words || []);
        } catch {
          notify("Invalid JSON file", true);
          return;
        }
      } else {
        words = text
          .split(/[\r\n,]+/)
          .map((w) => w.trim())
          .filter(Boolean);
      }
      if (words.length === 0) {
        notify("No words found in file", true);
        return;
      }
      notify(`Importing ${words.length} words from file…`);
      addMutation.mutate(words);
      if (fileInputRef.current) fileInputRef.current.value = "";
    };
    reader.readAsText(file);
  };

  return (
    <div className="split wide-left">
      <article className="card">
        <div className="card-head">
          <div>
            <p className="kicker">PERSONAL VOCABULARY</p>
            <h2>My dictionary</h2>
          </div>
          <span className="pill">
            <Database size={11} />
            <b>{dictionaryQuery.data?.total ?? 0}</b> words
          </span>
        </div>
        <p className="muted">
          Words added here are checked before the global English lexicon and appear in
          suggestions with a user-source boost. They are isolated by API key.
        </p>

        <form className="search-row" onSubmit={addOne}>
          <input
            type="text"
            value={newWord}
            autoComplete="off"
            spellCheck={false}
            placeholder="Add a word, e.g. kubernetes"
            onChange={(event) => setNewWord(event.target.value)}
          />
          <button className="btn primary" type="submit" disabled={!newWord.trim() || addMutation.isPending || !apiKey}>
            {addMutation.isPending ? <Loader2 className="spin" size={14} /> : <Plus size={14} />}
            Add word
          </button>
        </form>

        {!apiKey ? (
          <div className="empty-state">Creating your per-browser identity…</div>
        ) : dictionaryQuery.isError ? (
          <div className="error-box">
            <strong>Could not load your dictionary.</strong>
            <span>{dictionaryQuery.error.message}</span>
            <button className="btn ghost small" type="button" onClick={() => dictionaryQuery.refetch()}>
              Retry
            </button>
          </div>
        ) : dictionaryQuery.isPending ? (
          <div className="word-list">
            {[0, 1, 2, 3, 4, 5, 6].map((item) => (
              <span className="skeleton word-skeleton" key={item} />
            ))}
          </div>
        ) : dictionaryQuery.data && dictionaryQuery.data.words.length > 0 ? (
          <>
            <div className="word-list">
              {dictionaryQuery.data.words.map((word) => (
                <span className="word-chip" key={word}>
                  {word}
                  <button
                    type="button"
                    aria-label={`Remove ${word}`}
                    title={`Remove ${word}`}
                    disabled={removeMutation.isPending}
                    onClick={() => removeMutation.mutate(word)}
                  >
                    ×
                  </button>
                </span>
              ))}
            </div>
            {dictionaryQuery.data.total > dictionaryQuery.data.words.length && (
              <p className="muted tiny-note">
                Showing {dictionaryQuery.data.words.length} of {dictionaryQuery.data.total} words.
              </p>
            )}
          </>
        ) : (
          <div className="word-list empty-state">
            No personal words yet. Add a project name, acronym or product term.
          </div>
        )}

        <div className="card-actions">
          <button
            className="btn ghost small"
            type="button"
            onClick={() => dictionaryQuery.refetch()}
            disabled={dictionaryQuery.isFetching}
          >
            <RefreshCw size={12} className={dictionaryQuery.isFetching ? "spin" : ""} /> Refresh
          </button>
          <div className="btn-group">
            <button
              className="btn ghost small"
              type="button"
              disabled={exporting || !dictionaryQuery.data?.total}
              onClick={() => handleExport("txt")}
              title="Export as plain text"
            >
              {exporting ? <Loader2 size={12} className="spin" /> : <Download size={12} />} Export .txt
            </button>
            <button
              className="btn ghost small"
              type="button"
              disabled={exporting || !dictionaryQuery.data?.total}
              onClick={() => handleExport("csv")}
              title="Export as CSV"
            >
              .csv
            </button>
            <button
              className="btn ghost small"
              type="button"
              disabled={exporting || !dictionaryQuery.data?.total}
              onClick={() => handleExport("json")}
              title="Export as JSON"
            >
              .json
            </button>
          </div>
          <button
            className="btn danger small"
            type="button"
            disabled={clearMutation.isPending || !dictionaryQuery.data?.total}
            onClick={() => {
              if (window.confirm("Delete every word in your personal dictionary?")) {
                clearMutation.mutate();
              }
            }}
          >
            {clearMutation.isPending ? <Loader2 className="spin" size={12} /> : <Trash2 size={12} />}
            Clear dictionary
          </button>
        </div>
      </article>

      <article className="card">
        <div className="card-head">
          <div>
            <p className="kicker">BULK</p>
            <h2>Import words</h2>
          </div>
        </div>
        <p className="muted">
          One word per line, or comma-separated. Invalid tokens are reported; valid ones
          are added in one transaction.
        </p>
        <form className="stack-form" onSubmit={importWords}>
          <textarea
            spellCheck={false}
            value={importText}
            placeholder={"kubernetes\ngrafana\npostgresql"}
            onChange={(event) => setImportText(event.target.value)}
          />
          <div className="button-row">
            <button
              className="btn primary"
              type="submit"
              disabled={!importText.trim() || addMutation.isPending || !apiKey}
            >
              {addMutation.isPending ? <Loader2 className="spin" size={14} /> : <Plus size={14} />}
              Import words
            </button>
            <input
              type="file"
              ref={fileInputRef}
              style={{ display: "none" }}
              accept=".txt,.csv,.json"
              onChange={handleFileUpload}
            />
            <button
              className="btn ghost"
              type="button"
              disabled={addMutation.isPending || !apiKey}
              onClick={() => fileInputRef.current?.click()}
              title="Upload text, csv, or json file"
            >
              <FileUp size={14} /> Upload file
            </button>
          </div>
        </form>

        <div className="identity-box">
          <div>
            <p className="kicker">YOUR IDENTITY</p>
            <strong>{identity?.name ?? "creating…"}</strong>
            <small>{identity ? `${identity.apiKeyPrefix}…` : "—"}</small>
          </div>
          <button className="btn ghost small" type="button" onClick={onOpenIdentity}>
            Manage key
          </button>
        </div>
      </article>
    </div>
  );
}
