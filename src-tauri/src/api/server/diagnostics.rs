use crate::models::{AiDiagnosticReport, SystemStatus};
use crate::state::AppState;
use axum::{
    extract::State,
    response::{IntoResponse, Json, Response},
};

use super::app_version;

pub(super) async fn get_ai_diagnostics_handler(State(state): State<AppState>) -> Response {
    let logs = state.logs.read().await;
    let recent_logs: Vec<_> = logs.iter().cloned().collect();
    let ext_count = state.extensions.read().await.len();
    let gh = state.gh_user.read().await;

    let failed_sites: Vec<String> = recent_logs
        .iter()
        .filter(|l| l.level == "ERROR" || l.level == "WARN")
        .map(|l| l.message.clone())
        .collect();

    let report = AiDiagnosticReport {
        app_version: app_version().to_string(),
        platform: if cfg!(target_os = "android") {
            "android".to_string()
        } else {
            "desktop".to_string()
        },
        recent_logs,
        failed_sites,
        system_info: SystemStatus {
            app_version: app_version().to_string(),
            api_port: 45731,
            mcp_endpoint: "http://localhost:45731/mcp".to_string(),
            is_mobile: cfg!(target_os = "android"),
            os: std::env::consts::OS.to_string(),
            active_extensions_count: ext_count,
            gh_logged_in: gh.is_some(),
            gh_user: gh.as_ref().map(|u| u.username.clone()),
        },
    };

    Json(report).into_response()
}
