use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GenerationMode {
    #[serde(rename = "local")]
    LocalOnly,
    #[serde(rename = "github")]
    GithubRepo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRequest {
    pub urls: Vec<String>,
    pub mode: GenerationMode,
    pub repo_name: Option<String>,
    pub private_repo: Option<bool>,
    pub custom_name: Option<String>,
    pub lang: Option<String>,
    pub nsfw: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionItem {
    pub id: String,
    pub pkg_id: String,
    pub name: String,
    pub class_name: String,
    pub base_url: String,
    pub lang: String,
    pub ext_type: String, // "anime" | "manga"
    pub version: String,
    pub version_code: i32,
    pub cms: String,
    pub has_sub_dub: bool,
    pub icon_base64: Option<String>,
    pub github_repo_url: Option<String>,
    pub nekoyomi_repo_url: Option<String>,
    pub apk_download_url: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubAccountInfo {
    pub username: String,
    pub display_name: Option<String>,
    pub avatar_url: String,
    pub auth_type: String, // "cli" | "pat"
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String, // "INFO" | "WARN" | "ERROR" | "SUCCESS"
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub app_version: String,
    pub api_port: u16,
    pub is_mobile: bool,
    pub os: String,
    pub active_extensions_count: usize,
    pub gh_logged_in: bool,
    pub gh_user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractorTestResult {
    pub host: String,
    pub success: bool,
    pub stream_url: Option<String>,
    pub quality: Option<String>,
    pub headers: Vec<(String, String)>,
    pub error: Option<String>,
}
