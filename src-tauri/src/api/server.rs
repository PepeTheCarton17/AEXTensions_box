use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
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
        .layer(cors)
        .with_state(app_state.clone());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    app_state.add_log("INFO", &format!("Servidor API embebido escuchando en http://0.0.0.0:{}", port)).await;

    tokio::spawn(async move {
        if let Ok(listener) = tokio::net::TcpListener::bind(&addr).await {
            let _ = axum::serve(listener, app).await;
        } else {
            app_state.add_log("ERROR", &format!("No se pudo enlazar el servidor API en el puerto {}", port)).await;
        }
    });
}

async fn get_status_handler(State(state): State<AppState>) -> impl IntoResponse {
    let ext_count = state.extensions.read().await.len();
    let gh = state.gh_user.read().await;

    let status = serde_json::json!({
        "status": "online",
        "app_name": "AEXT",
        "version": "1.0.0",
        "extensions_count": ext_count,
        "github_logged_in": gh.is_some(),
        "github_user": gh.as_ref().map(|u| &u.username),
        "platform": if cfg!(target_os = "android") { "android" } else { "desktop" }
    });

    Json(status)
}

async fn get_extensions_handler(State(state): State<AppState>) -> impl IntoResponse {
    let items = state.extensions.read().await.clone();
    Json(items)
}

async fn download_extension_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
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
            Ok(Json(info))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn get_logs_handler(State(state): State<AppState>) -> impl IntoResponse {
    let logs = state.logs.read().await;
    let list: Vec<_> = logs.iter().cloned().collect();
    Json(list)
}

#[derive(serde::Deserialize)]
struct AnalyzePayload {
    url: String,
}

async fn analyze_url_handler(
    State(state): State<AppState>,
    Json(payload): Json<AnalyzePayload>,
) -> impl IntoResponse {
    state.add_log("INFO", &format!("API /analyze llamada para: {}", payload.url)).await;
    let analyzer = crate::engine::analyzer::SiteAnalyzer::new();
    match analyzer.analyze(&payload.url).await {
        Ok(res) => (StatusCode::OK, Json(serde_json::to_value(res).unwrap())),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e }))),
    }
}
