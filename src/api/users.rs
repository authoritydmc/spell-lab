use axum::{extract::State, http::HeaderMap, Json};
use serde::Deserialize;
use serde_json::json;

use crate::{
    db::StoredUser,
    error::AppError,
    models::{CreateUserResponse, LimitsView, MeResponse, RotateKeyResponse, UserView},
    state::AppState,
};

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserRequest {
    #[serde(default)]
    pub name: Option<String>,
}

pub async fn create_user(
    State(state): State<AppState>,
    body: Option<Json<CreateUserRequest>>,
) -> Result<Json<CreateUserResponse>, AppError> {
    let name = body.and_then(|Json(body)| body.name);
    let db = state.db.clone();
    let created = tokio::task::spawn_blocking(move || {
        let nu = db.create_user(name.clone())?;
        // If this is a demo user or requested, seed useful domain terms
        let is_demo = name.as_deref().map(|n| n.to_lowercase().contains("demo")).unwrap_or(false);
        if is_demo {
            let seed_words = vec![
                "kubernetes".to_string(),
                "traefik".to_string(),
                "fastapi".to_string(),
                "tailscale".to_string(),
                "grafana".to_string(),
                "postgresql".to_string(),
                "grpc".to_string(),
                "redis".to_string(),
                "authentik".to_string(),
                "coolify".to_string(),
                "prometheus".to_string(),
            ];
            let _ = db.add_user_words(&nu.user.id, &seed_words);
        }
        Ok::<_, anyhow::Error>(nu)
    })
    .await
    .map_err(|error| AppError::Internal(error.into()))??;
    Ok(Json(CreateUserResponse {
        user: view(&created.user),
        api_key: created.api_key,
        limits: limits(&state),
        message: "Store this API key now. It cannot be recovered; you can rotate it later."
            .to_string(),
    }))
}

pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MeResponse>, AppError> {
    let user = state.require_user(&headers).await?;
    let db = state.db.clone();
    let user_id = user.id.clone();
    let user_words = tokio::task::spawn_blocking(move || db.user_word_count(&user_id))
        .await
        .map_err(|error| AppError::Internal(error.into()))??;
    Ok(Json(MeResponse {
        user: view(&user),
        user_words,
        limits: limits(&state),
    }))
}

pub async fn rotate_key(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RotateKeyResponse>, AppError> {
    let user = state.require_user(&headers).await?;
    let db = state.db.clone();
    let user_id = user.id.clone();
    let rotated = tokio::task::spawn_blocking(move || db.rotate_api_key(&user_id))
        .await
        .map_err(|error| AppError::Internal(error.into()))??;
    let (user, api_key) = rotated.ok_or_else(|| AppError::not_found("user no longer exists"))?;
    Ok(Json(RotateKeyResponse {
        api_key,
        api_key_prefix: user.api_key_prefix,
        message: "The previous API key is now invalid.".to_string(),
    }))
}

pub async fn delete_me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let user = state.require_user(&headers).await?;
    let db = state.db.clone();
    let user_id = user.id.clone();
    let deleted = tokio::task::spawn_blocking(move || db.delete_user(&user_id))
        .await
        .map_err(|error| AppError::Internal(error.into()))??;
    if !deleted {
        return Err(AppError::not_found("user no longer exists"));
    }
    state.invalidate_user(&user.id);
    Ok(Json(json!({ "deleted": true })))
}

fn view(user: &StoredUser) -> UserView {
    UserView {
        id: user.id.clone(),
        name: user.name.clone(),
        created_at: user.created_at.clone(),
        api_key_prefix: user.api_key_prefix.clone(),
    }
}

fn limits(state: &AppState) -> LimitsView {
    LimitsView {
        max_suggestions: state.config.max_suggestions_limit,
        max_distance: state.config.max_distance_limit,
        max_text_chars: state.config.max_text_chars,
    }
}
