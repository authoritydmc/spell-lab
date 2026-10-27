use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Instant,
};

use anyhow::Result;
use axum::http::HeaderMap;
use parking_lot::RwLock;

use crate::{
    config::Config,
    db::{Db, StoredUser},
    engine::{Lexicon, Suggestion},
    error::AppError,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Db,
    pub global: Arc<Lexicon>,
    pub started: Instant,
    user_cache: Arc<RwLock<HashMap<String, Arc<Lexicon>>>>,
    suggestion_cache: Arc<RwLock<HashMap<String, Vec<Suggestion>>>>,
    suggestion_hits: Arc<AtomicU64>,
    suggestion_misses: Arc<AtomicU64>,
}

impl AppState {
    pub fn new(config: Config, db: Db, global: Lexicon) -> Self {
        Self {
            config: Arc::new(config),
            db,
            global: Arc::new(global),
            started: Instant::now(),
            user_cache: Arc::new(RwLock::new(HashMap::new())),
            suggestion_cache: Arc::new(RwLock::new(HashMap::new())),
            suggestion_hits: Arc::new(AtomicU64::new(0)),
            suggestion_misses: Arc::new(AtomicU64::new(0)),
        }
    }

    pub async fn optional_user(&self, headers: &HeaderMap) -> Result<Option<StoredUser>, AppError> {
        let Some(api_key) = extract_api_key(headers) else {
            return Ok(None);
        };
        let db = self.db.clone();
        let user = tokio::task::spawn_blocking(move || db.user_by_api_key(&api_key))
            .await
            .map_err(|error| AppError::Internal(error.into()))??;
        Ok(Some(user.ok_or(AppError::Unauthorized)?))
    }

    pub async fn require_user(&self, headers: &HeaderMap) -> Result<StoredUser, AppError> {
        self.optional_user(headers)
            .await?
            .ok_or(AppError::Unauthorized)
    }

    pub async fn user_lexicon(&self, user_id: &str) -> Result<Arc<Lexicon>, AppError> {
        if let Some(lexicon) = self.user_cache.read().get(user_id).cloned() {
            return Ok(lexicon);
        }

        let db = self.db.clone();
        let cache_key = user_id.to_string();
        let user_id = user_id.to_string();
        let lexicon = tokio::task::spawn_blocking(move || -> Result<Arc<Lexicon>> {
            let words = db.all_user_words(&user_id)?;
            let entries = words.into_iter().map(|word| (word, 0u32)).collect();
            Ok(Arc::new(Lexicon::from_entries(entries, true)))
        })
        .await
        .map_err(|error| AppError::Internal(error.into()))??;

        self.user_cache.write().insert(cache_key, lexicon.clone());
        Ok(lexicon)
    }

    pub fn invalidate_user(&self, user_id: &str) {
        self.user_cache.write().remove(user_id);
    }

    /// Global suggestion cache. User dictionaries are merged after this cache,
    /// so user-specific data is never shared between callers.
    pub fn cached_global_suggestions(
        &self,
        query: &str,
        max_suggestions: usize,
        max_distance: usize,
        min_confidence: f64,
    ) -> Vec<Suggestion> {
        if self.config.suggestion_cache_entries == 0 || max_suggestions == 0 {
            self.suggestion_misses.fetch_add(1, Ordering::Relaxed);
            let request_count = (max_suggestions * 2).clamp(1, 20);
            return self
                .global
                .suggest(query, request_count, max_distance, min_confidence);
        }

        let key = format!(
            "{query}\u{1f}{max_suggestions}\u{1f}{max_distance}\u{1f}{}",
            min_confidence.round() as u16
        );
        if let Some(cached) = self.suggestion_cache.read().get(&key) {
            self.suggestion_hits.fetch_add(1, Ordering::Relaxed);
            return cached.clone();
        }
        self.suggestion_misses.fetch_add(1, Ordering::Relaxed);

        let request_count = (max_suggestions * 2).clamp(1, 20);
        let suggestions = self
            .global
            .suggest(query, request_count, max_distance, min_confidence);
        let mut cache = self.suggestion_cache.write();
        if cache.len() >= self.config.suggestion_cache_entries {
            if let Some(oldest) = cache.keys().next().cloned() {
                cache.remove(&oldest);
            }
        }
        cache.insert(key, suggestions.clone());
        suggestions
    }

    pub fn suggestion_cache_stats(&self) -> (usize, usize, u64, u64) {
        (
            self.suggestion_cache.read().len(),
            self.config.suggestion_cache_entries,
            self.suggestion_hits.load(Ordering::Relaxed),
            self.suggestion_misses.load(Ordering::Relaxed),
        )
    }

    pub fn user_cache_len(&self) -> usize {
        self.user_cache.read().len()
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.started.elapsed().as_secs()
    }
}

pub fn extract_api_key(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        let value = value.trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    let value = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())?
        .trim();
    let token = value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))?
        .trim();
    (!token.is_empty()).then(|| token.to_string())
}
