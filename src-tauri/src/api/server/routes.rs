use crate::state::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};

use super::app_version;
use super::diagnostics::get_ai_diagnostics_handler;
use super::extensions::download_extension_handler;
use super::mcp::{mcp_jsonrpc_handler, mcp_manifest_handler};
use super::models::AnalyzePayload;
use super::patch::apply_ai_patch_handler;

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

    // Solo loopback: la app es local y así no se abre el puerto a la red
    // (ni se dispara el aviso del firewall de Windows).
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    app_state
        .add_log(
            "INFO",
            &format!(
                "Servidor API & MCP embebido escuchando en http://127.0.0.1:{}",
                port
            ),
        )
        .await;

    tokio::spawn(async move {
        if let Ok(listener) = tokio::net::TcpListener::bind(&addr).await {
            let _ = axum::serve(listener, app).await;
        } else {
            app_state
                .add_log(
                    "ERROR",
                    &format!("No se pudo enlazar el servidor API en el puerto {}", port),
                )
                .await;
        }
    });
}

async fn get_status_handler(State(state): State<AppState>) -> Response {
    let ext_count = state.extensions.read().await.len();
    let gh = state.gh_user.read().await;

    let status = serde_json::json!({
        "status": "online",
        "app_name": "AEXT",
        "version": app_version(),
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

async fn get_logs_handler(State(state): State<AppState>) -> Response {
    let logs = state.logs.read().await;
    let list: Vec<_> = logs.iter().cloned().collect();
    Json(list).into_response()
}

async fn analyze_url_handler(
    State(state): State<AppState>,
    Json(payload): Json<AnalyzePayload>,
) -> Response {
    state
        .add_log("INFO", &format!("API /analyze llamada para: {}", payload.url))
        .await;
    let analyzer = crate::engine::analyzer::SiteAnalyzer::new();
    match analyzer.analyze(&payload.url).await {
        Ok(res) => (StatusCode::OK, Json(serde_json::to_value(res).unwrap())).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}
