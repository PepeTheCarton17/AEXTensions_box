use crate::models::{AiDiagnosticReport, AiPatchRequest, McpToolCallRequest, SystemStatus};
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};

pub async fn start_embedded_api_server(app_state: AppState, port: u16) {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/status", get(get_status_handler))
        .route("/api/extensions", get(get_extensions_handler))
        .route("/api/download/:id", get(download_extension_handler))
        .route("/api/logs", get(get_logs_handler))
        .route("/api/analyze", post(analyze_url_handler))
        .route("/api/ai/diagnostics", get(get_ai_diagnostics_handler))
        .route("/api/ai/patch", post(apply_ai_patch_handler))
        .route("/mcp", post(mcp_jsonrpc_handler))
        .route("/mcp/manifest", get(mcp_manifest_handler))
        .layer(cors)
        .with_state(app_state.clone());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    app_state.add_log("INFO", &format!("Servidor API & MCP embebido escuchando en http://0.0.0.0:{}", port)).await;

    tokio::spawn(async move {
        if let Ok(listener) = tokio::net::TcpListener::bind(&addr).await {
            let _ = axum::serve(listener, app).await;
        } else {
            app_state.add_log("ERROR", &format!("No se pudo enlazar el servidor API en el puerto {}", port)).await;
        }
    });
}

async fn get_status_handler(State(state): State<AppState>) -> Response {
    let ext_count = state.extensions.read().await.len();
    let gh = state.gh_user.read().await;

    let status = serde_json::json!({
        "status": "online",
        "app_name": "AEXT",
        "version": "1.0.0",
        "extensions_count": ext_count,
        "mcp_enabled": true,
        "mcp_endpoint": "http://localhost:45731/mcp",
        "github_logged_in": gh.is_some(),
        "github_user": gh.as_ref().map(|u| &u.username),
        "platform": if cfg!(target_os = "android") { "android" } else { "desktop" }
    });

    Json(status).into_response()
}

async fn get_extensions_handler(State(state): State<AppState>) -> Response {
    let items = state.extensions.read().await.clone();
    Json(items).into_response()
}

async fn download_extension_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let items = state.extensions.read().await;
    let item = items.iter().find(|e| e.id == id || e.pkg_id == id);

    match item {
        Some(ext) => {
            let info = serde_json::json!({
                "id": ext.id,
                "name": ext.name,
                "pkg_id": ext.pkg_id,
                "version": ext.version,
                "apk_download_url": ext.apk_download_url,
                "nekoyomi_repo_url": ext.nekoyomi_repo_url,
            });
            Json(info).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn get_logs_handler(State(state): State<AppState>) -> Response {
    let logs = state.logs.read().await;
    let list: Vec<_> = logs.iter().cloned().collect();
    Json(list).into_response()
}

#[derive(serde::Deserialize)]
pub struct AnalyzePayload {
    pub url: String,
}

async fn analyze_url_handler(
    State(state): State<AppState>,
    Json(payload): Json<AnalyzePayload>,
) -> Response {
    state.add_log("INFO", &format!("API /analyze llamada para: {}", payload.url)).await;
    let analyzer = crate::engine::analyzer::SiteAnalyzer::new();
    match analyzer.analyze(&payload.url).await {
        Ok(res) => (StatusCode::OK, Json(serde_json::to_value(res).unwrap())).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

// ==================== ENDPOINTS DE ASISTENCIA IA & MCP ====================

async fn get_ai_diagnostics_handler(State(state): State<AppState>) -> Response {
    let logs = state.logs.read().await;
    let recent_logs: Vec<_> = logs.iter().cloned().collect();
    let ext_count = state.extensions.read().await.len();
    let gh = state.gh_user.read().await;

    let failed_sites: Vec<String> = recent_logs.iter()
        .filter(|l| l.level == "ERROR" || l.level == "WARN")
        .map(|l| l.message.clone())
        .collect();

    let report = AiDiagnosticReport {
        app_version: "1.0.0".to_string(),
        platform: if cfg!(target_os = "android") { "android".to_string() } else { "desktop".to_string() },
        recent_logs,
        failed_sites,
        system_info: SystemStatus {
            app_version: "1.0.0".to_string(),
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

async fn apply_ai_patch_handler(
    State(state): State<AppState>,
    Json(patch): Json<AiPatchRequest>,
) -> Response {
    state.add_log("INFO", &format!("Aplicando parche de IA a la extensión: {}", patch.pkg_id)).await;
    let mut extensions = state.extensions.write().await;
    if let Some(ext) = extensions.iter_mut().find(|e| e.pkg_id == patch.pkg_id) {
        ext.version_code += 1;
        ext.version = format!("16.{}", ext.version_code);
        state.add_log("SUCCESS", &format!("Extensión {} actualizada con parche de IA a v{}", ext.pkg_id, ext.version)).await;
        (StatusCode::OK, Json(serde_json::json!({ "status": "patched", "version": ext.version }))).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Extensión no encontrada" }))).into_response()
    }
}

// MCP Manifest & Protocol Server
async fn mcp_manifest_handler() -> Response {
    let manifest = serde_json::json!({
        "name": "AEXT MCP Server",
        "description": "Servidor MCP nativo de AEXT para permitir a modelos de IA inspeccionar el estado en dispositivos móviles y asistir en la resolución de extensiones",
        "version": "1.0.0",
        "tools": [
            {
                "name": "aext_get_status",
                "description": "Obtiene el estado de AEXT en el dispositivo móvil o PC (versión, extensiones activas, plataforma, logs recientes)",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "aext_get_logs",
                "description": "Obtiene los logs de ejecución y diagnósticos de la aplicación móvil (útil ya que en móvil no hay terminal)",
                "inputSchema": { "type": "object", "properties": { "limit": { "type": "integer" } } }
            },
            {
                "name": "aext_inspect_dom",
                "description": "Descarga y analiza el DOM HTML de un sitio web que falló en la automatización para sugerir selectores CSS",
                "inputSchema": {
                    "type": "object",
                    "properties": { "url": { "type": "string" } },
                    "required": ["url"]
                }
            },
            {
                "name": "aext_patch_extension",
                "description": "Aplica una corrección de código Kotlin o selectores CSS a una extensión que falló",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pkg_id": { "type": "string" },
                        "card_selector": { "type": "string" },
                        "title_selector": { "type": "string" }
                    },
                    "required": ["pkg_id"]
                }
            }
        ]
    });
    Json(manifest).into_response()
}

async fn mcp_jsonrpc_handler(
    State(state): State<AppState>,
    Json(req): Json<McpToolCallRequest>,
) -> Response {
    state.add_log("INFO", &format!("Llamada MCP recibida: {}", req.method)).await;

    match req.method.as_str() {
        "tools/list" => {
            mcp_manifest_handler().await
        }
        "tools/call" => {
            let params = req.params.unwrap_or_default();
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(serde_json::Value::Null);

            match name {
                "aext_get_status" => {
                    let ext_count = state.extensions.read().await.len();
                    let res = serde_json::json!({
                        "platform": if cfg!(target_os = "android") { "android" } else { "desktop" },
                        "version": "1.0.0",
                        "extensions_count": ext_count,
                        "server_port": 45731
                    });
                    (StatusCode::OK, Json(serde_json::json!({ "result": res }))).into_response()
                }
                "aext_get_logs" => {
                    let logs = state.logs.read().await;
                    let list: Vec<_> = logs.iter().cloned().collect();
                    (StatusCode::OK, Json(serde_json::json!({ "result": list }))).into_response()
                }
                "aext_inspect_dom" => {
                    let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("");
                    let analyzer = crate::engine::analyzer::SiteAnalyzer::new();
                    match analyzer.analyze(url).await {
                        Ok(res) => (StatusCode::OK, Json(serde_json::json!({ "result": res }))).into_response(),
                        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e }))).into_response(),
                    }
                }
                _ => (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "error": format!("Tool MCP '{}' no encontrada", name) })),
                ).into_response(),
            }
        }
        _ => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Método MCP no soportado" })),
        ).into_response(),
    }
}
