use crate::models::{ExtractorTestResult, LogEntry, SystemStatus};
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_system_logs(state: State<'_, AppState>) -> Result<Vec<LogEntry>, String> {
    let logs = state.logs.read().await;
    Ok(logs.iter().cloned().collect())
}

#[tauri::command]
pub async fn test_stream_extractor(
    url: String,
    state: State<'_, AppState>,
) -> Result<ExtractorTestResult, String> {
    state
        .add_log("INFO", &format!("Probando extractor de stream para: {}", url))
        .await;
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .build()
        .map_err(|e| e.to_string())?;

    let host = if url.contains("zilla-networks.com") {
        "Zilla HLS"
    } else if url.contains("voe.sx") {
        "Voe"
    } else if url.contains("mp4upload.com") {
        "MP4Upload"
    } else {
        "Servidor Desconocido"
    };

    let mut req = client.get(&url);
    if host == "Zilla HLS" {
        req = req
            .header("Origin", "https://player.zilla-networks.com")
            .header("Referer", "https://player.zilla-networks.com/")
            .header("Sec-Fetch-Mode", "cors")
            .header("Sec-Fetch-Site", "same-origin");
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status();
            let headers = resp
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();

            if status.is_success() {
                state
                    .add_log(
                        "SUCCESS",
                        &format!("Prueba de extractor {} exitosa (HTTP {})", host, status),
                    )
                    .await;
                Ok(ExtractorTestResult {
                    host: host.to_string(),
                    success: true,
                    stream_url: Some(url),
                    quality: Some("1080p / Directo".to_string()),
                    headers,
                    error: None,
                })
            } else {
                let err_msg = format!("Servidor retornó HTTP {}", status);
                state
                    .add_log("WARN", &format!("Prueba de extractor {}: {}", host, err_msg))
                    .await;
                Ok(ExtractorTestResult {
                    host: host.to_string(),
                    success: false,
                    stream_url: None,
                    quality: None,
                    headers,
                    error: Some(err_msg),
                })
            }
        }
        Err(e) => {
            state
                .add_log("ERROR", &format!("Error probando {}: {}", host, e))
                .await;
            Ok(ExtractorTestResult {
                host: host.to_string(),
                success: false,
                stream_url: None,
                quality: None,
                headers: Vec::new(),
                error: Some(e.to_string()),
            })
        }
    }
}

#[tauri::command]
pub async fn get_system_status(state: State<'_, AppState>) -> Result<SystemStatus, String> {
    let ext_count = state.extensions.read().await.len();
    let gh = state.gh_user.read().await;

    Ok(SystemStatus {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        api_port: 45731,
        mcp_endpoint: "http://localhost:45731/mcp".to_string(),
        is_mobile: cfg!(target_os = "android"),
        os: std::env::consts::OS.to_string(),
        active_extensions_count: ext_count,
        gh_logged_in: gh.is_some(),
        gh_user: gh.as_ref().map(|u| u.username.clone()),
    })
}

/// Abre un enlace externo en el navegador del sistema. En Tauri el webview no
/// debe navegar fuera de la app, así que todos los enlaces pasan por aquí.
#[tauri::command]
pub async fn open_url(url: String) -> Result<(), String> {
    let trimmed = url.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err("Solo se permiten enlaces http/https.".to_string());
    }
    tauri_plugin_opener::open_url(trimmed.to_string(), None::<&str>)
        .map_err(|e| format!("No se pudo abrir el enlace: {}", e))
}
