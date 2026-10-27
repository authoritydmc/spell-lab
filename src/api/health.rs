use std::fs;

use axum::{extract::State, Json};

use crate::{
    error::AppError,
    models::{
        HealthResponse, ProcessMemoryView, StatsResponse, SystemMetricsResponse, API_VERSION,
    },
    state::AppState,
};

fn read_process_memory() -> ProcessMemoryView {
    let mut resident_mb = 0.0;
    let mut peak_resident_mb = 0.0;
    let mut virtual_mb = 0.0;
    let mut threads = 1;

    if let Ok(content) = fs::read_to_string("/proc/self/status") {
        for line in content.lines() {
            if line.starts_with("VmRSS:") {
                if let Some(val) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = val.parse::<f64>() {
                        resident_mb = (kb / 1024.0 * 100.0).round() / 100.0;
                    }
                }
            } else if line.starts_with("VmHWM:") {
                if let Some(val) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = val.parse::<f64>() {
                        peak_resident_mb = (kb / 1024.0 * 100.0).round() / 100.0;
                    }
                }
            } else if line.starts_with("VmSize:") {
                if let Some(val) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = val.parse::<f64>() {
                        virtual_mb = (kb / 1024.0 * 100.0).round() / 100.0;
                    }
                }
            } else if line.starts_with("Threads:") {
                if let Some(val) = line.split_whitespace().nth(1) {
                    if let Ok(th) = val.parse::<u32>() {
                        threads = th;
                    }
                }
            }
        }
    }

    ProcessMemoryView {
        resident_mb,
        peak_resident_mb,
        virtual_mb,
        threads,
    }
}

fn read_system_load() -> [f64; 3] {
    let mut load = [0.0, 0.0, 0.0];
    if let Ok(content) = fs::read_to_string("/proc/loadavg") {
        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() >= 3 {
            load[0] = parts[0].parse().unwrap_or(0.0);
            load[1] = parts[1].parse().unwrap_or(0.0);
            load[2] = parts[2].parse().unwrap_or(0.0);
        }
    }
    load
}

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

pub async fn metrics(
    State(state): State<AppState>,
) -> Result<Json<SystemMetricsResponse>, AppError> {
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

    Ok(Json(SystemMetricsResponse {
        version: env!("CARGO_PKG_VERSION"),
        api_version: API_VERSION,
        uptime_seconds: state.uptime_seconds(),
        memory: read_process_memory(),
        system_load: read_system_load(),
        global_words: state.global.len(),
        users,
        user_words,
        suggestion_cache: crate::models::CacheStatsView {
            entries,
            capacity,
            hits,
            misses,
            hit_rate,
        },
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
        memory: read_process_memory(),
        system_load: read_system_load(),
        public_url: state.config.public_url.clone(),
    }))
}
