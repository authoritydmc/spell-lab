use serde::{Deserialize, Serialize};

pub const API_VERSION: &str = "v1";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserView {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub api_key_prefix: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitsView {
    pub max_suggestions: usize,
    pub max_distance: usize,
    pub max_text_chars: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserResponse {
    pub user: UserView,
    pub api_key: String,
    pub limits: LimitsView,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RotateKeyResponse {
    pub api_key: String,
    pub api_key_prefix: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeResponse {
    pub user: UserView,
    pub user_words: usize,
    pub limits: LimitsView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionView {
    pub word: String,
    pub distance: usize,
    pub confidence: u8,
    pub frequency: u32,
    pub source: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenView {
    pub raw: String,
    pub normalized: String,
    pub start: usize,
    pub end: usize,
    pub byte_start: usize,
    pub byte_end: usize,
    pub correct: bool,
    pub ignored: bool,
    pub in_user_dictionary: bool,
    pub suggestions: Vec<SuggestionView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionarySnapshot {
    pub global_words: usize,
    pub user_words: usize,
    pub user_dictionary_included: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckStats {
    pub total_tokens: usize,
    pub correct_words: usize,
    pub misspelled_words: usize,
    pub ignored_words: usize,
    pub suggestion_count: usize,
    pub text_chars: usize,
    pub elapsed_ms: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResponse {
    pub text: String,
    pub tokens: Vec<TokenView>,
    pub stats: CheckStats,
    pub dictionary: DictionarySnapshot,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckRequest {
    pub text: String,
    #[serde(default)]
    pub max_suggestions: Option<usize>,
    #[serde(default)]
    pub max_distance: Option<usize>,
    #[serde(default)]
    pub min_confidence: Option<f64>,
    #[serde(default = "default_true")]
    pub include_user_dictionary: bool,
    #[serde(default = "default_true")]
    pub ignore_acronyms: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestRequest {
    pub word: String,
    #[serde(default)]
    pub max_suggestions: Option<usize>,
    #[serde(default)]
    pub max_distance: Option<usize>,
    #[serde(default)]
    pub min_confidence: Option<f64>,
    #[serde(default = "default_true")]
    pub include_user_dictionary: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestResponse {
    pub word: String,
    pub correct: bool,
    pub user_dictionary_included: bool,
    pub suggestions: Vec<SuggestionView>,
    pub elapsed_ms: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteQuery {
    pub prefix: String,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default = "default_true")]
    pub include_user_dictionary: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionView {
    pub word: String,
    pub frequency: u32,
    pub source: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteResponse {
    pub prefix: String,
    pub completions: Vec<CompletionView>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectRequest {
    pub text: String,
    #[serde(default)]
    pub max_distance: Option<usize>,
    #[serde(default)]
    pub min_confidence: Option<f64>,
    #[serde(default = "default_true")]
    pub include_user_dictionary: bool,
    #[serde(default = "default_true")]
    pub ignore_acronyms: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectionChange {
    pub start: usize,
    pub end: usize,
    pub original: String,
    pub replacement: String,
    pub confidence: u8,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectResponse {
    pub original: String,
    pub corrected: String,
    pub changes: Vec<CorrectionChange>,
    pub elapsed_ms: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddWordsRequest {
    #[serde(default)]
    pub words: Vec<String>,
    #[serde(default)]
    pub text: Option<String>,
}

impl AddWordsRequest {
    pub fn into_words(self) -> Vec<String> {
        let mut words = self.words;
        if let Some(text) = self.text {
            words.extend(
                text.split(|c: char| c.is_whitespace() || c == ',')
                    .map(str::trim)
                    .filter(|w| !w.is_empty())
                    .map(ToOwned::to_owned),
            );
        }
        words
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddWordsResponse {
    pub added: usize,
    pub duplicates: usize,
    pub invalid: usize,
    pub total: usize,
    pub rejected: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryListResponse {
    pub words: Vec<String>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearDictionaryResponse {
    pub removed: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub api_version: &'static str,
    pub uptime_seconds: u64,
    pub global_words: usize,
    pub users: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStatsView {
    pub entries: usize,
    pub capacity: usize,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessMemoryView {
    pub resident_mb: f64,
    pub peak_resident_mb: f64,
    pub virtual_mb: f64,
    pub threads: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemMetricsResponse {
    pub version: &'static str,
    pub api_version: &'static str,
    pub uptime_seconds: u64,
    pub memory: ProcessMemoryView,
    pub system_load: [f64; 3],
    pub global_words: usize,
    pub users: u64,
    pub user_words: u64,
    pub suggestion_cache: CacheStatsView,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsResponse {
    pub version: &'static str,
    pub api_version: &'static str,
    pub global_words: usize,
    pub frequency_entries: usize,
    pub users: u64,
    pub user_words: u64,
    pub uptime_seconds: u64,
    pub algorithms: Vec<&'static str>,
    pub suggestion_cache: CacheStatsView,
    pub user_lexicon_cache_entries: usize,
    pub memory: ProcessMemoryView,
    pub system_load: [f64; 3],
    pub public_url: String,
}
