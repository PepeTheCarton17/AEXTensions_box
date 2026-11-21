use crate::models::McpToolCallRequest;
use crate::state::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};

use super::app_version;

// MCP Manifest & Protocol Server
pub(super) async fn mcp_manifest_handler() -> Response {
    let manifest = serde_json::json!({
        "name": "AEXT MCP Server",
        "description": "Servidor MCP nativo de AEXT (solo loopback) para que clientes MCP externos inspeccionen el estado y apliquen parches manuales a una extensión",
        "version": app_version(),
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

pub(super) async fn mcp_jsonrpc_handler(
    State(state): State<AppState>,
    Json(req): Json<McpToolCallRequest>,
) -> Response {
    state
        .add_log("INFO", &format!("Llamada MCP recibida: {}", req.method))
        .await;

    match req.method.as_str() {
        "tools/list" => mcp_manifest_handler().await,
        "tools/call" => {
            let params = req.params.unwrap_or_default();
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or(serde_json::Value::Null);

            match name {
                "aext_get_status" => {
                    let ext_count = state.extensions.read().await.len();
                    let res = serde_json::json!({
                        "platform": if cfg!(target_os = "android") { "android" } else { "desktop" },
                        "version": app_version(),
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
                        Ok(res) => {
                            (StatusCode::OK, Json(serde_json::json!({ "result": res }))).into_response()
                        }
                        Err(e) => (
                            StatusCode::BAD_REQUEST,
                            Json(serde_json::json!({ "error": e })),
                        )
                            .into_response(),
                    }
                }
                _ => (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "error": format!("Tool MCP '{}' no encontrada", name) })),
                )
                    .into_response(),
            }
        }
        _ => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Método MCP no soportado" })),
        )
            .into_response(),
    }
}
