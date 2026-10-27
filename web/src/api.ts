import type {
  AddWordsResponse,
  CheckResponse,
  CompleteResponse,
  CorrectResponse,
  CreateUserResponse,
  DictionaryListResponse,
  HealthResponse,
  MeResponse,
  RotateKeyResponse,
  StatsResponse,
  SuggestResponse,
} from "./types";

const API_BASE = (import.meta.env.VITE_API_BASE || "/api/v1").replace(/\/$/, "");
const KEY_STORAGE = "spelllab.apiKey";

let apiKey: string | null = localStorage.getItem(KEY_STORAGE);

export class ApiError extends Error {
  status: number;
  code?: string;

  constructor(message: string, status: number, code?: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
  }
}

export function getApiKey(): string | null {
  return apiKey;
}

export function setApiKey(next: string | null): void {
  apiKey = next;
  if (next) localStorage.setItem(KEY_STORAGE, next);
  else localStorage.removeItem(KEY_STORAGE);
}

interface RequestOptions {
  method?: "GET" | "POST" | "DELETE";
  body?: unknown;
  signal?: AbortSignal;
}

async function request<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const headers = new Headers();
  if (options.body !== undefined) headers.set("content-type", "application/json");
  if (apiKey) headers.set("x-api-key", apiKey);

  const response = await fetch(`${API_BASE}${path}`, {
    method: options.method || "GET",
    headers,
    body: options.body === undefined ? undefined : JSON.stringify(options.body),
    signal: options.signal,
  });

  let payload: unknown = null;
  try {
    payload = await response.json();
  } catch {
    payload = null;
  }

  if (!response.ok) {
    const errorBody = payload as { error?: { message?: string; code?: string } } | null;
    throw new ApiError(
      errorBody?.error?.message || `Request failed with HTTP ${response.status}`,
      response.status,
      errorBody?.error?.code
    );
  }
  return payload as T;
}

let creating: Promise<CreateUserResponse> | null = null;

export const api = {
  base: API_BASE,
  getApiKey,
  setApiKey,

  health: () => request<HealthResponse>("/health"),
  stats: () => request<StatsResponse>("/stats"),

  createUser(name = "Browser demo"): Promise<CreateUserResponse> {
    if (!creating) {
      creating = request<CreateUserResponse>("/users", {
        method: "POST",
        body: { name },
      }).finally(() => {
        creating = null;
      });
    }
    return creating;
  },

  me: () => request<MeResponse>("/users/me"),
  rotateKey: () => request<RotateKeyResponse>("/users/me/rotate-key", { method: "POST" }),
  deleteUser: () => request<{ deleted: boolean }>("/users/me", { method: "DELETE" }),

  check(
    payload: {
      text: string;
      maxSuggestions: number;
      maxDistance: number;
      minConfidence: number;
      includeUserDictionary: boolean;
      ignoreAcronyms: boolean;
    },
    signal?: AbortSignal
  ) {
    return request<CheckResponse>("/spell/check", { method: "POST", body: payload, signal });
  },

  suggest(payload: {
    word: string;
    maxSuggestions: number;
    maxDistance: number;
    minConfidence: number;
    includeUserDictionary: boolean;
  }) {
    return request<SuggestResponse>("/spell/suggest", { method: "POST", body: payload });
  },

  complete(prefix: string, limit: number, includeUserDictionary: boolean) {
    const params = new URLSearchParams({
      prefix,
      limit: String(limit),
      includeUserDictionary: String(includeUserDictionary),
    });
    return request<CompleteResponse>(`/spell/complete?${params}`);
  },

  correct(payload: {
    text: string;
    maxDistance: number;
    minConfidence: number;
    includeUserDictionary: boolean;
    ignoreAcronyms: boolean;
  }) {
    return request<CorrectResponse>("/spell/correct", { method: "POST", body: payload });
  },

  listWords(limit = 2000, offset = 0) {
    return request<DictionaryListResponse>(`/dictionary?limit=${limit}&offset=${offset}`);
  },

  async exportDictionary(format: "txt" | "csv" | "json" = "txt"): Promise<void> {
    const headers = new Headers();
    if (apiKey) headers.set("x-api-key", apiKey);
    const response = await fetch(`${API_BASE}/dictionary/export?format=${format}`, { headers });
    if (!response.ok) {
      throw new ApiError(`Export failed: HTTP ${response.status}`, response.status);
    }
    const blob = await response.blob();
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `dictionary.${format}`;
    document.body.appendChild(anchor);
    anchor.click();
    document.body.removeChild(anchor);
    URL.revokeObjectURL(url);
  },

  addWords(words: string[]) {
    return request<AddWordsResponse>("/dictionary/words", {
      method: "POST",
      body: { words },
    });
  },

  removeWord(word: string) {
    return request<{ removed: boolean; word: string }>(
      `/dictionary/words/${encodeURIComponent(word)}`,
      { method: "DELETE" }
    );
  },

  clearDictionary() {
    return request<{ removed: number }>("/dictionary", { method: "DELETE" });
  },
};
