# Osiris → Rust Port Plan

Osiris is a privacy-first, local digital-memory tool (a fork of Osiris) that periodically takes screenshots, runs OCR + semantic embeddings on them, stores everything in SQLite, and exposes a timeline + search web UI. Currently the entire application is written in Python (~1,500 LOC across 7 modules). This plan ports every subsystem to idiomatic Rust while preserving all current functionality and cross-platform support (Linux / macOS / Windows).

> [!IMPORTANT]
> This is a **full rewrite** — the new Rust binary will be a drop-in replacement and read the same SQLite schema and screenshot files as the current Python app.

---

## Proposed Workspace Layout

```
osiris/                          ← existing repo root
├── src/
│   ├── main.rs                  ← CLI entry point (clap)
│   ├── config.rs                ← CLI args + platform paths
│   ├── db.rs                    ← SQLite layer (rusqlite)
│   ├── screenshot.rs            ← screen capture + SSIM dedup
│   ├── ocr.rs                   ← OCR (tesseract-rs or leptess)
│   ├── nlp.rs                   ← Embeddings + cosine similarity (ort / candle)
│   ├── platform/
│   │   ├── mod.rs
│   │   ├── linux.rs             ← xprop / xprintidle
│   │   ├── macos.rs             ← NSWorkspace / ioreg
│   │   └── windows.rs           ← win32api / GetLastInputInfo
│   └── web/
│       ├── mod.rs               ← Axum router
│       ├── routes.rs            ← handlers: timeline, search, static files
│       └── templates/           ← Tera HTML templates (replaces Jinja2)
│           ├── base.html
│           ├── timeline.html
│           └── search.html
├── Cargo.toml
└── (Python files retained for reference until parity is verified)
```

---

## Proposed Changes

### 1 — Project Bootstrap

#### [NEW] [Cargo.toml](file:///home/sprime01/projects/osiris/Cargo.toml)

Create a new `Cargo.toml` at the repo root with the following key dependencies:

| Crate                            | Purpose                          | Replaces                     |
| -------------------------------- | -------------------------------- | ---------------------------- |
| `clap` (derive)                  | CLI arg parsing                  | `argparse`                   |
| `rusqlite` + `bundled` feature   | SQLite access                    | `sqlite3` stdlib             |
| `screenshots` or `xcap`          | Cross-platform screen capture    | `mss`                        |
| `image`                          | Image I/O, WebP save, SSIM       | `PIL`, `numpy`               |
| `leptess` (or `tesseract-rs`)    | OCR                              | `doctr`                      |
| `ort` (ONNX Runtime) or `candle` | Sentence embedding inference     | `sentence-transformers`      |
| `ndarray`                        | Float vectors, dot product       | `numpy`                      |
| `axum`                           | Async HTTP server                | `Flask`                      |
| `tokio` (full)                   | Async runtime                    | Python threads               |
| `tera`                           | Jinja2-compatible HTML templates | `jinja2`                     |
| `serde` + `serde_json`           | JSON serialization               | —                            |
| `tracing` + `tracing-subscriber` | Structured logging               | `logging`                    |
| `dirs`                           | Platform-specific paths          | manual `sys.platform` checks |
| `anyhow`                         | Error handling                   | bare `except`                |

---

### 2 — Config Module

#### [NEW] [src/config.rs](file:///home/sprime01/projects/osiris/src/config.rs)

- Define a `Config` struct (derives `clap::Parser`).
- Two flags: `--storage-path` and `--primary-monitor-only`.
- `Config::appdata_folder()` uses the `dirs` crate (`dirs::data_local_dir()`) — replaces manual `sys.platform` branching in `config.py`.
- Expose `db_path()` and `screenshots_path()` helpers; create directories if missing.

---

### 3 — Database Module

#### [NEW] [src/db.rs](file:///home/sprime01/projects/osiris/src/db.rs)

Direct translation of `database.py` using `rusqlite`:

- `create_db(conn)` — creates `entries` table + `idx_timestamp` index (identical schema).
- `Entry` struct with `id, app, title, text, timestamp, embedding: Vec<u8>`.
- `get_all_entries(conn)` → `Vec<Entry>`.
- `get_timestamps(conn)` → `Vec<i64>`.
- `insert_entry(conn, ...)` — uses `ON CONFLICT(timestamp) DO NOTHING`.
- Embeddings stored as raw `f32` bytes (same as current Python code, so DB is forward/backward compatible).

---

### 4 — Screenshot & Dedup Module

#### [NEW] [src/screenshot.rs](file:///home/sprime01/projects/osiris/src/screenshot.rs)

- `take_screenshots(primary_only: bool) -> Vec<DynamicImage>` using the `screenshots` or `xcap` crate.
- `mean_ssim(img1, img2) -> f64` — pure Rust implementation of the MSSIM formula currently in `screenshot.py` (rgb→grey, means, variances, cross-correlation).
- `is_similar(a, b, threshold: f64) -> bool`.
- `record_loop(config, db_conn)` — async `tokio::task::spawn_blocking` loop replacing the Python thread; calls `is_user_active()`, deduplication, OCR, embedding, and DB insert.
- Images are saved as lossless WebP via the `image` crate.

---

### 5 — Platform Module (Window Info + Idle Detection)

#### [NEW] [src/platform/linux.rs](file:///home/sprime01/projects/osiris/src/platform/linux.rs)

- `active_app_name()` and `active_window_title()`: spawn `xprop` subprocess (same logic as `utils.py`), parse regex with the `regex` crate.
- `is_user_active()`: spawn `xprintidle`, parse ms → bool.

#### [NEW] [src/platform/macos.rs](file:///home/sprime01/projects/osiris/src/platform/macos.rs)

- Use `objc2` / `core-foundation` crates or subprocess calls to `ioreg` / `osascript`.

#### [NEW] [src/platform/windows.rs](file:///home/sprime01/projects/osiris/src/platform/windows.rs)

- Use `windows` crate (`GetForegroundWindow`, `GetWindowTextW`, `GetLastInputInfo`).

#### [NEW] [src/platform/mod.rs](file:///home/sprime01/projects/osiris/src/platform/mod.rs)

- `#[cfg(target_os = …)]` dispatches to each platform module.
- Exports: `active_app_name() -> String`, `active_window_title() -> String`, `is_user_active() -> bool`.

---

### 6 — NLP / Embedding Module

#### [NEW] [src/nlp.rs](file:///home/sprime01/projects/osiris/src/nlp.rs)

> [!IMPORTANT]
> This is the highest-risk subsystem. `sentence-transformers/all-MiniLM-L6-v2` must be exported to ONNX once (a one-time Python script) and shipped alongside the binary. The `ort` crate (ONNX Runtime bindings) runs inference in Rust — no Python at runtime.

- Export script (one-time, `scripts/export_model.py`): `model.save('all-MiniLM-L6-v2-onnx')` using `optimum`.
- `Embedder::new(model_path)` — loads ONNX session via `ort`.
- `embed(text: &str) -> Vec<f32>` — tokenises with `tokenizers` crate (same HuggingFace tokenizer), runs inference, mean-pools across lines exactly as `nlp.py` does.
- `cosine_similarity(a: &[f32], b: &[f32]) -> f32` — pure Rust with `ndarray` or manual dot product.

---

### 7 — Web UI Module

#### [NEW] [src/web/routes.rs](file:///home/sprime01/projects/osiris/src/web/routes.rs)

Axum handlers replacing Flask routes:

| Python route         | Rust handler                                                                               |
| -------------------- | ------------------------------------------------------------------------------------------ |
| `GET /`              | `timeline()` — fetches timestamps, renders `timeline.html` via Tera                        |
| `GET /search?q=`     | `search()` — fetches all entries, computes cosine similarity, sorts, renders `search.html` |
| `GET /static/<file>` | `serve_image()` — `axum::extract::Path`, reads WebP from `screenshots_path`                |

#### [NEW] [src/web/templates/](file:///home/sprime01/projects/osiris/src/web/templates/)

Port the inline Jinja2 HTML from `app.py` to standalone Tera templates. Functionally identical markup; Bootstrap 4 CDN links retained.

#### [NEW] [src/main.rs](file:///home/sprime01/projects/osiris/src/main.rs)

- Parse `Config` with `clap`.
- Call `create_db`.
- Spawn `record_loop` as a `tokio` background task.
- Start Axum server on `0.0.0.0:8082`.

---

## Phased Execution Strategy

| Phase                      | Scope                              | Exit Criterion                                          |
| -------------------------- | ---------------------------------- | ------------------------------------------------------- |
| **1 — Scaffold**           | `Cargo.toml`, `config.rs`, `db.rs` | `cargo build` succeeds; DB created                      |
| **2 — Screenshot + Dedup** | `screenshot.rs`                    | Screenshots saved to disk; SSIM matches Python          |
| **3 — OCR**                | `ocr.rs`                           | Text extracted from test PNG                            |
| **4 — NLP**                | `nlp.rs` + export script           | Embeddings within 1e-4 of Python reference              |
| **5 — Platform**           | `platform/`                        | `is_user_active` and window title work on Linux         |
| **6 — Web UI**             | `web/`                             | Browser renders timeline and search at `localhost:8082` |
| **7 — Integration**        | `main.rs` wires everything         | End-to-end loop runs; existing `tests/` pass            |

---

## Verification Plan

### Automated Tests

All existing Python tests in `tests/` will be **rewritten** in Rust as integration tests in `tests/` directory (Rust convention):

| Python test file   | Rust equivalent          | Command             |
| ------------------ | ------------------------ | ------------------- |
| `test_config.py`   | `tests/test_config.rs`   | `cargo test config` |
| `test_database.py` | `tests/test_database.rs` | `cargo test db`     |
| `test_nlp.py`      | `tests/test_nlp.rs`      | `cargo test nlp`    |

Run all Rust tests:

```bash
cd /home/sprime01/projects/osiris
cargo test
```

### Embedding Parity Check

A temporary Python ↔ Rust numerical comparison script (`scripts/verify_embeddings.py`) will:

1. Generate an embedding for a fixed sentence in Python (`nlp.py`).
2. Call the Rust binary with `--embed "sentence"` and compare output.
3. Assert `max(|a - b|) < 1e-4`.

### Manual Verification

1. Build and run: `cargo run -- --storage-path /tmp/osiris-test`
2. Open `http://localhost:8082` — should show "Nothing recorded yet" initially.
3. Wait ~10s; timeline slider should appear with a screenshot.
4. Type any word visible in a recent screenshot into the search bar; relevant thumbnails should rank first.
5. Confirm WebP files exist in `/tmp/osiris-test/screenshots/`.
6. Confirm `recall.db` has rows: `sqlite3 /tmp/osiris-test/recall.db "SELECT count(*) FROM entries;"`.
