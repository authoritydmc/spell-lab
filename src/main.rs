mod api;
mod config;
mod db;
mod engine;
mod error;
mod models;
mod state;

use std::time::Instant;

use anyhow::Result;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::{config::Config, db::Db, engine::Lexicon, state::AppState};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info")),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;
    let db = Db::open(&config.db_path)?;

    tracing::info!("loading global English lexicon (this happens once at startup)");
    let started = Instant::now();
    let global = Lexicon::load_global()?;
    tracing::info!(
        words = global.len(),
        trie_nodes = global.trie_nodes(),
        bk_nodes = global.bk_nodes(),
        frequency_entries = global.frequency_count(),
        elapsed_ms = started.elapsed().as_millis(),
        "global lexicon ready"
    );

    let state = AppState::new(config.clone(), db, global);
    let app = api::router(state);

    let listener = TcpListener::bind(config.bind).await?;
    tracing::info!(address = %config.bind, "spell-lab listening");
    tracing::info!(url = %config.public_url, "public URL");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
