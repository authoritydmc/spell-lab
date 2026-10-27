import { useEffect } from "react";
import { Check, Copy, KeyRound, Loader2, RotateCw, Trash2, X } from "lucide-react";
import type { UserView } from "../types";

interface Props {
  open: boolean;
  apiKey: string | null;
  identity: UserView | null;
  rotating: boolean;
  deleting: boolean;
  onClose: () => void;
  onRotate: () => void;
  onDelete: () => void;
  notify: (message: string, error?: boolean) => void;
}

export function IdentityModal({
  open,
  apiKey,
  identity,
  rotating,
  deleting,
  onClose,
  onRotate,
  onDelete,
  notify,
}: Props) {
  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  const copyKey = async () => {
    if (!apiKey) return;
    try {
      await navigator.clipboard.writeText(apiKey);
      notify("API key copied to the clipboard.");
    } catch {
      notify("Clipboard permission denied. Select the key and copy it manually.", true);
    }
  };

  return (
    <div className="modal" role="presentation">
      <div className="modal-backdrop" onClick={onClose} />
      <div className="modal-card" role="dialog" aria-modal="true" aria-labelledby="identityTitle">
        <div className="card-head">
          <div>
            <p className="kicker">API IDENTITY</p>
            <h2 id="identityTitle">Your key</h2>
          </div>
          <button className="icon-btn" type="button" onClick={onClose} aria-label="Close">
            <X size={15} />
          </button>
        </div>

        <p className="muted">
          This browser stores the key in <code>localStorage</code>. The server only stores
          a SHA-256 hash and never returns the raw key after creation or rotation.
        </p>

        <div className="identity-summary">
          <div>
            <span className="kicker">LABEL</span>
            <strong>{identity?.name ?? "creating…"}</strong>
          </div>
          <div>
            <span className="kicker">PREFIX</span>
            <strong>{identity?.apiKeyPrefix ?? "—"}</strong>
          </div>
        </div>

        <div className="key-display">
          <code>{apiKey ?? "No key available yet"}</code>
          <button className="btn ghost small" type="button" onClick={copyKey} disabled={!apiKey}>
            <Copy size={12} /> Copy
          </button>
        </div>

        <div className="key-hints">
          <span>
            <Check size={11} /> Use as <code>x-api-key</code>
          </span>
          <span>
            <KeyRound size={11} /> Rotating invalidates the old key
          </span>
        </div>

        <div className="modal-actions">
          <button
            className="btn primary small"
            type="button"
            onClick={onRotate}
            disabled={!apiKey || rotating || deleting}
          >
            {rotating ? <Loader2 className="spin" size={12} /> : <RotateCw size={12} />}
            {rotating ? "Rotating…" : "Rotate key"}
          </button>
          <button
            className="btn danger small"
            type="button"
            disabled={deleting || rotating}
            onClick={() => {
              if (
                window.confirm(
                  "Delete this identity and every word in its dictionary? A fresh identity will be created."
                )
              ) {
                onDelete();
              }
            }}
          >
            {deleting ? <Loader2 className="spin" size={12} /> : <Trash2 size={12} />}
            {deleting ? "Deleting…" : "Delete identity"}
          </button>
        </div>
      </div>
    </div>
  );
}
