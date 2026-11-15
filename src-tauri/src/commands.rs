use crate::engine::{AnalysisResult, ExtensionGenerator, IconEngine, SiteAnalyzer};
use crate::github::{GithubApiClient, GithubAuthManager, RepoInfo, RepoSummary};
use crate::models::{
    ExtensionItem, ExtractorTestResult, GenerationMode, GenerationRequest, GithubAccountInfo,
    LogEntry, SystemStatus,
};
use crate::state::AppState;
use std::fs;
use std::path::Path;
use tauri::State;

const LOCAL_API_BASE: &str = "http://localhost:45731";

// ============================== COMANDOS GITHUB ==============================

#[tauri::command]
pub async fn get_gh_auth_status(
    state: State<'_, AppState>,
) -> Result<Option<GithubAccountInfo>, String> {
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
pub async fn login_with_token(
    token: String,
    state: State<'_, AppState>,
) -> Result<GithubAccountInfo, String> {
    let auth = GithubAuthManager::new();
    let info = auth.verify_token(&token).await?;
    auth.save_token(&token)?;

    let mut gh_state = state.gh_user.write().await;
    *gh_state = Some(info.clone());
    state
        .add_log("SUCCESS", &format!("Sesión iniciada con GitHub (@{})", info.username))
        .await;

    Ok(info)
}

#[tauri::command]
pub async fn login_with_cli(state: State<'_, AppState>) -> Result<GithubAccountInfo, String> {
    let auth = GithubAuthManager::new();
    let info = auth.detect_cli_session().await?;

    let mut gh_state = state.gh_user.write().await;
    *gh_state = Some(info.clone());
    state
        .add_log("SUCCESS", &format!("Sesión detectada vía GitHub CLI (@{})", info.username))
        .await;

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

/// Devuelve los repositorios del usuario autenticado para poblar el selector
/// del modal de generación (así no se crea un repo nuevo sin querer).
#[tauri::command]
pub async fn list_github_repos(state: State<'_, AppState>) -> Result<Vec<RepoSummary>, String> {
    let auth = GithubAuthManager::new();
    let token = match auth.get_saved_token() {
        Some(t) => t,
        None => match auth.extract_and_save_cli_token().await {
            Ok((t, info)) => {
                let mut gh_state = state.gh_user.write().await;
                *gh_state = Some(info);
                t
            }
            Err(_) => return Ok(Vec::new()),
        },
    };

    let client = GithubApiClient::new(token);
    client.list_repositories().await
}

// ============================== COMANDOS ANÁLISIS & MOTOR ==============================

#[tauri::command]
pub async fn analyze_url(url: String, state: State<'_, AppState>) -> Result<AnalysisResult, String> {
    state
        .add_log("INFO", &format!("Iniciando análisis de URL: {}", url))
        .await;
    let analyzer = SiteAnalyzer::new();
    match analyzer.analyze(&url).await {
        Ok(res) => {
            state
                .add_log(
                    "SUCCESS",
                    &format!(
                        "Análisis completado para {}: CMS={}, Tipo={}",
                        res.name, res.cms, res.ext_type
                    ),
                )
                .await;
            Ok(res)
        }
        Err(e) => {
            state
                .add_log("ERROR", &format!("Error analizando {}: {}", url, e))
                .await;
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn analyze_multiple_urls(
    urls: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<AnalysisResult>, String> {
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

    state
        .add_log(
            "INFO",
            &format!(
                "Generando {} extensión(es) en modo {:?}",
                request.urls.len(),
                request.mode
            ),
        )
        .await;

    for url in &request.urls {
        let analysis = analyzer
            .analyze(url)
            .await
            .map_err(|e| format!("Error analizando {}: {}", url, e))?;

        // 1. Generar iconos y mipmaps
        state
            .add_log("INFO", &format!("Generando iconos para {}", analysis.name))
            .await;
        let (mipmaps, icon_base64) = IconEngine::fetch_and_generate_icons(
            analysis.favicon_url.as_deref(),
            &analysis.name,
        )
        .await
        .unwrap_or_else(|_| (std::collections::HashMap::new(), String::new()));

        // 2. Generar código Kotlin y configuración de extensión
        state
            .add_log(
                "INFO",
                &format!("Generando código Kotlin y manifiesto para {}", analysis.name),
            )
            .await;
        let mut bundle = ExtensionGenerator::generate(
            &analysis,
            request.custom_name.as_deref(),
            request.lang.as_deref(),
            request.ext_type.as_deref(),
        );

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
            icon_base64: if icon_base64.trim().is_empty() {
                None
            } else {
                Some(icon_base64.clone())
            },
            github_repo_url: None,
            nekoyomi_repo_url: None,
            apk_download_url: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        match request.mode {
            GenerationMode::LocalOnly => {
                let target_dir = local_extension_dir(&state, &bundle.pkg_id);
                // Limpiar restos de una generación anterior (estructura antigua).
                let _ = fs::remove_dir_all(&target_dir);
                write_bundle_to_dir(&target_dir, &bundle.files)?;
                write_icons_to_dir(&target_dir, &bundle.module_path, &mipmaps)?;

                ext_item.apk_download_url = Some(format!(
                    "{}/api/download/{}",
                    LOCAL_API_BASE, bundle.pkg_id
                ));
                state
                    .add_log(
                        "SUCCESS",
                        &format!(
                            "Extensión {} generada localmente en {}",
                            bundle.name,
                            target_dir.display()
                        ),
                    )
                    .await;
            }

            GenerationMode::GithubRepo => {
                let token = match auth.get_saved_token() {
                    Some(t) => t,
                    None => match auth.extract_and_save_cli_token().await {
                        Ok((t, info)) => {
                            let mut gh_state = state.gh_user.write().await;
                            *gh_state = Some(info);
                            state
                                .add_log(
                                    "INFO",
                                    "Token de GitHub CLI detectado y guardado automáticamente",
                                )
                                .await;
                            t
                        }
                        Err(_) => {
                            return Err("Se requiere iniciar sesión en GitHub. Ve a la sección de autenticación e ingresa tu Personal Access Token (PAT).".to_string());
                        }
                    },
                };

                let gh_user = auth.verify_token(&token).await?;
                let client = GithubApiClient::new(token);

                let raw_repo = request.repo_name.as_deref().unwrap_or("").trim();
                let (owner_opt, parsed_repo) = parse_repo_target(raw_repo);
                let repo_name = if parsed_repo.trim().is_empty() {
                    format!("nekoyomi-ext-{}", bundle.class_name.to_lowercase())
                } else {
                    sanitize_repo_name(&parsed_repo)
                };
                let owner = owner_opt.unwrap_or_else(|| gh_user.username.clone());

                state
                    .add_log(
                        "INFO",
                        &format!(
                            "Buscando / creando repositorio en GitHub: {}/{}",
                            owner, repo_name
                        ),
                    )
                    .await;
                let repo = client
                    .ensure_repository(
                        &owner,
                        &repo_name,
                        request.private_repo.unwrap_or(false),
                        &gh_user.username,
                    )
                    .await?;

                // El README generado trae un placeholder de repo: lo sustituimos
                // por el repositorio real para que los deep links funcionen.
                rewrite_readme_for_repo(&mut bundle.files, &repo.full_name);

                // En un repo reutilizado, conservar los módulos ya incluidos
                // para no romper las demás extensiones del repositorio.
                merge_existing_settings(&client, &repo, &mut bundle).await;

                state
                    .add_log(
                        "INFO",
                        if repo.created {
                            "Repositorio creado. Subiendo archivos de código fuente..."
                        } else {
                            "Repositorio existente detectado. Actualizando archivos..."
                        },
                    )
                    .await;

                let mut branch = if repo.empty {
                    String::new()
                } else {
                    repo.default_branch.clone()
                };
                let total_files = bundle.files.len();
                let mut uploaded = 0usize;
                let mut last_error: Option<String> = None;

                for (rel_path, content) in &bundle.files {
                    match client
                        .put_file_content(
                            &repo.owner,
                            &repo.name,
                            rel_path,
                            content,
                            &format!("AEXT: Añadir {}", rel_path),
                            &branch,
                        )
                        .await
                    {
                        Ok(()) => {
                            uploaded += 1;
                            if branch.is_empty() {
                                branch = repo.default_branch.clone();
                            }
                        }
                        Err(e) => {
                            // Si el repo estaba vacío y GitHub rechazó el primer
                            // commit, reintentamos ya con la rama por defecto.
                            if branch.is_empty() {
                                branch = repo.default_branch.clone();
                                if client
                                    .put_file_content(
                                        &repo.owner,
                                        &repo.name,
                                        rel_path,
                                        content,
                                        &format!("AEXT: Añadir {}", rel_path),
                                        &branch,
                                    )
                                    .await
                                    .is_ok()
                                {
                                    uploaded += 1;
                                    continue;
                                }
                            }
                            last_error = Some(e);
                        }
                    }
                }

                for (res_dir, icon_bytes) in &mipmaps {
                    let p = format!(
                        "{}/src/main/res/{}/ic_launcher.png",
                        bundle.module_path, res_dir
                    );
                    let _ = client
                        .put_file_content(
                            &repo.owner,
                            &repo.name,
                            &p,
                            icon_bytes,
                            "AEXT: Añadir icono launcher",
                            &branch,
                        )
                        .await;

                    if res_dir == "mipmap-xxxhdpi" || res_dir == "mipmap-xxhdpi" {
                        let _ = client
                            .put_file_content(
                                &repo.owner,
                                &repo.name,
                                &format!("icon/{}.png", bundle.pkg_id),
                                icon_bytes,
                                "AEXT: Añadir icono del paquete para Nekoyomi",
                                &branch,
                            )
                            .await;
                        let _ = client
                            .put_file_content(
                                &repo.owner,
                                &repo.name,
                                "icon/ic_launcher.png",
                                icon_bytes,
                                "AEXT: Añadir icono genérico para repositorio",
                                &branch,
                            )
                            .await;
                    }
                }

                if uploaded == 0 {
                    return Err(format!(
                        "No se pudo subir ningún archivo a {}/{}: {}",
                        repo.owner,
                        repo.name,
                        last_error.unwrap_or_else(|| "error desconocido de GitHub".to_string())
                    ));
                }
                if uploaded < total_files {
                    state
                        .add_log(
                            "WARN",
                            &format!(
                                "Se subieron {}/{} archivos; último error: {}",
                                uploaded,
                                total_files,
                                last_error.unwrap_or_default()
                            ),
                        )
                        .await;
                }

                ext_item.github_repo_url = Some(repo.html_url.clone());
                ext_item.nekoyomi_repo_url = Some(format!(
                    "https://raw.githubusercontent.com/{}/gh-pages/index.min.json",
                    repo.full_name
                ));
                // En modo GitHub no existe un APK todavía: la compilación la
                // hace GitHub Actions. No inventamos una URL de APK.
                ext_item.apk_download_url = None;

                state
                    .add_log(
                        "SUCCESS",
                        &format!(
                            "Extensión subida a GitHub: {}. La compilación del APK corre en Actions.",
                            repo.html_url
                        ),
                    )
                    .await;
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

/// Regenera una extensión existente (re-analiza su sitio, sube la versión y
/// reescribe los archivos locales y, si aplica, el repositorio de GitHub).
#[tauri::command]
pub async fn update_extension(
    pkg_id: String,
    state: State<'_, AppState>,
) -> Result<ExtensionItem, String> {
    let existing = {
        let list = state.extensions.read().await;
        list.iter()
            .find(|e| e.pkg_id == pkg_id || e.id == pkg_id)
            .cloned()
            .ok_or_else(|| format!("No se encontró la extensión {}", pkg_id))?
    };

    state
        .add_log(
            "INFO",
            &format!("Actualizando extensión {} desde {}", existing.name, existing.base_url),
        )
        .await;

    let analyzer = SiteAnalyzer::new();
    let analysis = analyzer
        .analyze(&existing.base_url)
        .await
        .map_err(|e| format!("No se pudo re-analizar {}: {}", existing.base_url, e))?;

    let next_code = existing.version_code + 1;
    let next_version = format!("16.{}", next_code);

    let (mipmaps, icon_base64) = IconEngine::fetch_and_generate_icons(
        analysis.favicon_url.as_deref(),
        &existing.name,
    )
    .await
    .unwrap_or_else(|_| (std::collections::HashMap::new(), String::new()));

    let mut bundle = ExtensionGenerator::generate_with_version(
        &analysis,
        Some(existing.name.as_str()),
        Some(existing.lang.as_str()),
        Some(existing.ext_type.as_str()),
        next_code,
        &next_version,
    );

    let mut updated = existing.clone();
    updated.version = bundle.version.clone();
    updated.version_code = bundle.version_code;
    updated.class_name = bundle.class_name.clone();
    updated.pkg_id = bundle.pkg_id.clone();
    updated.id = bundle.pkg_id.clone();
    updated.lang = bundle.lang.clone();
    updated.ext_type = bundle.ext_type.clone();
    updated.cms = analysis.cms.clone();
    updated.has_sub_dub = analysis.has_sub_dub;
    if !icon_base64.trim().is_empty() {
        updated.icon_base64 = Some(icon_base64.clone());
    }

    // 1. Reescribir siempre la copia local (sirve también como fuente del ZIP).
    let target_dir = local_extension_dir(&state, &bundle.pkg_id);
    let _ = fs::remove_dir_all(&target_dir);
    write_bundle_to_dir(&target_dir, &bundle.files)?;
    write_icons_to_dir(&target_dir, &bundle.module_path, &mipmaps)?;
    if existing.pkg_id != bundle.pkg_id {
        // El paquete cambió de id: borrar la carpeta local antigua.
        let _ = fs::remove_dir_all(local_extension_dir(&state, &existing.pkg_id));
    }
    if updated.github_repo_url.is_none() {
        updated.apk_download_url = Some(format!(
            "{}/api/download/{}",
            LOCAL_API_BASE, bundle.pkg_id
        ));
    }

    // 2. Si vive en GitHub, subir la nueva versión al repo existente.
    if let Some(repo_url) = existing.github_repo_url.clone() {
        let auth = GithubAuthManager::new();
        let token = match auth.get_saved_token() {
            Some(t) => t,
            None => match auth.extract_and_save_cli_token().await {
                Ok((t, info)) => {
                    let mut gh_state = state.gh_user.write().await;
                    *gh_state = Some(info);
                    t
                }
                Err(_) => {
                    return Err(format!(
                        "La extensión se actualizó localmente, pero para subirla a GitHub necesitas iniciar sesión. Repo: {}",
                        repo_url
                    ));
                }
            },
        };

        let gh_user = auth.verify_token(&token).await?;
        let client = GithubApiClient::new(token);

        let (owner, repo_name) = parse_github_url(&repo_url)
            .ok_or_else(|| format!("URL de repositorio no válida: {}", repo_url))?;
        let repo: RepoInfo = client
            .ensure_repository(&owner, &repo_name, false, &gh_user.username)
            .await?;

        rewrite_readme_for_repo(&mut bundle.files, &repo.full_name);
        // Igual que al generar: no perder los include() de un repo compartido.
        merge_existing_settings(&client, &repo, &mut bundle).await;

        let mut branch = if repo.empty {
            String::new()
        } else {
            repo.default_branch.clone()
        };
        let mut uploaded = 0usize;
        let mut last_error: Option<String> = None;

        for (rel_path, content) in &bundle.files {
            match client
                .put_file_content(
                    &repo.owner,
                    &repo.name,
                    rel_path,
                    content,
                    &format!("AEXT: Actualizar {} a v{}", rel_path, bundle.version),
                    &branch,
                )
                .await
            {
                Ok(()) => {
                    uploaded += 1;
                    if branch.is_empty() {
                        branch = repo.default_branch.clone();
                    }
                }
                Err(e) => last_error = Some(e),
            }
        }

        for (res_dir, icon_bytes) in &mipmaps {
            let _ = client
                .put_file_content(
                    &repo.owner,
                    &repo.name,
                    &format!(
                        "{}/src/main/res/{}/ic_launcher.png",
                        bundle.module_path, res_dir
                    ),
                    icon_bytes,
                    "AEXT: Actualizar icono launcher",
                    &branch,
                )
                .await;
            if res_dir == "mipmap-xxxhdpi" || res_dir == "mipmap-xxhdpi" {
                let _ = client
                    .put_file_content(
                        &repo.owner,
                        &repo.name,
                        &format!("icon/{}.png", bundle.pkg_id),
                        icon_bytes,
                        "AEXT: Actualizar icono del paquete",
                        &branch,
                    )
                    .await;
            }
        }

        if uploaded == 0 {
            return Err(format!(
                "No se pudo subir la actualización a {}/{}: {}",
                repo.owner,
                repo.name,
                last_error.unwrap_or_else(|| "error desconocido de GitHub".to_string())
            ));
        }

        updated.github_repo_url = Some(repo.html_url.clone());
        updated.nekoyomi_repo_url = Some(format!(
            "https://raw.githubusercontent.com/{}/gh-pages/index.min.json",
            repo.full_name
        ));
        updated.apk_download_url = None;
    }

    {
        let mut list = state.extensions.write().await;
        list.retain(|x| {
            x.pkg_id != updated.pkg_id
                && x.id != updated.id
                && x.pkg_id != existing.pkg_id
                && x.id != existing.id
        });
        list.push(updated.clone());
    }
    state.persist_extensions().await;

    state
        .add_log(
            "SUCCESS",
            &format!("Extensión {} actualizada a v{}", updated.name, updated.version),
        )
        .await;

    Ok(updated)
}

#[tauri::command]
pub async fn delete_extension(pkg_id: String, state: State<'_, AppState>) -> Result<bool, String> {
    let mut list = state.extensions.write().await;
    let initial_len = list.len();
    let target_pkg = list
        .iter()
        .find(|e| e.pkg_id == pkg_id || e.id == pkg_id)
        .map(|e| e.pkg_id.clone());
    list.retain(|e| e.pkg_id != pkg_id && e.id != pkg_id);
    let removed = list.len() < initial_len;
    drop(list);

    if removed {
        // Borrar también los archivos locales para que el ZIP no quede huérfano.
        if let Some(pkg) = target_pkg {
            let dir = local_extension_dir(&state, &pkg);
            let _ = fs::remove_dir_all(dir);
        }
        state.persist_extensions().await;
        state
            .add_log("INFO", &format!("Extensión {} eliminada del catálogo", pkg_id))
            .await;
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

#[tauri::command]
pub async fn check_aext_updates() -> Result<serde_json::Value, String> {
    let current_version = env!("CARGO_PKG_VERSION");
    let client = reqwest::Client::builder()
        .user_agent("AEXT-Updater/1.0")
        .build()
        .map_err(|e| e.to_string())?;

    let url = "https://api.github.com/repos/PepeTheCarton17/AEXTensions_box/releases/latest";
    match client.get(url).send().await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                let tag_name = json
                    .get("tag_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let clean_tag = tag_name.trim_start_matches('v').trim();
                let release_notes = json
                    .get("body")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let release_url = json
                    .get("html_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("https://github.com/PepeTheCarton17/AEXTensions_box/releases")
                    .to_string();

                let latest_version = if clean_tag.is_empty() {
                    current_version.to_string()
                } else {
                    clean_tag.to_string()
                };
                let update_available = version_is_newer(&latest_version, current_version);

                let notes = if update_available {
                    release_notes
                } else {
                    "Estás ejecutando la versión más reciente.".to_string()
                };

                return Ok(serde_json::json!({
                    "current_version": current_version,
                    "latest_version": latest_version,
                    "tag_name": if tag_name.is_empty() { format!("v{}", current_version) } else { tag_name.to_string() },
                    "update_available": update_available,
                    "release_notes": notes,
                    "release_url": release_url
                }));
            }
        }
        Ok(resp) => {
            return Ok(serde_json::json!({
                "current_version": current_version,
                "latest_version": current_version,
                "tag_name": format!("v{}", current_version),
                "update_available": false,
                "release_notes": format!("GitHub respondió HTTP {}. No se pudo comprobar la última release.", resp.status()),
                "release_url": "https://github.com/PepeTheCarton17/AEXTensions_box/releases",
                "error": true
            }));
        }
        Err(e) => {
            return Ok(serde_json::json!({
                "current_version": current_version,
                "latest_version": current_version,
                "tag_name": format!("v{}", current_version),
                "update_available": false,
                "release_notes": format!("No se pudo conectar con GitHub: {}", e),
                "release_url": "https://github.com/PepeTheCarton17/AEXTensions_box/releases",
                "error": true
            }));
        }
    }

    Ok(serde_json::json!({
        "current_version": current_version,
        "latest_version": current_version,
        "tag_name": format!("v{}", current_version),
        "update_available": false,
        "release_notes": "No se pudo leer la respuesta de GitHub.",
        "release_url": "https://github.com/PepeTheCarton17/AEXTensions_box/releases",
        "error": true
    }))
}

// ============================== HELPERS ==============================

pub fn local_extension_dir(state: &AppState, pkg_id: &str) -> std::path::PathBuf {
    state.storage_dir.join("extensions").join(pkg_id)
}

/// Al reutilizar un repositorio con varias extensiones, añade el `include()`
/// del nuevo módulo sin borrar los que ya existían.
async fn merge_existing_settings(
    client: &GithubApiClient,
    repo: &RepoInfo,
    bundle: &mut crate::engine::ExtensionBundle,
) {
    if repo.empty {
        return;
    }

    let existing = match client
        .get_file_content(
            &repo.owner,
            &repo.name,
            "settings.gradle.kts",
            &repo.default_branch,
        )
        .await
    {
        Ok(Some(content)) => content,
        _ => return,
    };

    let include = format!("include(\":{}\")", bundle.module_path.replace('/', ":"));
    if existing.contains(&include) {
        return;
    }

    let mut merged = existing.trim_end().to_string();
    merged.push('\n');
    merged.push_str(&include);
    merged.push('\n');
    bundle
        .files
        .insert("settings.gradle.kts".to_string(), merged.into_bytes());
}

fn write_bundle_to_dir(
    target_dir: &Path,
    files: &std::collections::HashMap<String, Vec<u8>>,
) -> Result<(), String> {
    fs::create_dir_all(target_dir)
        .map_err(|e| format!("No se pudo crear {}: {}", target_dir.display(), e))?;
    for (rel_path, content) in files {
        let file_path = target_dir.join(rel_path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("No se pudo crear {}: {}", parent.display(), e))?;
        }
        fs::write(&file_path, content)
            .map_err(|e| format!("No se pudo escribir {}: {}", file_path.display(), e))?;
    }
    Ok(())
}

fn write_icons_to_dir(
    target_dir: &Path,
    module_path: &str,
    mipmaps: &std::collections::HashMap<String, Vec<u8>>,
) -> Result<(), String> {
    for (res_dir, icon_bytes) in mipmaps {
        let icon_path = target_dir
            .join(module_path)
            .join("src")
            .join("main")
            .join("res")
            .join(res_dir)
            .join("ic_launcher.png");
        if let Some(parent) = icon_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("No se pudo crear {}: {}", parent.display(), e))?;
        }
        fs::write(&icon_path, icon_bytes)
            .map_err(|e| format!("No se pudo escribir {}: {}", icon_path.display(), e))?;
    }
    Ok(())
}

fn rewrite_readme_for_repo(
    files: &mut std::collections::HashMap<String, Vec<u8>>,
    repo_full_name: &str,
) {
    let Some(readme) = files.get_mut("README.md") else {
        return;
    };
    let text = String::from_utf8_lossy(readme).to_string();
    let mut encoded = String::new();
    for byte in repo_full_name.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char)
            }
            _ => encoded.push_str(&format!("%{:02X}", byte)),
        }
    }
    let replaced = text
        .replace("PepeTheCarton17%2F{{REPO_NAME}}", &encoded)
        .replace("PepeTheCarton17/{{REPO_NAME}}", repo_full_name)
        .replace("PepeTheCarton17%2F{REPO_NAME}", &encoded)
        .replace("PepeTheCarton17/{REPO_NAME}", repo_full_name);
    *readme = replaced.into_bytes();
}

pub fn parse_repo_target(raw: &str) -> (Option<String>, String) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return (None, String::new());
    }

    let without_git = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    let without_scheme = without_git
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("ssh://")
        .trim_start_matches("git@")
        .trim_start_matches("www.");
    let without_host = without_scheme
        .trim_start_matches("github.com/")
        .trim_start_matches("github.com:")
        .trim_end_matches('/');

    let parts: Vec<&str> = without_host
        .split('/')
        .filter(|p| !p.trim().is_empty())
        .collect();

    match parts.as_slice() {
        [] => (None, String::new()),
        [repo] => (None, (*repo).to_string()),
        [owner, repo, ..] => (Some((*owner).to_string()), (*repo).to_string()),
    }
}

pub fn parse_github_url(url: &str) -> Option<(String, String)> {
    let (owner, repo) = parse_repo_target(url);
    let owner = owner?;
    if repo.is_empty() {
        None
    } else {
        Some((owner, repo))
    }
}

fn sanitize_repo_name(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.trim().chars() {
        match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => out.push(ch),
            _ => out.push('-'),
        }
    }
    let trimmed = out.trim_matches(|c| c == '-' || c == '.').to_string();
    if trimmed.is_empty() {
        "nekoyomi-extensions".to_string()
    } else {
        trimmed
    }
}

fn version_is_newer(latest: &str, current: &str) -> bool {
    let latest_parts = parse_version(latest);
    let current_parts = parse_version(current);
    let len = latest_parts.len().max(current_parts.len());
    for i in 0..len {
        let l = *latest_parts.get(i).unwrap_or(&0);
        let c = *current_parts.get(i).unwrap_or(&0);
        if l > c {
            return true;
        }
        if l < c {
            return false;
        }
    }
    false
}

fn parse_version(value: &str) -> Vec<u64> {
    value
        .trim()
        .trim_start_matches('v')
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_repo_targets() {
        assert_eq!(parse_repo_target(""), (None, String::new()));
        assert_eq!(
            parse_repo_target("unbox_extensions"),
            (None, "unbox_extensions".to_string())
        );
        assert_eq!(
            parse_repo_target("PepeTheCarton17/unbox_extensions"),
            (
                Some("PepeTheCarton17".to_string()),
                "unbox_extensions".to_string()
            )
        );
        assert_eq!(
            parse_repo_target("https://github.com/PepeTheCarton17/unbox_extensions.git"),
            (
                Some("PepeTheCarton17".to_string()),
                "unbox_extensions".to_string()
            )
        );
    }

    #[test]
    fn compares_versions() {
        assert!(version_is_newer("1.2.0", "1.1.9"));
        assert!(!version_is_newer("1.0.0", "1.0.0"));
        assert!(!version_is_newer("1.0.0", "1.0.1"));
    }
}
