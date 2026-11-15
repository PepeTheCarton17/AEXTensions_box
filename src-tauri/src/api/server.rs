use crate::models::{AiDiagnosticReport, AiPatchRequest, McpToolCallRequest, SystemStatus};
use crate::state::AppState;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use std::fs;
use std::path::{Path as FsPath, PathBuf};
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

fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
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

/// Entrega un ZIP real con el código fuente de la extensión (y su
/// `extension.json`). En modo local no existe un APK compilado en el
/// dispositivo, así que este es el artefacto honesto que podemos ofrecer.
async fn download_extension_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let items = state.extensions.read().await;
    let item = items
        .iter()
        .find(|e| e.id == id || e.pkg_id == id)
        .cloned();
    drop(items);

    let Some(ext) = item else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let dir = crate::commands::local_extension_dir(&state, &ext.pkg_id);
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    if dir.is_dir() {
        collect_dir_entries(&dir, &dir, &mut entries);
    }

    // Metadatos legibles por humanos / por Nekoyomi.
    match serde_json::to_vec_pretty(&ext) {
        Ok(bytes) => entries.push(("extension.json".to_string(), bytes)),
        Err(e) => {
            state
                .add_log("WARN", &format!("No se pudo serializar extension.json: {}", e))
                .await;
        }
    }

    if entries.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "No hay archivos locales para esta extensión",
                "pkg_id": ext.pkg_id,
            })),
        )
            .into_response();
    }

    let zip = crate::engine::create_zip(&entries);
    let filename = format!("{}.zip", ext.pkg_id);
    let disposition = format!("attachment; filename=\"{}\"", filename);

    let mut response = Response::new(Body::from(zip));
    *response.status_mut() = StatusCode::OK;
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("application/zip"));
    if let Ok(value) = HeaderValue::from_str(&disposition) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    response
}

fn collect_dir_entries(root: &FsPath, current: &FsPath, out: &mut Vec<(String, Vec<u8>)>) {
    let Ok(read_dir) = fs::read_dir(current) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_dir_entries(root, &path, out);
        } else if path.is_file() {
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            let name = relative.to_string_lossy().replace('\\', "/");
            if let Ok(bytes) = fs::read(&path) {
                out.push((name, bytes));
            }
        }
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

// ==================== ENDPOINTS DE ASISTENCIA IA & MCP ====================

async fn get_ai_diagnostics_handler(State(state): State<AppState>) -> Response {
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

async fn apply_ai_patch_handler(
    State(state): State<AppState>,
    Json(patch): Json<AiPatchRequest>,
) -> Response {
    state
        .add_log(
            "INFO",
            &format!("Aplicando parche de IA a la extensión: {}", patch.pkg_id),
        )
        .await;

    let mut extensions = state.extensions.write().await;
    let Some(ext) = extensions
        .iter_mut()
        .find(|e| e.pkg_id == patch.pkg_id || e.id == patch.pkg_id)
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Extensión no encontrada" })),
        )
            .into_response();
    };

    let next_code = ext.version_code + 1;
    let next_version = format!("16.{}", next_code);
    ext.version_code = next_code;
    ext.version = next_version.clone();

    let dir = crate::commands::local_extension_dir(&state, &ext.pkg_id);
    let persisted = apply_patch_to_files(&dir, &patch, next_code, &next_version);

    drop(extensions);
    state.persist_extensions().await;

    match &persisted {
        Ok(true) => state
            .add_log(
                "SUCCESS",
                &format!(
                    "Extensión {} actualizada con parche de IA a v{} (archivos locales reescritos)",
                    patch.pkg_id, next_version
                ),
            )
            .await,
        Ok(false) => state
            .add_log(
                "WARN",
                &format!(
                    "Extensión {} actualizada a v{} solo en el catálogo: no hay copia local de los archivos para parchear",
                    patch.pkg_id, next_version
                ),
            )
            .await,
        Err(e) => state
            .add_log(
                "WARN",
                &format!("Parche aplicado en catálogo, pero falló la escritura de archivos: {}", e),
            )
            .await,
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "patched",
            "version": next_version,
            "files_patched": persisted.unwrap_or(false)
        })),
    )
        .into_response()
}

/// Reescribe `build.gradle.kts` (versión) y los selectores del Kotlin si la
/// extensión tiene copia local. Devuelve `false` si no existe esa copia.
fn apply_patch_to_files(
    dir: &FsPath,
    patch: &AiPatchRequest,
    version_code: i32,
    version_name: &str,
) -> Result<bool, String> {
    if !dir.is_dir() {
        return Ok(false);
    }

    let gradle_path = find_module_gradle(dir, &patch.pkg_id);
    if let Some(gradle_path) = gradle_path {
        let content = fs::read_to_string(&gradle_path)
            .map_err(|e| format!("No se pudo leer {}: {}", gradle_path.display(), e))?;
        let re_code = regex::Regex::new(r"versionCode\s*=\s*\d+")
            .map_err(|e| e.to_string())?;
        let re_name = regex::Regex::new(r#"versionName\s*=\s*"[^"]*""#)
            .map_err(|e| e.to_string())?;
        let content = re_code
            .replace(&content, format!("versionCode = {}", version_code).as_str())
            .to_string();
        let content = re_name
            .replace(&content, format!("versionName = \"{}\"", version_name).as_str())
            .to_string();
        fs::write(&gradle_path, content)
            .map_err(|e| format!("No se pudo escribir {}: {}", gradle_path.display(), e))?;
    }

    let mut kotlin_files = Vec::new();
    collect_files_with_extension(dir, "kt", &mut kotlin_files);
    for kt in kotlin_files {
        let Ok(content) = fs::read_to_string(&kt) else {
            continue;
        };
        let original = content.clone();
        let mut patched = content;

        if let Some(selector) = patch.custom_card_selector.as_deref() {
            if !selector.trim().is_empty() {
                let escaped = escape_kotlin_string(selector);
                for func in [
                    "popularAnimeSelector",
                    "latestUpdatesSelector",
                    "searchAnimeSelector",
                ] {
                    patched = replace_selector_function(&patched, func, &escaped);
                }
            }
        }

        if let Some(selector) = patch.custom_next_selector.as_deref() {
            if !selector.trim().is_empty() {
                let escaped = escape_kotlin_string(selector);
                for func in [
                    "popularAnimeNextPageSelector",
                    "latestUpdatesNextPageSelector",
                    "searchAnimeNextPageSelector",
                ] {
                    patched = replace_selector_function(&patched, func, &escaped);
                }
            }
        }

        if let Some(selector) = patch.custom_title_selector.as_deref() {
            if !selector.trim().is_empty() {
                let escaped = escape_kotlin_string(selector);
                if let Ok(re) = regex::Regex::new(r#"title\s*=\s*element\.selectFirst\("[^"]*"\)"#) {
                    patched = re
                        .replace(&patched, |_: &regex::Captures| {
                            format!("title = element.selectFirst(\"{}\")", escaped)
                        })
                        .to_string();
                }
            }
        }

        if let Some(code) = patch.custom_video_code.as_deref() {
            if !code.trim().is_empty() {
                let marker = "\n// ============================== AEXT IA PATCH ==============================\n";
                if !patched.contains("AEXT IA PATCH") {
                    patched.push_str(marker);
                    patched.push_str(code);
                    patched.push('\n');
                }
            }
        }

        if patched != original {
            fs::write(&kt, patched)
                .map_err(|e| format!("No se pudo escribir {}: {}", kt.display(), e))?;
        }
    }

    Ok(true)
}

fn replace_selector_function(content: &str, func: &str, escaped_selector: &str) -> String {
    let pattern = format!(r#"({}\(\)\s*:\s*String\s*=\s*")[^"]*(")"#, regex::escape(func));
    match regex::Regex::new(&pattern) {
        Ok(re) => re
            .replace(content, |caps: &regex::Captures| {
                format!(
                    "{}{}{}",
                    caps.get(1).map(|m| m.as_str()).unwrap_or(""),
                    escaped_selector,
                    caps.get(2).map(|m| m.as_str()).unwrap_or("")
                )
            })
            .to_string(),
        Err(_) => content.to_string(),
    }
}

fn escape_kotlin_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Localiza el `build.gradle.kts` del módulo de la extensión. La ruta se
/// deriva del `pkg_id` (`src/<lang>/<slug>`); si no existe, se busca.
fn find_module_gradle(dir: &FsPath, pkg_id: &str) -> Option<PathBuf> {
    let parts: Vec<&str> = pkg_id.split('.').filter(|p| !p.is_empty()).collect();
    if parts.len() >= 2 {
        let lang = parts[parts.len() - 2];
        let slug = parts[parts.len() - 1];
        let direct = dir
            .join("src")
            .join(lang)
            .join(slug)
            .join("build.gradle.kts");
        if direct.exists() {
            return Some(direct);
        }
    }

    let mut candidates = Vec::new();
    collect_gradle_files(dir, &mut candidates);
    candidates.sort_by_key(|path| path.components().count());
    candidates.into_iter().find(|path| {
        fs::read_to_string(path)
            .map(|content| content.contains("applicationId"))
            .unwrap_or(false)
    })
}

fn collect_gradle_files(dir: &FsPath, out: &mut Vec<PathBuf>) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_gradle_files(&path, out);
        } else if path.file_name().and_then(|name| name.to_str()) == Some("build.gradle.kts") {
            out.push(path);
        }
    }
}

fn collect_files_with_extension(dir: &FsPath, extension: &str, out: &mut Vec<PathBuf>) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files_with_extension(&path, extension, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case(extension))
            .unwrap_or(false)
        {
            out.push(path);
        }
    }
}

// MCP Manifest & Protocol Server
async fn mcp_manifest_handler() -> Response {
    let manifest = serde_json::json!({
        "name": "AEXT MCP Server",
        "description": "Servidor MCP nativo de AEXT para permitir a modelos de IA inspeccionar el estado en dispositivos móviles y asistir en la resolución de extensiones",
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

async fn mcp_jsonrpc_handler(
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
