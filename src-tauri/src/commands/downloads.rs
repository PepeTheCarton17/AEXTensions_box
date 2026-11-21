use serde_json::json;

/// Descarga un ZIP del servidor local y lo guarda en la carpeta de descargas.
///
/// El webview no siempre completa las descargas Blob (WebView2 puede ignorar
/// el evento y no avisar al usuario), así que en escritorio la escritura la
/// hace Rust y devuelve la ruta real del archivo.
#[tauri::command]
pub async fn save_extension_zip(
    app: tauri::AppHandle,
    url: String,
    filename: Option<String>,
) -> Result<serde_json::Value, String> {
    let trimmed = url.trim().to_string();
    let lower = trimmed.to_ascii_lowercase();
    // Solo el propio servidor embebido: evita que la app actúe como proxy.
    if !(lower.starts_with("http://127.0.0.1:") || lower.starts_with("http://localhost:")) {
        return Err("Solo se permiten descargas del servidor local de AEXT.".to_string());
    }

    let client = reqwest::Client::builder()
        .user_agent("AEXT-Downloader/1.0")
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(&trimmed)
        .send()
        .await
        .map_err(|e| format!("No se pudo descargar el ZIP: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("El servidor respondió HTTP {}", response.status()));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("No se pudo leer el ZIP: {}", e))?;

    use tauri::Manager;
    let dir = app
        .path()
        .download_dir()
        .or_else(|_| app.path().app_data_dir())
        .unwrap_or_else(|_| std::env::temp_dir());

    let raw = filename.unwrap_or_default();
    let base = raw
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    let mut name: String = base
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect();
    if name.is_empty() {
        name = "extension.zip".to_string();
    }
    if !name.to_ascii_lowercase().ends_with(".zip") {
        name.push_str(".zip");
    }

    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("No se pudo crear {}: {}", dir.display(), e))?;
    let path = dir.join(&name);
    std::fs::write(&path, &bytes)
        .map_err(|e| format!("No se pudo escribir {}: {}", path.display(), e))?;

    Ok(json!({
        "path": path.display().to_string(),
        "bytes": bytes.len(),
        "filename": name,
    }))
}
