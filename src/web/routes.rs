use axum::{
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tera::{Context, Tera};
use tokio::fs;

use crate::config::Config;
use crate::db::{get_all_entries_conn, get_timestamps_conn, Entry};
use crate::nlp::{cosine_similarity, Embedder};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub tera: Tera,
    /// Shared database connection. Wrapped in `Arc<Mutex<...>>` so web handlers
    /// can reuse a single connection rather than opening a new one per request.
    pub db_conn: Arc<Mutex<Connection>>,
    /// Embedder requires `&mut self` for inference, so it lives behind a tokio Mutex.
    pub embedder: Arc<tokio::sync::Mutex<Embedder>>,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    q: String,
}

#[derive(Serialize)]
struct RenderEntry {
    timestamp: i64,
    app: String,
    title: String,
    text: String,
}

impl From<Entry> for RenderEntry {
    fn from(entry: Entry) -> Self {
        Self {
            timestamp: entry.timestamp,
            app: entry.app,
            title: entry.title,
            text: entry.text,
        }
    }
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(timeline))
        .route("/search", get(search))
        .route("/static/:filename", get(serve_image))
        .with_state(state)
}

async fn timeline(State(state): State<AppState>) -> impl IntoResponse {
    let timestamps = tokio::task::block_in_place(|| {
        match state.db_conn.lock() {
            Ok(conn) => get_timestamps_conn(&conn).unwrap_or_else(|e| {
                tracing::error!("Failed to get timestamps: {}", e);
                vec![]
            }),
            Err(e) => {
                tracing::error!("DB lock poisoned: {}", e);
                vec![]
            }
        }
    });

    let mut context = Context::new();
    context.insert("timestamps", &timestamps);

    match state.tera.render("timeline.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!("Template rendering error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Template error").into_response()
        }
    }
}

async fn search(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    let entries = tokio::task::block_in_place(|| {
        match state.db_conn.lock() {
            Ok(conn) => get_all_entries_conn(&conn).unwrap_or_else(|e| {
                tracing::error!("Failed to get entries: {}", e);
                vec![]
            }),
            Err(e) => {
                tracing::error!("DB lock poisoned: {}", e);
                vec![]
            }
        }
    });

    // Use tokio's block_in_place for synchronous Embedder logic
    let query_embedding = tokio::task::block_in_place(|| {
        let mut embedder = state.embedder.blocking_lock();
        embedder.embed(&query.q).unwrap_or_else(|_| vec![0.0; 384])
    });

    let mut scored_entries: Vec<(f32, Entry)> = entries
        .into_iter()
        .map(|entry| {
            let embedding_f32: Vec<f32> = entry.embedding
                .chunks_exact(4)
                .map(|b| f32::from_ne_bytes(b.try_into().unwrap()))
                .collect();
            let score = cosine_similarity(&query_embedding, &embedding_f32);
            (score, entry)
        })
        .collect();

    // Sort descending by score
    scored_entries.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let render_entries: Vec<RenderEntry> = scored_entries.into_iter().map(|(_, e)| e.into()).collect();

    let mut context = Context::new();
    context.insert("entries", &render_entries);

    match state.tera.render("search.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!("Template rendering error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Template error").into_response()
        }
    }
}

async fn serve_image(
    State(state): State<AppState>,
    AxumPath(filename): AxumPath<String>,
) -> impl IntoResponse {
    // Security: validate filename to prevent path traversal attacks.
    // Only allow filenames consisting of digits, optional underscore+digits, exactly one dot,
    // and a safe extension (e.g. "1678886400.webp" or "1678886400_1.webp").
    let dot_count = filename.chars().filter(|&c| c == '.').count();
    let valid = !filename.is_empty()
        && (filename.ends_with(".webp") || filename.ends_with(".png"))
        && filename
            .chars()
            .all(|c| c.is_ascii_digit() || c == '_' || c == '.')
        && dot_count == 1
        && !filename.contains('/')
        && !filename.contains('\\');

    if !valid {
        return (StatusCode::BAD_REQUEST, "Invalid filename").into_response();
    }

    let screenshots_path = state.config.screenshots_path();
    let filepath = screenshots_path.join(&filename);

    match fs::read(&filepath).await {
        Ok(data) => {
            let content_type = if filename.ends_with(".webp") {
                "image/webp"
            } else if filename.ends_with(".png") {
                "image/png"
            } else {
                "application/octet-stream"
            };

            ([(axum::http::header::CONTENT_TYPE, content_type)], data).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "Image not found").into_response(),
    }
}
