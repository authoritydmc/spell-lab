use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub db_path: PathBuf,
    pub max_text_chars: usize,
    pub default_max_suggestions: usize,
    pub max_suggestions_limit: usize,
    pub default_max_distance: usize,
    pub max_distance_limit: usize,
    pub default_min_confidence: f64,
    pub suggestion_cache_entries: usize,
    pub public_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let bind = env::var("SPELL_BIND")
            .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
            .parse()
            .context("SPELL_BIND must be a valid socket address")?;

        let db_path = env::var("SPELL_DB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./data/spell.db"));

        Ok(Self {
            bind,
            db_path,
            max_text_chars: env_usize("SPELL_MAX_TEXT_CHARS", 20_000),
            default_max_suggestions: env_usize("SPELL_DEFAULT_MAX_SUGGESTIONS", 5).clamp(0, 20),
            max_suggestions_limit: env_usize("SPELL_MAX_SUGGESTIONS_LIMIT", 20).clamp(1, 50),
            default_max_distance: env_usize("SPELL_DEFAULT_MAX_DISTANCE", 2).clamp(1, 3),
            max_distance_limit: env_usize("SPELL_MAX_DISTANCE_LIMIT", 3).clamp(1, 4),
            default_min_confidence: env_f64("SPELL_DEFAULT_MIN_CONFIDENCE", 0.0).clamp(0.0, 100.0),
            suggestion_cache_entries: env_usize("SPELL_SUGGESTION_CACHE_ENTRIES", 10_000)
                .clamp(0, 200_000),
            public_url: env::var("SPELL_PUBLIC_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
        })
    }
}

fn env_usize(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_f64(key: &str, default: f64) -> f64 {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
