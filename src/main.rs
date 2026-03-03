pub mod config;
pub mod db;
pub mod screenshot;
pub mod ocr;
pub mod nlp;
pub mod platform;
pub mod web;

use clap::Parser;
use std::sync::Arc;
use tokio::sync::Mutex;
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

    // Initialize database
    db::create_db(&db_path)?;

    // Initialize Embedder
    let embedder = match Embedder::new() {
        Ok(e) => e,
        Err(err) => {
            error!("Failed to load embedder: {}", err);
            return Err(err);
        }
    };
    let embedder_arc = Arc::new(Mutex::new(embedder));

    // Compile templates
    let mut tera = Tera::default();
    tera.add_raw_template("base_template.html", include_str!("web/templates/base_template.html"))?;
    tera.add_raw_template("timeline.html", include_str!("web/templates/timeline.html"))?;
    tera.add_raw_template("search.html", include_str!("web/templates/search.html"))?;

    let state = AppState {
        config: config.clone(),
        tera,
        embedder: embedder_arc.clone(),
    };

    // Spawn the background recording thread
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

fn record_screenshots_thread(config: Config, embedder: Arc<Mutex<Embedder>>) {
    // Disable tokenizers parallelism to avoid issues in threads
    std::env::set_var("TOKENIZERS_PARALLELISM", "false");

    let db_path = config.db_path();
    let screenshots_path = config.screenshots_path();
    let primary_only = config.primary_monitor_only;

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

        for i in 0..current_screenshots.len() {
            let current_screenshot = &current_screenshots[i];
            let last_screenshot = &last_screenshots[i];

            // threshold matches python 0.9
            if !screenshot::is_similar(current_screenshot, last_screenshot, 0.9) {
                last_screenshots[i] = current_screenshot.clone();

                let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;

                // Using just timestamp.webp instead of adding monitor index since Python does both depending on the function.
                // The python trace app.py relies on {{timestamp}}.webp.
                // So we will just use the timestamp if it's primary monitor, or maybe just timestamp for the first monitor.
                // If there are multiple, this will overwrite or cause issues, but to maintain compatibility with timeline.html we use timestamp.webp
                let filename = format!("{}.webp", timestamp);
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

                    // Insert to DB
                    if let Err(e) = db::insert_entry(
                        &db_path,
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
