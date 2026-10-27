import { useState } from "react";
import { Check, ChevronDown, ChevronUp, Code2, Copy, Play } from "lucide-react";

interface Props {
  title?: string;
  method: "GET" | "POST" | "DELETE";
  path: string;
  body?: unknown;
  headers?: Record<string, string>;
  response?: unknown;
  description?: string;
}

export function ApiCallViewer({
  title = "Backend API Call",
  method,
  path,
  body,
  headers = {},
  response,
  description,
}: Props) {
  const [open, setOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  const [testing, setTesting] = useState(false);
  const [liveResponse, setLiveResponse] = useState<unknown | null>(null);
  const [liveStatus, setLiveStatus] = useState<number | null>(null);

  const fullUrl = path.startsWith("http")
    ? path
    : `${window.location.origin}${path}`;

  const headerList: Record<string, string> = {
    ...headers,
  };
  if (body !== undefined && !headerList["Content-Type"]) {
    headerList["Content-Type"] = "application/json";
  }

  // Construct curl command
  let curl = `curl -s -X ${method} "${fullUrl}"`;
  for (const [k, v] of Object.entries(headerList)) {
    curl += ` \\\n  -H "${k}: ${v}"`;
  }
  if (body !== undefined) {
    curl += ` \\\n  -d '${typeof body === "string" ? body : JSON.stringify(body, null, 2)}'`;
  }

  const handleCopy = () => {
    navigator.clipboard.writeText(curl);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const executeLive = async () => {
    try {
      setTesting(true);
      setLiveResponse(null);
      setLiveStatus(null);
      const res = await fetch(path, {
        method,
        headers: headerList,
        body: body !== undefined ? (typeof body === "string" ? body : JSON.stringify(body)) : undefined,
      });
      setLiveStatus(res.status);
      let payload: unknown;
      try {
        payload = await res.json();
      } catch {
        payload = await res.text();
      }
      setLiveResponse(payload);
    } catch (err) {
      setLiveResponse({ error: err instanceof Error ? err.message : String(err) });
    } finally {
      setTesting(false);
    }
  };

  return (
    <div className="api-viewer-card">
      <div className="api-viewer-header" onClick={() => setOpen(!open)}>
        <div className="api-viewer-title">
          <Code2 size={15} className="api-icon" />
          <span className={`method-badge ${method.toLowerCase()}`}>{method}</span>
          <span className="api-path">{path}</span>
          <span className="api-subtitle">{title}</span>
        </div>
        <div className="api-viewer-actions" onClick={(e) => e.stopPropagation()}>
          <button
            type="button"
            className="btn ghost tiny"
            onClick={handleCopy}
            title="Copy cURL to clipboard"
          >
            {copied ? <Check size={12} color="#10b981" /> : <Copy size={12} />}
            <span>{copied ? "Copied" : "cURL"}</span>
          </button>
          <button
            type="button"
            className="btn ghost tiny toggle-btn"
            onClick={() => setOpen(!open)}
            aria-label={open ? "Hide API details" : "Show API details"}
          >
            {open ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
          </button>
        </div>
      </div>

      {open && (
        <div className="api-viewer-body">
          {description && <p className="api-desc">{description}</p>}
          <div className="api-tabs">
            <div className="api-section">
              <div className="api-section-head">
                <span className="kicker">cURL Command</span>
                <button
                  type="button"
                  className="btn primary tiny run-btn"
                  onClick={executeLive}
                  disabled={testing}
                >
                  <Play size={10} />
                  <span>{testing ? "Running…" : "Send Request"}</span>
                </button>
              </div>
              <pre className="curl-pre">
                <code>{curl}</code>
              </pre>
            </div>

            {body !== undefined && (
              <div className="api-section">
                <span className="kicker">Request JSON Body</span>
                <pre className="json-pre">
                  <code>{typeof body === "string" ? body : JSON.stringify(body, null, 2)}</code>
                </pre>
              </div>
            )}

            {(liveResponse !== null || response !== undefined) && (
              <div className="api-section">
                <div className="api-section-head">
                  <span className="kicker">
                    {liveResponse !== null
                      ? `Live Execution Response (HTTP ${liveStatus})`
                      : "Latest Engine Response"}
                  </span>
                </div>
                <pre className="json-pre">
                  <code>
                    {JSON.stringify(
                      liveResponse !== null ? liveResponse : response,
                      null,
                      2
                    )}
                  </code>
                </pre>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
