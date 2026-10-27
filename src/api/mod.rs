pub mod dictionary;
pub mod health;
pub mod spell;
pub mod users;

use axum::{
    extract::DefaultBodyLimit,
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde_json::json;
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/", get(api_index))
        .route("/health", get(health::health))
        .route("/metrics", get(health::metrics))
        .route("/stats", get(health::stats))
        .route("/users", post(users::create_user))
        .route("/users/me", get(users::me).delete(users::delete_me))
        .route("/users/me/rotate-key", post(users::rotate_key))
        .route(
            "/dictionary",
            get(dictionary::list).delete(dictionary::clear),
        )
        .route("/dictionary/export", get(dictionary::export))
        .route("/dictionary/words", post(dictionary::add_words))
        .route("/dictionary/import", post(dictionary::add_words))
        .route("/dictionary/words/{word}", delete(dictionary::remove_word))
        .route("/spell/check", post(spell::check))
        .route("/spell/suggest", post(spell::suggest_word))
        .route("/spell/complete", get(spell::complete))
        .route("/spell/correct", post(spell::correct));

    Router::new()
        .route("/", get(api_index))
        .nest("/api/v1", api)
        .route("/healthz", get(health::health))
        .fallback(not_found)
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .with_state(state)
}

async fn api_index() -> impl IntoResponse {
    Json(json!({
        "name": "spell-lab",
        "version": env!("CARGO_PKG_VERSION"),
        "apiVersion": "v1",
        "description": "Confidence-ranked spellcheck API with per-user dictionaries",
        "endpoints": {
            "health": "GET /api/v1/health",
            "metrics": "GET /api/v1/metrics",
            "stats": "GET /api/v1/stats",
            "createUser": "POST /api/v1/users",
            "me": "GET /api/v1/users/me",
            "dictionary": "GET|DELETE /api/v1/dictionary",
            "addWords": "POST /api/v1/dictionary/words",
            "removeWord": "DELETE /api/v1/dictionary/words/{word}",
            "check": "POST /api/v1/spell/check",
            "suggest": "POST /api/v1/spell/suggest",
            "complete": "GET /api/v1/spell/complete?prefix=spel",
            "correct": "POST /api/v1/spell/correct"
        },
        "auth": "Send X-API-Key: <key> or Authorization: Bearer <key> for user dictionaries"
    }))
}

async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": {
                "code": "not_found",
                "message": "the requested resource does not exist"
            }
        })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{Method, Request, StatusCode},
    };
    use serde_json::{json, Value};
    use std::path::PathBuf;
    use tempfile::TempDir;
    use tower::ServiceExt;

    use crate::{config::Config, db::Db, engine::Lexicon, state::AppState};

    fn test_state() -> (AppState, TempDir) {
        let dir = TempDir::new().expect("temp dir");
        let config = Config {
            bind: "127.0.0.1:0".parse().expect("bind"),
            db_path: PathBuf::from(dir.path()).join("test.db"),
            max_text_chars: 20_000,
            default_max_suggestions: 5,
            max_suggestions_limit: 20,
            default_max_distance: 2,
            max_distance_limit: 3,
            default_min_confidence: 0.0,
            suggestion_cache_entries: 128,
            public_url: "http://test.local".to_string(),
        };
        let db = Db::open(&config.db_path).expect("db");
        let global = Lexicon::from_entries(
            vec![
                ("the".to_string(), 1_000_000),
                ("quick".to_string(), 100_000),
                ("brown".to_string(), 80_000),
                ("fox".to_string(), 60_000),
                ("order".to_string(), 50_000),
                ("spell".to_string(), 20_000),
            ],
            false,
        );
        (AppState::new(config, db, global), dir)
    }

    async fn send(
        app: &Router,
        method: Method,
        uri: &str,
        body: Option<Value>,
        key: Option<&str>,
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder().method(method).uri(uri);
        if body.is_some() {
            builder = builder.header("content-type", "application/json");
        }
        if let Some(key) = key {
            builder = builder.header("x-api-key", key);
        }
        let body = body
            .map(|value| Body::from(value.to_string()))
            .unwrap_or_else(Body::empty);
        let response = app
            .clone()
            .oneshot(builder.body(body).expect("request"))
            .await
            .expect("response");
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).expect("json")
        };
        (status, value)
    }

    #[tokio::test]
    async fn health_and_ranked_suggestion() {
        let (state, _dir) = test_state();
        let app = router(state);

        let (status, health) = send(&app, Method::GET, "/api/v1/health", None, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(health["status"], "ok");
        assert_eq!(health["globalWords"], 6);

        let (status, suggestion) = send(
            &app,
            Method::POST,
            "/api/v1/spell/suggest",
            Some(json!({"word": "teh", "maxSuggestions": 5, "maxDistance": 2})),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(suggestion["suggestions"][0]["word"], "the");
        assert_eq!(suggestion["suggestions"][0]["reason"], "transposition");
    }

    #[tokio::test]
    async fn user_dictionary_is_isolated_and_rotates() {
        let (state, _dir) = test_state();
        let app = router(state);

        let (status, created) = send(
            &app,
            Method::POST,
            "/api/v1/users",
            Some(json!({"name": "test"})),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let key = created["apiKey"].as_str().expect("key").to_string();

        let (status, added) = send(
            &app,
            Method::POST,
            "/api/v1/dictionary/words",
            Some(json!({"words": ["kubernets"]})),
            Some(&key),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(added["added"], 1);

        let (status, checked) = send(
            &app,
            Method::POST,
            "/api/v1/spell/check",
            Some(json!({"text": "kubernets", "maxSuggestions": 3})),
            Some(&key),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(checked["tokens"][0]["correct"], true);
        assert_eq!(checked["tokens"][0]["inUserDictionary"], true);

        let (status, rotated) = send(
            &app,
            Method::POST,
            "/api/v1/users/me/rotate-key",
            None,
            Some(&key),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let new_key = rotated["apiKey"].as_str().expect("new key").to_string();

        let (status, _) = send(&app, Method::GET, "/api/v1/users/me", None, Some(&key)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let (status, _) = send(&app, Method::GET, "/api/v1/users/me", None, Some(&new_key)).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_bad_requests_with_envelopes() {
        let (state, _dir) = test_state();
        let app = router(state);

        let (status, body) = send(
            &app,
            Method::POST,
            "/api/v1/spell/suggest",
            Some(json!({"word": "!!!"})),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "bad_request");

        let (status, body) = send(&app, Method::GET, "/does-not-exist", None, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
    }
}
