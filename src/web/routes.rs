use axum::{
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tera::{Context, Tera};
use tokio::fs;

use crate::config::Config;
use crate::db::{get_all_entries, get_timestamps, Entry};
use crate::nlp::{cosine_similarity, Embedder};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub tera: Tera,
    // Add Embedder inside a Mutex since its embed() method requires `&mut self`
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
    let db_path = state.config.db_path();
    let timestamps = match get_timestamps(&db_path) {
        Ok(ts) => ts,
        Err(e) => {
            tracing::error!("Failed to get timestamps: {}", e);
            vec![]
        }
    };

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
    let db_path = state.config.db_path();
    let entries = match get_all_entries(&db_path) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("Failed to get entries: {}", e);
            vec![]
        }
    };

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
