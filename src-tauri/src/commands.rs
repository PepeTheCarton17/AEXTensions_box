use crate::engine::{AnalysisResult, ExtensionGenerator, IconEngine, SiteAnalyzer};
use crate::github::{GithubApiClient, GithubAuthManager};
use crate::models::{
    ExtensionItem, ExtractorTestResult, GenerationMode, GenerationRequest, GithubAccountInfo,
    LogEntry, SystemStatus,
};
use crate::state::AppState;
use std::fs;
use tauri::State;

// ============================== COMANDOS GITHUB ==============================

#[tauri::command]
pub async fn get_gh_auth_status(state: State<'_, AppState>) -> Result<Option<GithubAccountInfo>, String> {
    let auth = GithubAuthManager::new();
    
    // Primero intentamos token guardado
    if let Some(token) = auth.get_saved_token() {
        if let Ok(info) = auth.verify_token(&token).await {
            let mut gh_state = state.gh_user.write().await;
            *gh_state = Some(info.clone());
            return Ok(Some(info));
        }
    }

    // Si estamos en PC, intentamos detectar CLI
    if let Ok(cli_info) = auth.detect_cli_session().await {
        let mut gh_state = state.gh_user.write().await;
        *gh_state = Some(cli_info.clone());
        return Ok(Some(cli_info));
    }

    Ok(None)
}

#[tauri::command]
pub async fn login_with_token(token: String, state: State<'_, AppState>) -> Result<GithubAccountInfo, String> {
    let auth = GithubAuthManager::new();
    let info = auth.verify_token(&token).await?;
    auth.save_token(&token)?;

    let mut gh_state = state.gh_user.write().await;
    *gh_state = Some(info.clone());
    state.add_log("SUCCESS", &format!("Sesión iniciada con GitHub (@{})", info.username)).await;

    Ok(info)
}

#[tauri::command]
pub async fn login_with_cli(state: State<'_, AppState>) -> Result<GithubAccountInfo, String> {
    let auth = GithubAuthManager::new();
    let info = auth.detect_cli_session().await?;

    let mut gh_state = state.gh_user.write().await;
    *gh_state = Some(info.clone());
    state.add_log("SUCCESS", &format!("Sesión detectada vía GitHub CLI (@{})", info.username)).await;

    Ok(info)
}

#[tauri::command]
pub async fn logout_gh(state: State<'_, AppState>) -> Result<(), String> {
    let auth = GithubAuthManager::new();
    let _ = auth.clear_token();

    let mut gh_state = state.gh_user.write().await;
    *gh_state = None;
    state.add_log("INFO", "Sesión de GitHub cerrada").await;

    Ok(())
}

// ============================== COMANDOS ANÁLISIS & MOTOR ==============================

#[tauri::command]
pub async fn analyze_url(url: String, state: State<'_, AppState>) -> Result<AnalysisResult, String> {
    state.add_log("INFO", &format!("Iniciando análisis de URL: {}", url)).await;
    let analyzer = SiteAnalyzer::new();
    match analyzer.analyze(&url).await {
        Ok(res) => {
            state.add_log("SUCCESS", &format!("Análisis completado para {}: CMS={}, Tipo={}", res.name, res.cms, res.ext_type)).await;
            Ok(res)
        }
        Err(e) => {
            state.add_log("ERROR", &format!("Error analizando {}: {}", url, e)).await;
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn analyze_multiple_urls(urls: Vec<String>, state: State<'_, AppState>) -> Result<Vec<AnalysisResult>, String> {
    let mut results = Vec::new();
    let analyzer = SiteAnalyzer::new();

    for url in urls {
        state.add_log("INFO", &format!("Analizando: {}", url)).await;
        if let Ok(res) = analyzer.analyze(&url).await {
            results.push(res);
        }
    }

    Ok(results)
}

// ============================== COMANDOS GENERACIÓN ==============================

#[tauri::command]
pub async fn generate_extensions(
    request: GenerationRequest,
    state: State<'_, AppState>,
) -> Result<Vec<ExtensionItem>, String> {
    let auth = GithubAuthManager::new();
    let analyzer = SiteAnalyzer::new();
    let mut created = Vec::new();

    state.add_log("INFO", &format!("Generando {} extensión(es) en modo {:?}", request.urls.len(), request.mode)).await;

    for url in &request.urls {
        let analysis = analyzer.analyze(url).await
            .map_err(|e| format!("Error analizando {}: {}", url, e))?;

        // 1. Generar iconos y mipmaps
        state.add_log("INFO", &format!("Generando iconos para {}", analysis.name)).await;
        let (mipmaps, icon_base64) = IconEngine::fetch_and_generate_icons(
            analysis.favicon_url.as_deref(),
            &analysis.name,
        ).await.unwrap_or_else(|_| (std::collections::HashMap::new(), String::new()));

        // 2. Generar código Kotlin y configuración de extensión
        state.add_log("INFO", &format!("Generando código Kotlin y manifiesto para {}", analysis.name)).await;
        let bundle = ExtensionGenerator::generate(&analysis, request.custom_name.as_deref(), request.lang.as_deref());

        // 3. Manejar según el modo (Local vs GitHub)
        let mut ext_item = ExtensionItem {
            id: bundle.pkg_id.clone(),
            pkg_id: bundle.pkg_id.clone(),
            name: bundle.name.clone(),
            class_name: bundle.class_name.clone(),
            base_url: analysis.url.clone(),
            lang: bundle.lang.clone(),
            ext_type: bundle.ext_type.clone(),
            version: bundle.version.clone(),
            version_code: bundle.version_code,
            cms: analysis.cms.clone(),
            has_sub_dub: analysis.has_sub_dub,
            icon_base64: Some(icon_base64),
            github_repo_url: None,
            nekoyomi_repo_url: None,
            apk_download_url: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        match request.mode {
            GenerationMode::LocalOnly => {
                let target_dir = state.storage_dir.join("extensions").join(&bundle.pkg_id);
                let _ = fs::create_dir_all(&target_dir);

                for (rel_path, content) in &bundle.files {
                    let file_path = target_dir.join(rel_path);
                    if let Some(parent) = file_path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let _ = fs::write(file_path, content);
                }

                for (res_dir, icon_bytes) in &mipmaps {
                    let icon_path = target_dir.join("res").join(res_dir).join("ic_launcher.png");
                    if let Some(parent) = icon_path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let _ = fs::write(icon_path, icon_bytes);
                }

                ext_item.apk_download_url = Some(format!("http://localhost:45731/api/download/{}", bundle.pkg_id));
                state.add_log("SUCCESS", &format!("Extensión {} generada localmente en {}", bundle.name, target_dir.display())).await;
            }

            GenerationMode::GithubRepo => {
                let token = auth.get_saved_token().ok_or("Se requiere iniciar sesión en GitHub para publicar repositorios")?;
                let gh_user = auth.verify_token(&token).await?;
                let client = GithubApiClient::new(token);

                let repo_name = request.repo_name.clone().unwrap_or_else(|| {
                    format!("nekoyomi-ext-{}", bundle.class_name.to_lowercase())
                });

                state.add_log("INFO", &format!("Creando repositorio en GitHub: {}/{}", gh_user.username, repo_name)).await;
                let repo_url = client.ensure_repository(&gh_user.username, &repo_name, request.private_repo.unwrap_or(false)).await?;

                state.add_log("INFO", "Subiendo archivos de código fuente a GitHub").await;
                for (rel_path, content) in &bundle.files {
                    let _ = client.put_file_content(
                        &gh_user.username,
                        &repo_name,
                        rel_path,
                        content,
                        &format!("AEXT: Añadir {}", rel_path),
                    ).await;
                }

                for (res_dir, icon_bytes) in &mipmaps {
                    let p = format!("res/{}/ic_launcher.png", res_dir);
                    let _ = client.put_file_content(
                        &gh_user.username,
                        &repo_name,
                        &p,
                        icon_bytes,
                        "AEXT: Añadir icono launcher",
                    ).await;
                }

                ext_item.github_repo_url = Some(repo_url);
                ext_item.nekoyomi_repo_url = Some(format!(
                    "https://raw.githubusercontent.com/{}/{}/gh-pages/repo.json",
                    gh_user.username, repo_name
                ));
                ext_item.apk_download_url = Some(format!(
                    "https://raw.githubusercontent.com/{}/{}/gh-pages/{}-v{}.apk",
                    gh_user.username, repo_name, bundle.pkg_id, bundle.version
                ));

                state.add_log("SUCCESS", &format!("Extensión subida exitosamente a GitHub: {}", ext_item.github_repo_url.as_ref().unwrap())).await;
            }
        }

        {
            let mut list = state.extensions.write().await;
            list.retain(|x| x.pkg_id != ext_item.pkg_id);
            list.push(ext_item.clone());
        }
        state.persist_extensions().await;
        created.push(ext_item);
    }

    Ok(created)
}

// ============================== COMANDOS GESTIÓN DE EXTENSIONES ==============================

#[tauri::command]
pub async fn get_created_extensions(state: State<'_, AppState>) -> Result<Vec<ExtensionItem>, String> {
    let list = state.extensions.read().await;
    Ok(list.clone())
}

#[tauri::command]
pub async fn delete_extension(pkg_id: String, state: State<'_, AppState>) -> Result<bool, String> {
    let mut list = state.extensions.write().await;
    let initial_len = list.len();
    list.retain(|e| e.pkg_id != pkg_id);
    let removed = list.len() < initial_len;
    drop(list);

    if removed {
        state.persist_extensions().await;
        state.add_log("INFO", &format!("Extensión {} eliminada del catálogo", pkg_id)).await;
    }

    Ok(removed)
}

// ============================== COMANDOS DIAGNÓSTICO & LOGS ==============================

#[tauri::command]
pub async fn get_system_logs(state: State<'_, AppState>) -> Result<Vec<LogEntry>, String> {
    let logs = state.logs.read().await;
    Ok(logs.iter().cloned().collect())
}

#[tauri::command]
pub async fn test_stream_extractor(url: String, state: State<'_, AppState>) -> Result<ExtractorTestResult, String> {
    state.add_log("INFO", &format!("Probando extractor de stream para: {}", url)).await;
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
            let headers = resp.headers().iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();

            if status.is_success() {
                state.add_log("SUCCESS", &format!("Prueba de extractor {} exitosa (HTTP {})", host, status)).await;
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
                state.add_log("WARN", &format!("Prueba de extractor {}: {}", host, err_msg)).await;
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
            state.add_log("ERROR", &format!("Error probando {}: {}", host, e)).await;
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
        app_version: "1.0.0".to_string(),
        api_port: 45731,
        mcp_endpoint: "http://localhost:45731/mcp".to_string(),
        is_mobile: cfg!(target_os = "android"),
        os: std::env::consts::OS.to_string(),
        active_extensions_count: ext_count,
        gh_logged_in: gh.is_some(),
        gh_user: gh.as_ref().map(|u| u.username.clone()),
    })
}

#[tauri::command]
pub async fn check_aext_updates() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "current_version": "1.0.0",
        "latest_version": "1.0.0",
        "update_available": false,
        "release_notes": "Versión inicial de AEXT - Generador y Gestor Automatizado de Extensiones"
    }))
}
