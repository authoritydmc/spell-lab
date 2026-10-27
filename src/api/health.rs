use axum::{extract::State, Json};

use crate::{
    error::AppError,
    models::{HealthResponse, StatsResponse, API_VERSION},
    state::AppState,
};

pub async fn health(State(state): State<AppState>) -> Result<Json<HealthResponse>, AppError> {
    let db = state.db.clone();
    let (users, _) = tokio::task::spawn_blocking(move || db.counts())
        .await
        .map_err(|error| AppError::Internal(error.into()))??;
    Ok(Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        api_version: API_VERSION,
        uptime_seconds: state.uptime_seconds(),
        global_words: state.global.len(),
        users,
    }))
}

pub async fn stats(State(state): State<AppState>) -> Result<Json<StatsResponse>, AppError> {
    let db = state.db.clone();
    let (users, user_words) = tokio::task::spawn_blocking(move || db.counts())
        .await
        .map_err(|error| AppError::Internal(error.into()))??;
    let (entries, capacity, hits, misses) = state.suggestion_cache_stats();
    let hit_rate = if hits + misses > 0 {
        hits as f64 / (hits + misses) as f64
    } else {
        0.0
    };
    Ok(Json(StatsResponse {
        version: env!("CARGO_PKG_VERSION"),
        api_version: API_VERSION,
        global_words: state.global.len(),
        frequency_entries: state.global.frequency_count(),
        users,
        user_words,
        uptime_seconds: state.uptime_seconds(),
        algorithms: vec![
            "bloom-filter",
            "compact-trie",
            "levenshtein-automaton",
            "bk-tree",
            "damerau-levenshtein",
            "qwerty-weighted-ranking",
            "frequency-prior",
            "suggestion-cache",
            "user-lexicon-cache",
        ],
        suggestion_cache: crate::models::CacheStatsView {
            entries,
            capacity,
            hits,
            misses,
            hit_rate,
        },
        user_lexicon_cache_entries: state.user_cache_len(),
        public_url: state.config.public_url.clone(),
    }))
}
