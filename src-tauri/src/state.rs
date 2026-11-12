use crate::models::{ExtensionItem, LogEntry, GithubAccountInfo};
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub extensions: Arc<RwLock<Vec<ExtensionItem>>>,
    pub logs: Arc<RwLock<VecDeque<LogEntry>>>,
    pub gh_user: Arc<RwLock<Option<GithubAccountInfo>>>,
    pub storage_dir: PathBuf,
}

impl AppState {
    pub fn new() -> Self {
        let storage_dir = get_storage_path();
        let _ = fs::create_dir_all(&storage_dir);

        let extensions = Self::load_persisted_extensions(&storage_dir);

        Self {
            extensions: Arc::new(RwLock::new(extensions)),
            logs: Arc::new(RwLock::new(VecDeque::with_capacity(500))),
            gh_user: Arc::new(RwLock::new(None)),
            storage_dir,
        }
    }

    fn load_persisted_extensions(dir: &PathBuf) -> Vec<ExtensionItem> {
        let file = dir.join("aext_extensions.json");
        if file.exists() {
            if let Ok(content) = fs::read_to_string(&file) {
                if let Ok(items) = serde_json::from_str::<Vec<ExtensionItem>>(&content) {
                    return items;
                }
            }
        }
        Vec::new()
    }

    pub async fn persist_extensions(&self) {
        let items = self.extensions.read().await;
        let file = self.storage_dir.join("aext_extensions.json");
        if let Ok(json) = serde_json::to_string_pretty(&*items) {
            let _ = fs::write(file, json);
        }
    }

    pub async fn add_log(&self, level: &str, message: &str) {
        let entry = LogEntry {
            timestamp: chrono::Utc::now().format("%H:%M:%S").to_string(),
            level: level.to_string(),
            message: message.to_string(),
        };

        let mut logs = self.logs.write().await;
        if logs.len() >= 500 {
            logs.pop_front();
        }
        logs.push_back(entry);
    }
}

fn get_storage_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("AEXT");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".aext");
    }
    PathBuf::from("./aext_data")
}
