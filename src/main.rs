pub mod config;
pub mod db;
pub mod screenshot;
pub mod ocr;
pub mod nlp;
pub mod platform;
pub mod web;

use clap::Parser;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as TokioMutex;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use tera::Tera;
use tracing::{info, error};

use crate::config::Config;
use crate::nlp::Embedder;
use crate::web::routes::{create_router, AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = Config::parse();

    // Make sure database path and screenshots path exist
    let db_path = config.db_path();
    let screenshots_path = config.screenshots_path();

    info!("Appdata folder: {:?}", config.appdata_folder());
    info!("Database path: {:?}", db_path);
    info!("Screenshots path: {:?}", screenshots_path);

    // Initialize database schema
    db::create_db(&db_path)?;

    // Open a shared connection for the web server (reused across requests)
    let web_conn = db::open_connection(&db_path)?;
    let db_conn = Arc::new(Mutex::new(web_conn));

    // Initialize Embedder
    let embedder = match Embedder::new() {
        Ok(e) => e,
        Err(err) => {
            error!("Failed to load embedder: {}", err);
            return Err(err);
        }
    };
    let embedder_arc = Arc::new(TokioMutex::new(embedder));

    // Compile templates
    let mut tera = Tera::default();
    tera.add_raw_template("base_template.html", include_str!("web/templates/base_template.html"))?;
    tera.add_raw_template("timeline.html", include_str!("web/templates/timeline.html"))?;
    tera.add_raw_template("search.html", include_str!("web/templates/search.html"))?;

    let state = AppState {
        config: config.clone(),
        tera,
        db_conn,
        embedder: embedder_arc.clone(),
    };

    // Spawn the background recording thread with its own long-lived connection
    let record_config = config.clone();
    let record_embedder = embedder_arc.clone();
    tokio::task::spawn_blocking(move || {
        record_screenshots_thread(record_config, record_embedder);
    });

    // Start web server
    let router = create_router(state);
    let port = 8082;
    let addr = format!("0.0.0.0:{}", port);

    info!("Starting server on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}

fn record_screenshots_thread(config: Config, embedder: Arc<TokioMutex<Embedder>>) {
    // Disable tokenizers parallelism to avoid issues in threads
    std::env::set_var("TOKENIZERS_PARALLELISM", "false");

    let db_path = config.db_path();
    let screenshots_path = config.screenshots_path();
    let primary_only = config.primary_monitor_only;

    // Open a single long-lived connection for the recording thread
    let conn = match db::open_connection(&db_path) {
        Ok(c) => c,
        Err(e) => {
            error!("Recording thread failed to open DB connection: {}", e);
            return;
        }
    };

    // Initial screenshots
    let mut last_screenshots = screenshot::take_screenshots(primary_only).unwrap_or_default();

    loop {
        if !platform::is_user_active() {
            std::thread::sleep(Duration::from_secs(3));
            continue;
        }

        let current_screenshots = screenshot::take_screenshots(primary_only).unwrap_or_default();

        if last_screenshots.len() != current_screenshots.len() {
            last_screenshots = current_screenshots;
            std::thread::sleep(Duration::from_secs(3));
            continue;
        }

        // Determine whether to include the monitor index in filenames.
        // This is fixed for the duration of this iteration since the count is stable.
        let multi_monitor = current_screenshots.len() > 1;

        for i in 0..current_screenshots.len() {
            let current_screenshot = &current_screenshots[i];
            let last_screenshot = &last_screenshots[i];

            // threshold matches python 0.9
            if !screenshot::is_similar(current_screenshot, last_screenshot, 0.9) {
                last_screenshots[i] = current_screenshot.clone();

                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                // Include the monitor index in the filename when capturing multiple monitors
                // so that screenshots from different displays don't overwrite each other.
                let filename = if multi_monitor {
                    format!("{}_{}.webp", timestamp, i)
                } else {
                    format!("{}.webp", timestamp)
                };
                let filepath = screenshots_path.join(&filename);

                // Save WebP lossless
                if let Err(e) = current_screenshot.save_with_format(&filepath, image::ImageFormat::WebP) {
                    error!("Failed to save screenshot: {}", e);
                    continue;
                }

                // OCR
                let text = match ocr::extract_text_from_image(current_screenshot) {
                    Ok(t) => t,
                    Err(e) => {
                        error!("OCR failed: {}", e);
                        String::new()
                    }
                };

                if !text.trim().is_empty() {
                    // Embedding
                    let embedding = {
                        let mut emb_guard = embedder.blocking_lock();
                        emb_guard.embed(&text).unwrap_or_else(|_| vec![0.0; 384])
                    };

                    // Convert f32 vec to u8 vec
                    let embedding_bytes: Vec<u8> = embedding.iter()
                        .flat_map(|&f| f.to_ne_bytes().to_vec())
                        .collect();

                    let active_app = platform::active_app_name();
                    let active_app = if active_app.is_empty() { "Unknown App".to_string() } else { active_app };

                    let active_title = platform::active_window_title();
                    let active_title = if active_title.is_empty() { "Unknown Title".to_string() } else { active_title };

                    // Insert to DB using the long-lived recording-thread connection
                    if let Err(e) = db::insert_entry_conn(
                        &conn,
                        &text,
                        timestamp,
                        &embedding_bytes,
                        &active_app,
                        &active_title
                    ) {
                        error!("Failed to insert entry to DB: {}", e);
                    }
                }
            }
        }

        std::thread::sleep(Duration::from_secs(3));
    }
}
