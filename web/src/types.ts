export interface UserView {
  id: string;
  name: string;
  createdAt: string;
  apiKeyPrefix: string;
}

export interface Limits {
  maxSuggestions: number;
  maxDistance: number;
  maxTextChars: number;
}

export interface CreateUserResponse {
  user: UserView;
  apiKey: string;
  limits: Limits;
  message: string;
}

export interface MeResponse {
  user: UserView;
  userWords: number;
  limits: Limits;
}

export interface RotateKeyResponse {
  apiKey: string;
  apiKeyPrefix: string;
  message: string;
}

export interface Suggestion {
  word: string;
  distance: number;
  confidence: number;
  frequency: number;
  source: "global" | "user";
  reason: string;
}

export interface TokenView {
  raw: string;
  normalized: string;
  start: number;
  end: number;
  byteStart: number;
  byteEnd: number;
  correct: boolean;
  ignored: boolean;
  inUserDictionary: boolean;
  suggestions: Suggestion[];
}

export interface CheckStats {
  totalTokens: number;
  correctWords: number;
  misspelledWords: number;
  ignoredWords: number;
  suggestionCount: number;
  textChars: number;
  elapsedMs: number;
}

export interface DictionarySnapshot {
  globalWords: number;
  userWords: number;
  userDictionaryIncluded: boolean;
}

export interface CheckResponse {
  text: string;
  tokens: TokenView[];
  stats: CheckStats;
  dictionary: DictionarySnapshot;
}

export interface SuggestResponse {
  word: string;
  correct: boolean;
  userDictionaryIncluded: boolean;
  suggestions: Suggestion[];
  elapsedMs: number;
}

export interface Completion {
  word: string;
  frequency: number;
  source: "global" | "user";
}

export interface CompleteResponse {
  prefix: string;
  completions: Completion[];
}

export interface CorrectionChange {
  start: number;
  end: number;
  original: string;
  replacement: string;
  confidence: number;
}

export interface CorrectResponse {
  original: string;
  corrected: string;
  changes: CorrectionChange[];
  elapsedMs: number;
}

export interface DictionaryListResponse {
  words: string[];
  total: number;
  limit: number;
  offset: number;
}

export interface AddWordsResponse {
  added: number;
  duplicates: number;
  invalid: number;
  total: number;
  rejected: string[];
}

export interface HealthResponse {
  status: string;
  version: string;
  apiVersion: string;
  uptimeSeconds: number;
  globalWords: number;
  users: number;
}

export interface CacheStats {
  entries: number;
  capacity: number;
  hits: number;
  misses: number;
  hitRate: number;
}

export interface StatsResponse {
  version: string;
  apiVersion: string;
  globalWords: number;
  frequencyEntries: number;
  users: number;
  userWords: number;
  uptimeSeconds: number;
  algorithms: string[];
  suggestionCache: CacheStats;
  userLexiconCacheEntries: number;
  publicUrl: string;
}

export interface EngineSettings {
  maxSuggestions: number;
  maxDistance: number;
  minConfidence: number;
  includeUserDictionary: boolean;
  ignoreAcronyms: boolean;
}
