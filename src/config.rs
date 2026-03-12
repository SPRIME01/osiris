use clap::Parser;
use dirs;
use std::path::PathBuf;
use std::fs;
use std::env;

#[derive(Parser, Debug, Clone)]
#[command(name = "osiris", version = "0.8", about = "Osiris")]
pub struct Config {
    #[arg(long, help = "Path to store the screenshots and database")]
    pub storage_path: Option<PathBuf>,

    #[arg(long, default_value_t = false, help = "Only record the primary monitor")]
    pub primary_monitor_only: bool,
}

impl Config {
    pub fn appdata_folder(&self) -> PathBuf {
        if let Some(path) = &self.storage_path {
            return path.clone();
        }

        let app_name = "osiris";

        let path = if cfg!(target_os = "windows") {
            if let Ok(appdata) = env::var("APPDATA") {
                PathBuf::from(appdata).join(app_name)
            } else {
                panic!("APPDATA environment variable is not set.");
            }
        } else if cfg!(target_os = "macos") {
            dirs::home_dir().expect("Failed to get home dir").join("Library").join("Application Support").join(app_name)
        } else {
            dirs::data_local_dir().unwrap_or_else(|| dirs::home_dir().expect("Failed to get home dir").join(".local").join("share")).join(app_name)
        };

        if !path.exists() {
            fs::create_dir_all(&path).expect("Failed to create appdata folder");
        }

        path
    }

    pub fn db_path(&self) -> PathBuf {
        self.appdata_folder().join("recall.db")
    }

    pub fn screenshots_path(&self) -> PathBuf {
        let path = self.appdata_folder().join("screenshots");
        if !path.exists() {
            let _ = fs::create_dir_all(&path);
        }
        path
    }
}
