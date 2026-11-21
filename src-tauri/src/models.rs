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
    pub ext_type: Option<String>,
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
    /// Estado de la última compilación: "building" | "pending" | "success" | "error".
    #[serde(default)]
    pub build_status: Option<String>,
    #[serde(default)]
    pub build_error: Option<String>,
    #[serde(default)]
    pub build_apk_path: Option<String>,
    /// URL del run de GitHub Actions que compila esta extensión (modo GitHub).
    #[serde(default)]
    pub actions_run_url: Option<String>,
    /// SHA del commit que AEXT subió; permite volver a consultar ese run.
    #[serde(default)]
    pub actions_commit_sha: Option<String>,
    /// Icono publicado en `gh-pages/icon/<pkg>.png` para variantes adoptadas
    /// desde el índice (no llevan `icon_base64` generado localmente).
    #[serde(default)]
    pub icon_url: Option<String>,
    /// Estado del módulo en el repo: "present" | "missing". `None` significa
    /// que no se ha comprobado (o que la extensión es solo local).
    #[serde(default)]
    pub repo_state: Option<String>,
    /// Momento en que se comprobó por última vez el módulo en GitHub.
    #[serde(default)]
    pub repo_checked_at: Option<String>,
    /// Momento en que el módulo dejó de existir en el repositorio.
    #[serde(default)]
    pub repo_deleted_at: Option<String>,
    /// El usuario restauró la ficha aun sabiendo que el módulo no está en el
    /// repo. Se respeta hasta que el módulo vuelve a aparecer.
    #[serde(default)]
    pub repo_ignore_missing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoCheckResult {
    pub checked: usize,
    pub present: usize,
    pub missing: usize,
    pub errors: usize,
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
    pub mcp_endpoint: String,
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

// ==================== MCP & MANUAL PATCH MODELS ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCallRequest {
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCallResponse {
    pub success: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiDiagnosticReport {
    pub app_version: String,
    pub platform: String,
    pub recent_logs: Vec<LogEntry>,
    pub failed_sites: Vec<String>,
    pub system_info: SystemStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiPatchRequest {
    pub pkg_id: String,
    pub custom_card_selector: Option<String>,
    pub custom_title_selector: Option<String>,
    pub custom_next_selector: Option<String>,
    pub custom_video_code: Option<String>,
    pub custom_headers: Option<Vec<(String, String)>>,
}
