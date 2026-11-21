use crate::engine::{AnalysisResult, ExtensionGenerator, IconEngine, SiteAnalyzer};
use crate::engine::generator::{
    decode_base64, generate_repo_readme, GRADLEW_BAT, GRADLEW_SH, GRADLE_WRAPPER_JAR_B64,
    GRADLE_WRAPPER_PROPERTIES, ROOT_BUILD_GRADLE,
};
use crate::github::{GithubApiClient, GithubAuthManager, RepoInfo, RepoSummary};
use crate::models::{
    ExtensionItem, ExtractorTestResult, GenerationMode, GenerationRequest, GithubAccountInfo,
    LogEntry, RepoCheckResult, SystemStatus,
};
use crate::state::AppState;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use tauri::State;

const LOCAL_API_BASE: &str = "http://localhost:45731";

// Mensajes de commit genéricos: nunca incluir el package ni el nombre de la
// extensión (p. ej. eu.kanade.tachiyomi.animeextension.es.pluto3). El historial
// del repo destino debe hablar de AEXT, no del contenido generado.
const COMMIT_MSG_GENERATE: &str = "AEXT: add generated extension";
const COMMIT_MSG_UPDATE: &str = "AEXT: update generated extension";
const COMMIT_MSG_DELETE: &str = "AEXT: remove generated extension";

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

        // Avisos honestos del análisis: el usuario debe saber qué no se pudo
        // detectar antes de publicar o compilar.
        for warning in &analysis.warnings {
            state
                .add_log("WARN", &format!("{}: {}", analysis.name, warning))
                .await;
        }

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
            build_status: None,
            build_error: None,
            build_apk_path: None,
            actions_run_url: None,
            actions_commit_sha: None,
            icon_url: None,
            repo_state: None,
            repo_checked_at: None,
            repo_deleted_at: None,
            repo_ignore_missing: false,
        };

        // Datos del commit subido a GitHub, para verificar el run de Actions
        // después de que el modo GitHub haya terminado de subir el código.
        let mut actions_commit_sha: Option<String> = None;
        let mut actions_client: Option<GithubApiClient> = None;
        let mut actions_owner = String::new();
        let mut actions_repo = String::new();

        match request.mode {
            GenerationMode::LocalOnly => {
                let target_dir = local_extension_dir(&state, &bundle.pkg_id);
                // Limpiar restos de una generación anterior (estructura antigua).
                let _ = fs::remove_dir_all(&target_dir);
                prepare_bundle_for_local(&mut bundle.files);
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

                // El README genérico solo se usa si el repo no tiene uno propio
                // (o tiene únicamente el placeholder de `auto_init`).
                bundle.files.insert(
                    "README.md".to_string(),
                    generate_repo_readme(&repo.owner, &repo.name, &repo.html_url).into_bytes(),
                );
                // Comprobar los archivos del scaffold uno por uno: un repo
                // recién creado con `auto_init` NO está vacío, pero sí necesita
                // workflow, wrapper, keystore y build files. En cambio, los que
                // ya existen pertenecen al usuario y no se tocan.
                reconcile_repo_files(&client, &repo, &mut bundle).await?;

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

                // Un único commit con todos los archivos e iconos. Antes cada
                // archivo producía su propio commit y, con él, un run de Actions
                // distinto: decenas de runs (casi todos cancelados) y mucho ruido.
                let mut all_files: Vec<(String, Vec<u8>)> = bundle
                    .files
                    .iter()
                    .map(|(path, content)| (path.clone(), content.clone()))
                    .collect();
                for (res_dir, icon_bytes) in &mipmaps {
                    all_files.push((
                        format!(
                            "{}/src/main/res/{}/ic_launcher.png",
                            bundle.module_path, res_dir
                        ),
                        icon_bytes.clone(),
                    ));
                    if res_dir == "mipmap-xxxhdpi" || res_dir == "mipmap-xxhdpi" {
                        all_files.push((format!("icon/{}.png", bundle.pkg_id), icon_bytes.clone()));
                        all_files.push(("icon/ic_launcher.png".to_string(), icon_bytes.clone()));
                    }
                }
                let branch = repo.default_branch.clone();
                let commit_sha = client
                    .commit_files(
                        &repo.owner,
                        &repo.name,
                        &all_files,
                        COMMIT_MSG_GENERATE,
                        &branch,
                    )
                    .await
                    .map_err(|e| format!("No se pudo subir la extensión a GitHub: {}", e))?;
                actions_owner = repo.owner.clone();
                actions_repo = repo.name.clone();
                actions_commit_sha = Some(commit_sha.clone());
                ext_item.actions_commit_sha = Some(commit_sha.clone());
                actions_client = Some(client);
                state
                    .add_log(
                        "INFO",
                        &format!(
                            "Subidos {} archivos en un solo commit ({}) a {}/{}",
                            all_files.len(),
                            &commit_sha[..commit_sha.len().min(7)],
                            repo.owner,
                            repo.name
                        ),
                    )
                    .await;

                // Copia local del proyecto: es la fuente del ZIP de código y
                // permite ver los archivos sin clonar el repositorio.
                let target_dir = local_extension_dir(&state, &ext_item.pkg_id);
                let _ = fs::remove_dir_all(&target_dir);
                write_bundle_to_dir(&target_dir, &bundle.files)?;
                write_icons_to_dir(&target_dir, &bundle.module_path, &mipmaps)?;
                write_repo_icons(&target_dir, &ext_item.pkg_id, &mipmaps)?;

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
                        "INFO",
                        &format!(
                            "Código de {} subido a GitHub: {}. Verificando el resultado de Actions…",
                            ext_item.pkg_id, repo.html_url
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

        // Comprobación REAL: localizar el run disparado por nuestro commit y
        // reflejar su conclusión. AEXT ya no dice "funcionó" solo por subir el
        // código; si Actions falla, se muestra el error y el enlace al run.
        if let (Some(sha), Some(gh_client)) = (actions_commit_sha.take(), actions_client.take()) {
            verify_github_actions(&state, &gh_client, &actions_owner, &actions_repo, &sha, &ext_item.pkg_id).await;
            if let Some(updated) = state
                .extensions
                .read()
                .await
                .iter()
                .find(|x| x.pkg_id == ext_item.pkg_id)
                .cloned()
            {
                ext_item = updated;
            }
        }
        created.push(ext_item);
    }

    Ok(created)
}

// ============================== COMANDOS GESTIÓN DE EXTENSIONES ==============================

#[tauri::command]
pub async fn get_created_extensions(state: State<'_, AppState>) -> Result<Vec<ExtensionItem>, String> {
    let mut list = state.extensions.read().await.clone();
    for ext in &mut list {
        // Entradas antiguas: antes AEXT marcaba "success" en cuanto subía el
        // código, sin mirar Actions. Sin run verificado no podemos afirmar que
        // compiló, así que no mostramos el badge verde.
        if ext.github_repo_url.is_some()
            && ext.actions_run_url.is_none()
            && ext.build_apk_path.is_none()
            && ext.build_status.as_deref() == Some("success")
        {
            ext.build_status = None;
        }
        // Un «compilando» persistido tras cerrar la app no es fiable: lo
        // mostramos como pendiente y invitamos a verificar el run real.
        if ext.github_repo_url.is_some()
            && ext.actions_commit_sha.is_some()
            && ext.build_status.as_deref() == Some("building")
        {
            ext.build_status = Some("pending".to_string());
            if ext.build_error.is_none() {
                ext.build_error = Some(
                    "La compilación no terminó en la sesión anterior. Pulsa «Verificar Actions» para consultar el run real.".to_string(),
                );
            }
        }
    }
    Ok(list)
}

/// Regenera una extensión existente (re-analiza su sitio, sube la versión y
/// reescribe los archivos locales y, si aplica, el repositorio de GitHub).
#[tauri::command]
async fn update_extension_inner(
    pkg_id: String,
    state: &AppState,
) -> Result<ExtensionItem, String> {
    let existing = {
        let list = state.extensions.read().await;
        list.iter()
            .find(|e| e.pkg_id == pkg_id || e.id == pkg_id)
            .cloned()
            .ok_or_else(|| format!("No se encontró la extensión {}", pkg_id))?
    };

    if existing.repo_state.as_deref() == Some("missing") && !existing.repo_ignore_missing {
        return Err(
            "Esta extensión figura como eliminada del repositorio. Restaura su ficha antes de actualizarla.".to_string(),
        );
    }

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
    let has_repo = existing.github_repo_url.is_some();
    if !has_repo {
        // Sin repositorio no hay rama `gh-pages`: el bundle local solo lleva
        // lo necesario para compilar (sin .github/, icon/ ni README con
        // placeholders de repo).
        prepare_bundle_for_local(&mut bundle.files);
    }
    write_bundle_to_dir(&target_dir, &bundle.files)?;
    write_icons_to_dir(&target_dir, &bundle.module_path, &mipmaps)?;
    if has_repo {
        write_repo_icons(&target_dir, &bundle.pkg_id, &mipmaps)?;
    }
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
    let mut update_actions: Option<(GithubApiClient, String, String, String)> = None;
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

        // Igual que al generar: completar el scaffold solo donde falte y
        // respetar README, firma, workflow y build files que ya existan.
        bundle.files.insert(
            "README.md".to_string(),
            generate_repo_readme(&repo.owner, &repo.name, &repo.html_url).into_bytes(),
        );
        reconcile_repo_files(&client, &repo, &mut bundle).await?;

        let mut all_files: Vec<(String, Vec<u8>)> = bundle
            .files
            .iter()
            .map(|(path, content)| (path.clone(), content.clone()))
            .collect();
        for (res_dir, icon_bytes) in &mipmaps {
            all_files.push((
                format!(
                    "{}/src/main/res/{}/ic_launcher.png",
                    bundle.module_path, res_dir
                ),
                icon_bytes.clone(),
            ));
            if res_dir == "mipmap-xxxhdpi" || res_dir == "mipmap-xxhdpi" {
                all_files.push((format!("icon/{}.png", bundle.pkg_id), icon_bytes.clone()));
                all_files.push(("icon/ic_launcher.png".to_string(), icon_bytes.clone()));
            }
        }
        let branch = repo.default_branch.clone();
        let commit_sha = client
            .commit_files(
                &repo.owner,
                &repo.name,
                &all_files,
                COMMIT_MSG_UPDATE,
                &branch,
            )
            .await
            .map_err(|e| format!("No se pudo actualizar GitHub: {}", e))?;
        updated.actions_commit_sha = Some(commit_sha.clone());
        update_actions = Some((
            client,
            repo.owner.clone(),
            repo.name.clone(),
            commit_sha.clone(),
        ));
        state
            .add_log(
                "INFO",
                &format!(
                    "Actualización subida en un solo commit ({}) a {}/{}. Verificando Actions…",
                    &commit_sha[..commit_sha.len().min(7)],
                    repo.owner,
                    repo.name
                ),
            )
            .await;

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

    // El estado final lo decide GitHub Actions, no la subida del código.
    if let Some((gh_client, owner, repo_name, sha)) = update_actions {
        verify_github_actions(&state, &gh_client, &owner, &repo_name, &sha, &updated.pkg_id)
            .await;
        if let Some(latest) = state
            .extensions
            .read()
            .await
            .iter()
            .find(|e| e.pkg_id == updated.pkg_id || e.id == updated.id)
            .cloned()
        {
            updated = latest;
        }
    }

    state
        .add_log(
            "SUCCESS",
            &format!("Extensión {} actualizada a v{}", updated.name, updated.version),
        )
        .await;

    Ok(updated)
}

/// El índice publicado (`gh-pages/index.min.json`) es la fuente de verdad de
/// las variantes que GitHub ya compila. Si AEXT no las adopta, `update_extension`
/// no puede saber que existen y su versión se queda congelada para siempre.
/// Se llama antes de cada actualización para que la lista local se reconcilie
/// sola, sin intervención manual.
async fn sync_published_variants(state: &AppState) -> usize {
    let repos: Vec<String> = {
        let list = state.extensions.read().await;
        list.iter()
            .filter_map(|e| e.github_repo_url.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    };

    let client = reqwest::Client::builder()
        .user_agent(concat!("AEXT/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_default();
    let mut adopted = 0usize;
    let mut repaired = 0usize;

    for repo_url in repos {
        let Some((owner, repo_name)) = parse_github_url(&repo_url) else {
            continue;
        };
        let index_url = format!(
            "https://raw.githubusercontent.com/{}/{}/gh-pages/index.min.json",
            owner, repo_name
        );
        let Ok(resp) = client.get(&index_url).send().await else {
            continue;
        };
        if !resp.status().is_success() {
            continue;
        }
        let Ok(json) = resp.json::<serde_json::Value>().await else {
            continue;
        };
        let Some(entries) = json.as_array() else {
            continue;
        };

        for item in entries {
            let Some(pkg_id) = item.get("pkg").and_then(|v| v.as_str()) else {
                continue;
            };
            let lang = item
                .get("lang")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("all");
            let version = item
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let version_code = item
                .get("code")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            if version.is_empty() || version_code <= 0 {
                continue;
            }
            let indexed_name = item
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(pkg_id);
            let raw_name = indexed_name
                .strip_prefix("Aniyomi: ")
                .unwrap_or(indexed_name)
                .to_string();
            let class_name: String = raw_name
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect();
            if class_name.is_empty() {
                continue;
            }
            let base_url = item
                .get("sources")
                .and_then(|v| v.as_array())
                .and_then(|sources| sources.first())
                .and_then(|source| source.get("baseUrl").and_then(|v| v.as_str()))
                .unwrap_or("");
            let apk = item
                .get("apk")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|apk| {
                    format!(
                        "https://raw.githubusercontent.com/{}/{}/gh-pages/{}",
                        owner, repo_name, apk
                    )
                });
            let icon_url = format!(
                "https://raw.githubusercontent.com/{}/{}/gh-pages/icon/{}.png",
                owner, repo_name, pkg_id
            );

            let mut list = state.extensions.write().await;
            if let Some(existing) = list
                .iter_mut()
                .find(|e| e.pkg_id == pkg_id || e.id == pkg_id)
            {
                // Variantes ya adoptadas por versiones anteriores: completamos
                // solo los datos que faltaban, sin sobrescribir nada del usuario.
                let mut changed = false;
                if existing.icon_url.is_none() {
                    existing.icon_url = Some(icon_url);
                    changed = true;
                }
                if existing.apk_download_url.is_none() {
                    existing.apk_download_url = apk.clone();
                    changed = true;
                }
                if existing.nekoyomi_repo_url.is_none() {
                    existing.nekoyomi_repo_url = Some(index_url.clone());
                    changed = true;
                }
                // El índice vuelve a publicar esta variante: su módulo existe
                // otra vez, así que no debe seguir oculta como "eliminada".
                if existing.repo_state.as_deref() == Some("missing") {
                    existing.repo_state = Some("present".to_string());
                    existing.repo_deleted_at = None;
                    existing.repo_ignore_missing = false;
                    changed = true;
                }
                if version_code > existing.version_code {
                    existing.version = version.to_string();
                    existing.version_code = version_code;
                    changed = true;
                }
                if changed {
                    repaired += 1;
                }
                continue;
            }
            list.push(ExtensionItem {
                id: pkg_id.to_string(),
                pkg_id: pkg_id.to_string(),
                name: raw_name,
                class_name,
                base_url: base_url.to_string(),
                lang: lang.to_string(),
                ext_type: "anime".to_string(),
                version: version.to_string(),
                version_code,
                cms: "generic".to_string(),
                has_sub_dub: false,
                icon_base64: None,
                github_repo_url: Some(repo_url.clone()),
                nekoyomi_repo_url: Some(index_url.clone()),
                apk_download_url: apk,
                created_at: chrono::Utc::now().to_rfc3339(),
                build_status: None,
                build_error: None,
                build_apk_path: None,
                actions_run_url: None,
                actions_commit_sha: None,
                icon_url: Some(icon_url),
                repo_state: None,
                repo_checked_at: None,
                repo_deleted_at: None,
                repo_ignore_missing: false,
            });
            adopted += 1;
        }
    }

    if adopted > 0 || repaired > 0 {
        state.persist_extensions().await;
        state
            .add_log(
                "INFO",
                &format!(
                    "Sincronización del índice: {} variante(s) adoptadas, {} ficha(s) completadas",
                    adopted, repaired
                ),
            )
            .await;
    }
    adopted
}

#[tauri::command]
pub async fn sync_extension_catalog(state: State<'_, AppState>) -> Result<usize, String> {
    Ok(sync_published_variants(&state).await)
}

/// Deriva la ruta `src/<lang>/<slug>` del package id. Es la misma convención
/// que usa el generador y el borrado remoto; validar los segmentos evita
/// construir rutas arbitrarias a partir de datos del catálogo.
fn extension_module_path(pkg_id: &str) -> Option<String> {
    let parts: Vec<&str> = pkg_id.split('.').collect();
    if parts.len() < 2 {
        return None;
    }
    let lang = parts[parts.len() - 2];
    let slug = parts[parts.len() - 1];
    if lang.is_empty()
        || slug.is_empty()
        || !lang.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        || !slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }
    Some(format!("src/{}/{}", lang, slug))
}

/// Comprueba, contra la rama por defecto de cada repositorio, si el módulo de
/// cada extensión sigue existiendo. Es genérico: no conoce nombres ni tipos,
/// solo la convención `src/<lang>/<slug>` del package id. Un fallo de red o de
/// autenticación NO marca nada como eliminado; solo se aplica un 404 real.
#[tauri::command]
pub async fn check_extensions_in_repo(
    state: State<'_, AppState>,
) -> Result<RepoCheckResult, String> {
    let auth = GithubAuthManager::new();
    let token = auth.get_saved_token().ok_or_else(|| {
        "Se requiere iniciar sesión en GitHub para comprobar el estado de los módulos.".to_string()
    })?;
    let client = GithubApiClient::new(token);

    let targets: Vec<(String, String, String)> = {
        let list = state.extensions.read().await;
        list.iter()
            .filter_map(|ext| {
                let repo_url = ext.github_repo_url.as_deref()?;
                let module_path = extension_module_path(&ext.pkg_id)?;
                Some((
                    ext.pkg_id.clone(),
                    repo_url.trim_end_matches('/').to_string(),
                    module_path,
                ))
            })
            .collect()
    };

    let mut result = RepoCheckResult {
        checked: 0,
        present: 0,
        missing: 0,
        errors: 0,
    };
    if targets.is_empty() {
        return Ok(result);
    }

    let mut repos: HashMap<String, (String, String, String)> = HashMap::new();
    let now = chrono::Utc::now().to_rfc3339();
    let mut dirty = false;

    for (pkg_id, repo_url, module_path) in targets {
        let repo_info = match repos.get(&repo_url) {
            Some(info) => info.clone(),
            None => {
                let Some((owner, name)) = parse_github_url(&repo_url) else {
                    result.errors += 1;
                    continue;
                };
                match client.get_repository(&owner, &name).await {
                    Ok(info) => {
                        let info = (info.owner, info.name, info.default_branch);
                        repos.insert(repo_url.clone(), info.clone());
                        info
                    }
                    Err(_) => {
                        result.errors += 1;
                        continue;
                    }
                }
            }
        };
        result.checked += 1;
        let marker = format!("{}/build.gradle.kts", module_path);
        let exists = match client
            .file_exists(&repo_info.0, &repo_info.1, &marker, &repo_info.2)
            .await
        {
            Ok(exists) => exists,
            Err(_) => {
                result.errors += 1;
                continue;
            }
        };

        let mut list = state.extensions.write().await;
        let Some(ext) = list.iter_mut().find(|e| e.pkg_id == pkg_id) else {
            continue;
        };
        if exists {
            result.present += 1;
            let changed = ext.repo_state.as_deref() != Some("present")
                || ext.repo_deleted_at.is_some()
                || ext.repo_ignore_missing;
            ext.repo_state = Some("present".to_string());
            ext.repo_checked_at = Some(now.clone());
            ext.repo_deleted_at = None;
            ext.repo_ignore_missing = false;
            dirty |= changed;
        } else {
            ext.repo_checked_at = Some(now.clone());
            // Si el usuario restauró la ficha a sabiendas, se respeta: no se
            // vuelve a mover a "Eliminadas" hasta que el módulo reaparezca.
            if !ext.repo_ignore_missing {
                if ext.repo_state.as_deref() != Some("missing") {
                    ext.repo_state = Some("missing".to_string());
                    if ext.repo_deleted_at.is_none() {
                        ext.repo_deleted_at = Some(now.clone());
                    }
                    dirty = true;
                }
                result.missing += 1;
            }
        }
        drop(list);
    }

    if dirty {
        state.persist_extensions().await;
    }
    if result.missing > 0 {
        state
            .add_log(
                "WARN",
                &format!(
                    "{} módulo(s) ya no existen en su repositorio; pasan a «Eliminadas»",
                    result.missing
                ),
            )
            .await;
    }
    Ok(result)
}

/// Restaura una ficha marcada como eliminada. No crea ni sube archivos: solo
/// devuelve la extensión a la lista activa y recuerda la decisión del usuario
/// para que el siguiente chequeo no la vuelva a ocultar.
#[tauri::command]
pub async fn restore_extension_repo_state(
    pkg_id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let restored = {
        let mut list = state.extensions.write().await;
        let Some(ext) = list.iter_mut().find(|e| e.pkg_id == pkg_id || e.id == pkg_id) else {
            return Ok(false);
        };
        ext.repo_state = None;
        ext.repo_deleted_at = None;
        ext.repo_checked_at = None;
        ext.repo_ignore_missing = true;
        true
    };
    if restored {
        state.persist_extensions().await;
        state
            .add_log(
                "SUCCESS",
                &format!("Ficha de {} restaurada en el catálogo de AEXT", pkg_id),
            )
            .await;
    }
    Ok(restored)
}

#[tauri::command]
pub async fn update_extension(
    pkg_id: String,
    state: State<'_, AppState>,
) -> Result<ExtensionItem, String> {
    sync_published_variants(&state).await;
    update_extension_inner(pkg_id, &state).await
}

/// Actualiza todas las extensiones visibles o, si se indica un repo, todas las
/// que apuntan a ese mismo repositorio. Se hacen en secuencia: así cada commit
/// usa el SHA base correcto y no aparece el 422 de escrituras concurrentes.
#[tauri::command]
pub async fn update_all_extensions(
    repo_url: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ExtensionItem>, String> {
    sync_published_variants(&state).await;

    let targets: Vec<String> = {
        let list = state.extensions.read().await;
        list.iter()
            .filter(|ext| {
                if ext.repo_state.as_deref() == Some("missing") && !ext.repo_ignore_missing {
                    return false;
                }
                match repo_url.as_deref() {
                    None => true,
                    Some(selected) => {
                        let selected = selected.trim_end_matches('/');
                        ext.github_repo_url
                            .as_deref()
                            .map(|url| url.trim_end_matches('/') == selected)
                            .unwrap_or(false)
                    }
                }
            })
            .map(|ext| ext.pkg_id.clone())
            .collect()
    };

    if targets.is_empty() {
        return Err("No hay extensiones que coincidan con ese repositorio".to_string());
    }

    state
        .add_log(
            "INFO",
            &format!("Actualizando {} extensión(es) en secuencia", targets.len()),
        )
        .await;

    let mut updated = Vec::new();
    let mut failures = Vec::new();
    for pkg_id in targets {
        match update_extension_inner(pkg_id.clone(), &state).await {
            Ok(item) => updated.push(item),
            Err(err) => {
                state
                    .add_log("ERROR", &format!("No se pudo actualizar {}: {}", pkg_id, err))
                    .await;
                failures.push(format!("{}: {}", pkg_id, err));
            }
        }
    }
    if updated.is_empty() {
        return Err(failures.join("; "));
    }
    if !failures.is_empty() {
        state
            .add_log(
                "WARN",
                &format!("{} extensión(es) no se actualizaron: {}", failures.len(), failures.join("; ")),
            )
            .await;
    }
    Ok(updated)
}

async fn delete_local_extension(pkg_id: &str, state: &AppState) -> Result<bool, String> {
    let target_pkg = {
        let list = state.extensions.read().await;
        list.iter()
            .find(|e| e.pkg_id == pkg_id || e.id == pkg_id)
            .map(|e| e.pkg_id.clone())
    };
    let Some(pkg) = target_pkg else {
        return Ok(false);
    };

    // Borrar primero los archivos. Si falla, NO quitamos la entrada del
    // catálogo: antes se eliminaba de la lista aunque la carpeta quedara en
    // disco (huérfanos como `...videospornoypelículas...`).
    let dir = local_extension_dir(state, &pkg);
    if dir.is_dir() {
        // Los directorios de Gradle suelen tener archivos bloqueados; limpiar
        // primero los derivados y reintentar aumenta mucho la tasa de éxito.
        let _ = remove_dir_with_retries(&dir.join("build"));
        let _ = remove_dir_with_retries(&dir.join(".gradle"));
        remove_dir_with_retries(&dir).map_err(|e| {
            format!(
                "No se pudo borrar la carpeta de {} ({}). Ciérrala en el explorador y vuelve a intentarlo.",
                pkg, e
            )
        })?;
    }

    let mut list = state.extensions.write().await;
    let initial_len = list.len();
    list.retain(|e| e.pkg_id != pkg_id && e.id != pkg_id);
    let removed = list.len() < initial_len;
    drop(list);

    if removed {
        state.persist_extensions().await;
        state
            .add_log(
                "SUCCESS",
                &format!("Extensión {} y su carpeta eliminadas", pkg_id),
            )
            .await;
    }
    Ok(removed)
}

#[tauri::command]
pub async fn delete_extension(pkg_id: String, state: State<'_, AppState>) -> Result<bool, String> {
    delete_local_extension(&pkg_id, &state).await
}

/// Borra SOLO el módulo de la extensión de GitHub y después su copia local.
/// Nunca elimina el repositorio ni sus archivos comunes (README, licencia,
/// workflow, wrapper, build files u otros módulos).
#[tauri::command]
pub async fn delete_extension_github(
    pkg_id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let ext = {
        let list = state.extensions.read().await;
        list.iter()
            .find(|e| e.pkg_id == pkg_id || e.id == pkg_id)
            .cloned()
    };
    let Some(ext) = ext else {
        return Ok(false);
    };
    let repo_url = ext.github_repo_url.as_deref().ok_or_else(|| {
        "Esta extensión no tiene un repositorio de GitHub asociado; usa solo Eliminar extensión local.".to_string()
    })?;
    let (owner, repo_name) = parse_github_url(repo_url).ok_or_else(|| {
        format!("No se pudo interpretar la URL de GitHub: {}", repo_url)
    })?;

    let module_path = extension_module_path(&ext.pkg_id).ok_or_else(|| {
        "La ruta del módulo calculada no es segura; no se borró nada".to_string()
    })?;

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
                return Err("Se requiere iniciar sesión en GitHub para borrar el módulo remoto.".to_string());
            }
        },
    };
    let client = GithubApiClient::new(token);
    let repo = client.get_repository(&owner, &repo_name).await?;

    state
        .add_log(
            "INFO",
            &format!(
                "Borrando de GitHub solo el módulo {} en {}/{}…",
                module_path, owner, repo_name
            ),
        )
        .await;

    let result = client
        .delete_module(
            &repo.owner,
            &repo.name,
            &module_path,
            COMMIT_MSG_DELETE,
            &repo.default_branch,
        )
        .await
        .map_err(|e| format!("No se pudo borrar el módulo de GitHub: {}", e))?;

    match result {
        Some(sha) => {
            state
                .add_log(
                    "SUCCESS",
                    &format!(
                        "Módulo {} eliminado de GitHub en el commit {}",
                        module_path,
                        &sha[..sha.len().min(7)]
                    ),
                )
                .await;
        }
        None => {
            state
                .add_log(
                    "WARN",
                    &format!("El módulo {} ya no existía en GitHub", module_path),
                )
                .await;
        }
    }

    delete_local_extension(&ext.pkg_id, &state).await
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

                let assets: Vec<serde_json::Value> = json
                    .get("assets")
                    .and_then(|v| v.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .map(|asset| {
                                serde_json::json!({
                                    "name": asset.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                                    "url": asset.get("browser_download_url").and_then(|v| v.as_str()).unwrap_or(""),
                                    "size": asset.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
                                    "content_type": asset.get("content_type").and_then(|v| v.as_str()).unwrap_or("")
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                return Ok(serde_json::json!({
                    "current_version": current_version,
                    "latest_version": latest_version,
                    "tag_name": if tag_name.is_empty() { format!("v{}", current_version) } else { tag_name.to_string() },
                    "update_available": update_available,
                    "release_notes": notes,
                    "release_url": release_url,
                    "assets": assets
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

// ============================== UTILIDADES DEL SISTEMA ==============================

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

    Ok(serde_json::json!({
        "path": path.display().to_string(),
        "bytes": bytes.len(),
        "filename": name,
    }))
}

/// Devuelve la ruta local del código de una extensión (para mostrarla aunque
/// el sistema no pueda abrir el explorador, p. ej. en Android).
#[tauri::command]
pub async fn open_extension_folder(
    state: State<'_, AppState>,
    pkg_id: String,
) -> Result<serde_json::Value, String> {
    let dir = local_extension_dir(&state, &pkg_id);
    if !dir.is_dir() {
        return Err(format!(
            "No hay copia local de esta extensión en {}",
            dir.display()
        ));
    }

    let path = dir.display().to_string();
    let opened = tauri_plugin_opener::open_path(&dir, None::<&str>).is_ok();

    Ok(serde_json::json!({ "path": path, "opened": opened }))
}

#[tauri::command]
pub async fn list_extension_backups(
    state: State<'_, AppState>,
    pkg_id: String,
) -> Result<Vec<crate::engine::backup::BackupInfo>, String> {
    let dir = local_extension_dir(&state, &pkg_id);
    Ok(crate::engine::backup::list_backups(&dir))
}

/// Restaura una copia de seguridad concreta ("versión original" o un parche
/// anterior). Es reversible: nunca borra archivos, solo los sobrescribe.
#[tauri::command]
pub async fn restore_extension_backup(
    state: State<'_, AppState>,
    pkg_id: String,
    backup: String,
) -> Result<serde_json::Value, String> {
    let dir = local_extension_dir(&state, &pkg_id);
    let count = crate::engine::backup::restore_backup(&dir, &backup)?;

    // La copia también revierte versionCode/versionName del build.gradle.kts:
    // sincronizamos la ficha de la extensión para que la UI no muestre una
    // versión que ya no existe en el código restaurado.
    let (restored_code, restored_name) = read_extension_versions(&dir);
    if restored_code.is_some() || restored_name.is_some() {
        let mut extensions = state.extensions.write().await;
        if let Some(ext) = extensions
            .iter_mut()
            .find(|e| e.pkg_id == pkg_id || e.id == pkg_id)
        {
            if let Some(code) = restored_code {
                ext.version_code = code;
            }
            if let Some(name) = &restored_name {
                ext.version = name.clone();
            }
        }
        drop(extensions);
        state.persist_extensions().await;
    }

    state
        .add_log(
            "SUCCESS",
            &format!(
                "Copia '{}' restaurada en {} ({} archivos)",
                backup, pkg_id, count
            ),
        )
        .await;
    Ok(serde_json::json!({
        "restored_files": count,
        "backup": backup,
        "version": restored_name,
        "version_code": restored_code,
    }))
}

/// Lee `versionCode` y `versionName` del primer módulo Gradle de la extensión.
fn read_extension_versions(dir: &std::path::Path) -> (Option<i32>, Option<String>) {
    let Ok(langs) = std::fs::read_dir(dir.join("src")) else {
        return (None, None);
    };
    for lang in langs.flatten() {
        let Ok(modules) = std::fs::read_dir(lang.path()) else {
            continue;
        };
        for module in modules.flatten() {
            let gradle = module.path().join("build.gradle.kts");
            let Ok(content) = std::fs::read_to_string(&gradle) else {
                continue;
            };
            let code = regex::Regex::new(r"versionCode\s*=\s*(\d+)")
                .ok()
                .and_then(|re| re.captures(&content))
                .and_then(|caps| caps.get(1))
                .and_then(|m| m.as_str().parse::<i32>().ok());
            let name = regex::Regex::new(r#"versionName\s*=\s*"([^"]*)""#)
                .ok()
                .and_then(|re| re.captures(&content))
                .and_then(|caps| caps.get(1))
                .map(|m| m.as_str().to_string());
            if code.is_some() || name.is_some() {
                return (code, name);
            }
        }
    }
    (None, None)
}

#[derive(serde::Serialize)]
pub struct BuildToolchain {
    pub os: String,
    pub java: bool,
    pub java_path: Option<String>,
    pub java_version: Option<String>,
    pub java_home: Option<String>,
    pub kotlinc: bool,
    pub kotlinc_version: Option<String>,
    pub gradle: bool,
    pub gradle_path: Option<String>,
    pub gradle_version: Option<String>,
    pub android_sdk: bool,
    pub android_sdk_path: Option<String>,
    pub can_build: bool,
    pub missing: Vec<String>,
    pub notes: Vec<String>,
}

#[tauri::command]
pub async fn check_build_toolchain() -> Result<BuildToolchain, String> {
    tauri::async_runtime::spawn_blocking(detect_toolchain)
        .await
        .map_err(|e| e.to_string())
}

/// Actualiza el estado de compilación que se muestra en "Mis extensiones".
async fn set_build_status(
    state: &AppState,
    pkg_id: &str,
    status: &str,
    error: Option<String>,
    apk_path: Option<String>,
) {
    {
        let mut list = state.extensions.write().await;
        if let Some(ext) = list
            .iter_mut()
            .find(|e| e.pkg_id == pkg_id || e.id == pkg_id)
        {
            ext.build_status = Some(status.to_string());
            if status == "building" {
                ext.build_error = None;
                ext.build_apk_path = None;
            } else {
                ext.build_error = error;
                ext.build_apk_path = apk_path;
            }
        }
    }
    state.persist_extensions().await;
}

/// Comprueba el run de GitHub Actions disparado por `commit_sha` y refleja su
/// resultado REAL. AEXT no afirma que la compilación funcionó solo porque el
/// código se subiera: espera a que GitHub devuelva `conclusion == success`.
async fn verify_github_actions(
    state: &AppState,
    client: &GithubApiClient,
    owner: &str,
    repo: &str,
    commit_sha: &str,
    pkg_id: &str,
) {
    // Un build de extensión tarda ~2-5 min en Actions. 90 s marcaba como
    // «pendiente» una actualización que luego terminaba en success, y el usuario
    // veía que «sólo se actualizaba» la extensión anterior. Esperamos lo suficiente
    // para el caso normal y dejamos el botón «Verificar Actions» para el resto.
    const MAX_WAIT_SECS: u64 = 360;
    const POLL_SECS: u64 = 10;
    const NO_RUN_GRACE_SECS: u64 = 30;
    let started = std::time::Instant::now();
    let short_sha = &commit_sha[..commit_sha.len().min(7)];
    let actions_url = format!("https://github.com/{}/{}/actions", owner, repo);
    let mut saw_run = false;
    let mut last_error = String::new();

    loop {
        match client
            .get_workflow_run_for_sha(owner, repo, commit_sha)
            .await
        {
            Ok(Some(run)) => {
                saw_run = true;
                last_error.clear();
                let finished = run.status == "completed";
                let conclusion = run.conclusion.clone().unwrap_or_default();
                {
                    let mut list = state.extensions.write().await;
                    if let Some(ext) = list
                        .iter_mut()
                        .find(|e| e.pkg_id == pkg_id || e.id == pkg_id)
                    {
                        ext.actions_run_url = Some(run.html_url.clone());
                        if finished {
                            if conclusion == "success" {
                                ext.build_status = Some("success".to_string());
                                ext.build_error = None;
                            } else {
                                ext.build_status = Some("error".to_string());
                                ext.build_error = Some(format!(
                                    "GitHub Actions: {} — {}",
                                    conclusion, run.html_url
                                ));
                                ext.build_apk_path = None;
                            }
                        } else {
                            ext.build_status = Some("building".to_string());
                            ext.build_error = None;
                        }
                    }
                }
                state.persist_extensions().await;

                if finished {
                    let level = if conclusion == "success" {
                        "SUCCESS"
                    } else {
                        "ERROR"
                    };
                    state
                        .add_log(
                            level,
                            &format!(
                                "GitHub Actions para {} ({}): {} — {}",
                                pkg_id, short_sha, conclusion, run.html_url
                            ),
                        )
                        .await;
                    return;
                }
            }
            Ok(None) => {
                let no_run_message = format!(
                    "GitHub Actions no inició ningún run para el commit {} en {}/{} tras {} s. Comprueba que exista `.github/workflows/build_push.yml` y que Actions esté habilitado en el repo; después pulsa «Verificar Actions». Revisa: {}",
                    short_sha, owner, repo, NO_RUN_GRACE_SECS, actions_url
                );
                let timed_out_no_run = !saw_run && started.elapsed().as_secs() >= NO_RUN_GRACE_SECS;
                {
                    let mut list = state.extensions.write().await;
                    if let Some(ext) = list.iter_mut().find(|e| e.pkg_id == pkg_id || e.id == pkg_id) {
                        if ext.actions_run_url.is_none() {
                            ext.actions_run_url = Some(actions_url.clone());
                        }
                        if timed_out_no_run {
                            ext.build_status = Some("error".to_string());
                            ext.build_error = Some(no_run_message.clone());
                            ext.build_apk_path = None;
                        } else {
                            ext.build_status = Some("building".to_string());
                            ext.build_error = None;
                        }
                    }
                }
                state.persist_extensions().await;
                if timed_out_no_run {
                    state.add_log("ERROR", &no_run_message).await;
                    return;
                }
            }
            Err(e) => {
                last_error = e;
                state
                    .add_log(
                        "WARN",
                        &format!("No se pudo consultar Actions ({}): {}", short_sha, last_error),
                    )
                    .await;
            }
        }

        if started.elapsed().as_secs() >= MAX_WAIT_SECS {
            // Nunca dejar «compilando» forever: si hay un run activo sigue en
            // curso; si ni siquiera pudimos consultar, marcamos pendiente con
            // un mensaje accionable y el enlace a Actions.
            let detail = if last_error.is_empty() {
                format!(
                    "El run de GitHub Actions para {} sigue activo tras {} s. AEXT no puede afirmar que haya terminado; pulsa «Verificar Actions» para consultar el resultado final.",
                    short_sha, MAX_WAIT_SECS
                )
            } else {
                format!(
                    "No se pudo confirmar el resultado de GitHub Actions para {} tras {} s ({}). Revisa el repo y pulsa «Verificar Actions».",
                    short_sha, MAX_WAIT_SECS, last_error
                )
            };
            {
                let mut list = state.extensions.write().await;
                if let Some(ext) = list.iter_mut().find(|e| e.pkg_id == pkg_id || e.id == pkg_id) {
                    if ext.actions_run_url.is_none() {
                        ext.actions_run_url = Some(actions_url.clone());
                    }
                    ext.build_status = Some("pending".to_string());
                    ext.build_error = Some(detail.clone());
                    ext.build_apk_path = None;
                }
            }
            state.persist_extensions().await;
            state.add_log("WARN", &detail).await;
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(POLL_SECS)).await;
    }
}

/// Vuelve a consultar el resultado real de GitHub Actions para el último commit
/// que AEXT subió. Útil cuando el run tardó más que la espera inicial.
#[tauri::command]
pub async fn refresh_github_actions(
    pkg_id: String,
    state: State<'_, AppState>,
) -> Result<ExtensionItem, String> {
    let ext = state
        .extensions
        .read()
        .await
        .iter()
        .find(|e| e.pkg_id == pkg_id || e.id == pkg_id)
        .cloned()
        .ok_or_else(|| format!("No se encontró la extensión {}", pkg_id))?;

    let sha = ext.actions_commit_sha.clone().ok_or_else(|| {
        "Esta extensión no tiene un commit de AEXT registrado: genera la extensión de nuevo para poder verificar Actions.".to_string()
    })?;
    let repo_url = ext
        .github_repo_url
        .clone()
        .ok_or_else(|| "Esta extensión no tiene un repositorio de GitHub asociado.".to_string())?;
    let (owner, repo) = parse_github_url(&repo_url)
        .ok_or_else(|| format!("URL de repositorio no válida: {}", repo_url))?;

    let auth = GithubAuthManager::new();
    let token = match auth.get_saved_token() {
        Some(t) => t,
        None => auth
            .extract_and_save_cli_token()
            .await
            .map_err(|_| {
                "Necesitas iniciar sesión en GitHub para consultar Actions.".to_string()
            })?
            .0,
    };
    let client = GithubApiClient::new(token);

    state
        .add_log(
            "INFO",
            &format!("Consultando de nuevo el resultado de Actions para {}…", ext.pkg_id),
        )
        .await;
    verify_github_actions(&state, &client, &owner, &repo, &sha, &ext.pkg_id).await;

    let updated = state
        .extensions
        .read()
        .await
        .iter()
        .find(|e| e.pkg_id == ext.pkg_id || e.id == ext.id)
        .cloned()
        .unwrap_or(ext);
    Ok(updated)
}

/// Extrae las líneas útiles de Gradle para la terminal (en vez de un genérico
/// "La compilación falló").
fn extract_build_error(output: &str) -> String {
    let mut picked: Vec<String> = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let lower = line.to_lowercase();
        let relevant = line.starts_with("e: ")
            || lower.contains("error:")
            || lower.contains("failure:")
            || lower.contains("what went wrong")
            || lower.contains("execution failed")
            || lower.contains("cannot mutate")
            || lower.contains("unresolved reference");
        if relevant && !picked.iter().any(|p| p == line) {
            picked.push(line.to_string());
        }
        if picked.len() >= 4 {
            break;
        }
    }
    if picked.is_empty() {
        "Gradle devolvió un error sin mensaje detallado. Revisa la salida completa abajo."
            .to_string()
    } else {
        picked.join("\n")
    }
}

/// Elimina APKs de compilaciones anteriores para no dar un éxito falso cuando
/// Gradle falla pero el APK viejo sigue en `build/outputs`.
fn remove_stale_apks(root: &Path) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            remove_stale_apks(&path);
        } else if path.extension() == Some(std::ffi::OsStr::new("apk")) {
            let _ = fs::remove_file(path);
        }
    }
}

/// Busca los `build.gradle.kts` de los módulos bajo `src/`.
fn find_module_build_files(root: &Path) -> Vec<std::path::PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.file_name().and_then(|n| n.to_str()) == Some("build.gradle.kts") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&root.join("src"), &mut out);
    out
}

/// Actualiza el toolchain de un proyecto generado por una versión anterior de
/// AEXT (AGP 8.2.2/Gradle 8.5) a la combinación actual (AGP 9/Gradle 9.5.1).
/// No toca el código Kotlin: el usuario puede haber personalizado los
/// extractores. Sin esto, una carpeta antigua sigue fallando aunque se instale
/// una versión nueva de AEXT.
fn upgrade_project_toolchain(dir: &Path) -> Result<(), String> {
    // 1. Wrapper: lo sobrescribe siempre, es propiedad de AEXT.
    let wrapper_dir = dir.join("gradle").join("wrapper");
    fs::create_dir_all(&wrapper_dir)
        .map_err(|e| format!("No se pudo crear {}: {}", wrapper_dir.display(), e))?;
    fs::write(
        wrapper_dir.join("gradle-wrapper.properties"),
        GRADLE_WRAPPER_PROPERTIES.as_bytes(),
    )
    .map_err(|e| format!("No se pudo actualizar gradle-wrapper.properties: {}", e))?;
    fs::write(
        wrapper_dir.join("gradle-wrapper.jar"),
        decode_base64(GRADLE_WRAPPER_JAR_B64),
    )
    .map_err(|e| format!("No se pudo actualizar gradle-wrapper.jar: {}", e))?;
    fs::write(dir.join("gradlew"), GRADLEW_SH.as_bytes())
        .map_err(|e| format!("No se pudo actualizar gradlew: {}", e))?;
    fs::write(dir.join("gradlew.bat"), GRADLEW_BAT.as_bytes())
        .map_err(|e| format!("No se pudo actualizar gradlew.bat: {}", e))?;

    // 2. build.gradle.kts raíz (no lleva código del usuario).
    fs::write(dir.join("build.gradle.kts"), ROOT_BUILD_GRADLE.as_bytes())
        .map_err(|e| format!("No se pudo actualizar build.gradle.kts: {}", e))?;

    // 3. build.gradle.kts del módulo: AGP 9 rechaza `kotlin("android")` y ya no
    //    expone `kotlinOptions`; hay que migrar al DSL `compilerOptions`.
    const NEW_KOTLIN: &str = "    kotlin {\n        compilerOptions {\n            jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_1_8\n            freeCompilerArgs.add(\"-opt-in=kotlinx.serialization.ExperimentalSerializationApi\")\n        }\n    }";
    for build_file in find_module_build_files(dir) {
        let text = fs::read_to_string(&build_file).unwrap_or_default();
        let mut out: String = text
            .lines()
            .filter(|line| !line.contains("kotlin(\"android\")"))
            .map(|line| format!("{}\n", line))
            .collect();
        if let Some(start) = out.find("kotlinOptions {") {
            let line_start = out[..start].rfind('\n').map(|p| p + 1).unwrap_or(0);
            let mut depth = 0i32;
            let mut end = None;
            for (i, ch) in out[start..].char_indices() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(start + i + 1);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(end) = end {
                out.replace_range(line_start..end, NEW_KOTLIN);
            }
        }
        // El lint de release de AGP 9 intenta resolver dependencias transitivas
        // muertas del extensions-lib antiguo (injekt-core de JitPack) y rompe
        // `assembleRelease`. Las extensiones no necesitan lint vital.
        if !out.contains("checkReleaseBuilds") {
            if let Some(pos) = out.find("compileOptions {") {
                let insert = "    lint {\n        checkReleaseBuilds = false\n    }\n\n";
                out.insert_str(pos, insert);
            }
        }
        fs::write(&build_file, out)
            .map_err(|e| format!("No se pudo actualizar {}: {}", build_file.display(), e))?;
    }
    Ok(())
}

/// Compila el módulo con Gradle + Android SDK si están instalados. Es una
/// compilación real (firma incluida); nunca simulamos un APK inexistente.
#[tauri::command]
pub async fn compile_extension(
    state: State<'_, AppState>,
    pkg_id: String,
) -> Result<serde_json::Value, String> {
    let app_state = state.inner().clone();
    let dir = local_extension_dir(&state, &pkg_id);
    if !dir.is_dir() {
        let msg = format!(
            "No hay código local para compilar. Descarga el ZIP o regenera la extensión ({}).",
            dir.display()
        );
        set_build_status(&app_state, &pkg_id, "error", Some(msg.clone()), None).await;
        return Err(msg);
    }
    if !dir.join("settings.gradle.kts").exists() {
        let msg = "El proyecto local no tiene settings.gradle.kts. Regenera la extensión con esta versión de AEXT.".to_string();
        set_build_status(&app_state, &pkg_id, "error", Some(msg.clone()), None).await;
        return Err(msg);
    }

    // Un proyecto creado con una versión anterior de AEXT sigue trayendo
    // AGP 8.2.2 + Gradle 8.5, que no arranca con las JDK 25/27 actuales.
    // Migrar los archivos de compilación sin tocar el código Kotlin.
    if let Err(e) = upgrade_project_toolchain(&dir) {
        set_build_status(&app_state, &pkg_id, "error", Some(e.clone()), None).await;
        return Err(e);
    }

    let toolchain = tauri::async_runtime::spawn_blocking(detect_toolchain)
        .await
        .map_err(|e| e.to_string())?;

    if !toolchain.java || !toolchain.android_sdk {
        let msg = format!(
            "Faltan herramientas para compilar: {}",
            toolchain.missing.join(", ")
        );
        set_build_status(&app_state, &pkg_id, "error", Some(msg.clone()), None).await;
        return Err(msg);
    }

    let parts: Vec<&str> = pkg_id.split('.').filter(|p| !p.is_empty()).collect();
    if parts.len() < 2 {
        let msg = "No se pudo deducir la ruta del módulo Gradle.".to_string();
        set_build_status(&app_state, &pkg_id, "error", Some(msg.clone()), None).await;
        return Err(msg);
    }
    let module = format!(
        ":src:{}:{}",
        parts[parts.len() - 2],
        parts[parts.len() - 1]
    );

    // Preferir SIEMPRE el wrapper del proyecto: fija Gradle 8.5. El Gradle del
    // PATH puede ser 9.x y romper AGP 8.2.2.
    let wrapper = if cfg!(windows) {
        dir.join("gradlew.bat")
    } else {
        dir.join("gradlew")
    };
    #[cfg(unix)]
    if wrapper.is_file() {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755));
    }
    let gradle = if wrapper.is_file() {
        wrapper.display().to_string()
    } else {
        let detected = toolchain.gradle_path.clone().ok_or_else(|| {
            "No se encontró Gradle ni un wrapper en el proyecto. Regenera la extensión."
                .to_string()
        })?;
        detected
    };
    let sdk = toolchain
        .android_sdk_path
        .clone()
        .ok_or_else(|| "No se encontró el Android SDK.".to_string())?;
    let java_home = toolchain.java_home.clone();

    set_build_status(&app_state, &pkg_id, "building", None, None).await;

    // local.properties apunta al SDK real del usuario.
    let local_properties = format!("sdk.dir={}\n", sdk.replace('\\', "/"));
    fs::write(dir.join("local.properties"), local_properties)
        .map_err(|e| format!("No se pudo escribir local.properties: {}", e))?;

    // Un APK anterior no debe hacer que un build fallido parezca exitoso.
    remove_stale_apks(&dir.join("src"));

    app_state
        .add_log(
            "INFO",
            &format!(
                "Compilando {}:assembleRelease (puede tardar varios minutos)…",
                module
            ),
        )
        .await;

    let build_dir = dir.clone();
    let build_module = module.clone();
    let build_sdk = sdk.clone();
    let build_java = java_home.clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut command = build_tool_command(&gradle);
        command
            .arg("--no-daemon")
            .arg("--console=plain")
            .arg(format!("{}:assembleRelease", build_module))
            .current_dir(&build_dir)
            .env("ANDROID_HOME", &build_sdk)
            .env("ANDROID_SDK_ROOT", &build_sdk);
        if let Some(home) = build_java {
            command.env("JAVA_HOME", home);
        }
        command.output()
    })
    .await
    .map_err(|e| format!("La compilación se interrumpió: {}", e))?;

    let output = result.map_err(|e| {
        format!(
            "No se pudo ejecutar Gradle ({}). Verifica que Java, Gradle y el Android SDK estén instalados.",
            e
        )
    })?;

    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push('\n');
    combined.push_str(&String::from_utf8_lossy(&output.stderr));

    let tail: Vec<&str> = combined.lines().rev().take(160).collect();
    let tail = tail.into_iter().rev().collect::<Vec<_>>().join("\n");
    let error_summary = if output.status.success() {
        None
    } else {
        Some(extract_build_error(&combined))
    };

    let apk = find_apk(&dir.join("src"));
    let success = output.status.success() && apk.is_some();
    let apk_path = apk.map(|p| p.display().to_string());

    if success {
        set_build_status(&app_state, &pkg_id, "success", None, apk_path.clone()).await;
        app_state
            .add_log(
                "SUCCESS",
                &format!("APK compilado en {}", apk_path.clone().unwrap_or_default()),
            )
            .await;
    } else {
        let detail = error_summary
            .clone()
            .unwrap_or_else(|| "Sin detalles de Gradle.".to_string());
        set_build_status(&app_state, &pkg_id, "error", Some(detail.clone()), None).await;
        app_state
            .add_log(
                "ERROR",
                &format!("La compilación de {} falló:\n{}", pkg_id, detail),
            )
            .await;
    }

    Ok(serde_json::json!({
        "success": success,
        "apk_path": apk_path,
        "exit_code": output.status.code(),
        "error": error_summary,
        "output": tail
    }))
}

fn build_tool_command(program: &str) -> std::process::Command {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let lower = program.to_ascii_lowercase();
        if lower.ends_with(".bat") || lower.ends_with(".cmd") {
            let mut command = std::process::Command::new("cmd");
            command.arg("/C").arg(program);
            command.creation_flags(CREATE_NO_WINDOW);
            return command;
        }
        let mut command = std::process::Command::new(program);
        command.creation_flags(CREATE_NO_WINDOW);
        command
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new(program)
    }
}

fn which_tool(program: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    let output = {
        use std::os::windows::process::CommandExt;
        let mut command = std::process::Command::new("where");
        command.arg(program).creation_flags(0x0800_0000);
        command.output().ok()?
    };
    #[cfg(not(target_os = "windows"))]
    let output = std::process::Command::new("which")
        .arg(program)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.trim())
        .find(|line| !line.is_empty())
        .map(|line| line.to_string())
}

fn run_version(program: &str, args: &[&str]) -> Option<String> {
    let output = build_tool_command(program).args(args).output().ok()?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if text.trim().is_empty() {
        text = stderr;
    }
    text.lines()
        .map(|line| line.trim())
        .find(|line| !line.is_empty())
        .map(|line| line.to_string())
}

fn find_file_under(root: &Path, file_name: &str, depth: usize) -> Option<std::path::PathBuf> {
    if depth == 0 || !root.is_dir() {
        return None;
    }
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .file_name()
                .map(|name| name.to_string_lossy().as_ref() == file_name)
                .unwrap_or(false)
        {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_file_under(&path, file_name, depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(std::path::PathBuf::from)
}

fn detect_toolchain() -> BuildToolchain {
    let mut missing = Vec::new();
    let mut notes = Vec::new();

    // Java (el JBR de Android Studio también sirve).
    let java_from_home = std::env::var_os("JAVA_HOME").map(std::path::PathBuf::from).and_then(|home| {
        let exe = if cfg!(windows) { "java.exe" } else { "java" };
        let candidate = home.join("bin").join(exe);
        if candidate.is_file() {
            Some((candidate.display().to_string(), home.display().to_string()))
        } else {
            None
        }
    });

    let studio_java = [
        "C:/Program Files/Android/Android Studio/jbr",
        "/Applications/Android Studio.app/Contents/jbr/Contents/Home",
        "/opt/android-studio/jbr",
    ]
    .iter()
    .find_map(|base| {
        let home = std::path::PathBuf::from(base);
        let exe = if cfg!(windows) { "java.exe" } else { "java" };
        let candidate = home.join("bin").join(exe);
        if candidate.is_file() {
            Some((candidate.display().to_string(), home.display().to_string()))
        } else {
            None
        }
    });

    let java_path = java_from_home
        .clone()
        .map(|(path, _)| path)
        .or_else(|| which_tool("java"))
        .or_else(|| studio_java.clone().map(|(path, _)| path));
    let java_home = java_from_home
        .map(|(_, home)| home)
        .or_else(|| studio_java.map(|(_, home)| home))
        .or_else(|| {
            java_path.as_ref().and_then(|path| {
                std::path::Path::new(path)
                    .parent()
                    .and_then(|bin| bin.parent())
                    .map(|home| home.display().to_string())
            })
        });

    let java_version = java_path.as_deref().and_then(|path| run_version(path, &["-version"]));
    let java = java_path.is_some();
    if !java {
        missing.push("Java (JDK 17)".to_string());
    }

    let kotlinc_path = which_tool("kotlinc").or_else(|| which_tool("kotlinc.bat"));
    let kotlinc_version = kotlinc_path
        .as_deref()
        .and_then(|path| run_version(path, &["-version"]));
    let kotlinc = kotlinc_path.is_some();

    // Gradle del PATH o una distribución descargada por el wrapper.
    let gradle_path = which_tool("gradle").or_else(|| {
        let home = home_dir()?;
        let wrapper = home.join(".gradle").join("wrapper").join("dists");
        find_file_under(
            &wrapper,
            if cfg!(windows) { "gradle.bat" } else { "gradle" },
            6,
        )
        .map(|path| path.display().to_string())
    });
    let gradle_version = gradle_path
        .as_deref()
        .and_then(|path| run_version(path, &["-version"]));
    let gradle = gradle_path.is_some();
    if !gradle {
        missing.push("Gradle".to_string());
    }

    // Android SDK.
    let android_sdk_path = std::env::var_os("ANDROID_HOME")
        .or_else(|| std::env::var_os("ANDROID_SDK_ROOT"))
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .map(|base| std::path::PathBuf::from(base).join("Android").join("Sdk"))
        })
        .or_else(|| home_dir().map(|home| home.join("Android").join("Sdk")))
        .or_else(|| Some(std::path::PathBuf::from(
            "/Users/Shared/Android/sdk",
        )))
        .filter(|path| path.is_dir());
    let android_sdk = android_sdk_path.is_some();
    if !android_sdk {
        missing.push("Android SDK".to_string());
    }

    if !kotlinc {
        notes.push(
            "Kotlin no es imprescindible: Gradle descarga el compilador si hace falta."
                .to_string(),
        );
    }
    if !android_sdk {
        notes.push(
            "Instala Android Studio (incluye SDK, JDK y Gradle) o exporta ANDROID_HOME."
                .to_string(),
        );
    }

    // Los proyectos generados incluyen el wrapper, así que no hace falta un
    // Gradle instalado en el sistema: el wrapper descarga la versión fijada.
    let can_build = java && android_sdk;
    if !gradle {
        notes.push(
            "No hay Gradle en el PATH, pero el proyecto incluye el wrapper: se descargará automáticamente."
                .to_string(),
        );
    }

    BuildToolchain {
        os: std::env::consts::OS.to_string(),
        java,
        java_path,
        java_version,
        java_home,
        kotlinc,
        kotlinc_version,
        gradle,
        gradle_path,
        gradle_version,
        android_sdk,
        android_sdk_path: android_sdk_path.map(|path| path.display().to_string()),
        can_build,
        missing,
        notes,
    }
}

fn find_apk(root: &Path) -> Option<std::path::PathBuf> {
    if !root.is_dir() {
        return None;
    }
    let entries = fs::read_dir(root).ok()?;
    let mut best: Option<std::path::PathBuf> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_apk(&path) {
                let newer = fs::metadata(&found)
                    .and_then(|m| m.modified())
                    .ok();
                let current = best.as_ref().and_then(|p| fs::metadata(p).and_then(|m| m.modified()).ok());
                if best.is_none() || newer > current {
                    best = Some(found);
                }
            }
        } else if path.extension().map(|ext| ext == "apk").unwrap_or(false) {
            best = Some(path);
        }
    }
    best
}

// ============================== HELPERS ==============================

pub fn local_extension_dir(state: &AppState, pkg_id: &str) -> std::path::PathBuf {
    state.storage_dir.join("extensions").join(pkg_id)
}

/// Borra un directorio reintentando un par de veces: en Windows los archivos
/// de `build/` pueden seguir abiertos justo después de Gradle.
fn remove_dir_with_retries(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let mut last = String::new();
    for attempt in 0..3 {
        match fs::remove_dir_all(path) {
            Ok(()) if !path.exists() => return Ok(()),
            Ok(()) => last = "el directorio sigue existiendo".to_string(),
            Err(e) => last = e.to_string(),
        }
        std::thread::sleep(std::time::Duration::from_millis(200 * (attempt + 1)));
    }
    Err(last)
}

/// Archivos que AEXT genera a nivel de repositorio. Se comprueba su
/// existencia remota uno por uno: si están, son del usuario; si faltan, se
/// suben para que el repo pueda compilar en Actions.
const REPO_SCAFFOLD_FILES: &[&str] = &[
    "README.md",
    "build.gradle.kts",
    "settings.gradle.kts",
    "gradle.properties",
    ".gitignore",
    "gradlew",
    "gradlew.bat",
    "gradle/wrapper/gradle-wrapper.properties",
    "gradle/wrapper/gradle-wrapper.jar",
    ".github/workflows/build_push.yml",
    ".github/scripts/generate-repo-index.py",
    "signingkey.jks",
    "icon/ic_launcher.png",
];

/// Decide, archivo por archivo, qué scaffold hay que subir.
///
/// `repo.empty` no es suficiente: GitHub crea los repos con `auto_init` y un
/// README, así que un repo recién creado parece no vacío aunque no tenga
/// workflow ni wrapper. Al revés, un repo antiguo del usuario puede no tener
/// README propio: en ese caso sí generamos uno mínimo y genérico.
async fn reconcile_repo_files(
    client: &GithubApiClient,
    repo: &RepoInfo,
    bundle: &mut crate::engine::ExtensionBundle,
) -> Result<(), String> {
    for path in REPO_SCAFFOLD_FILES {
        if !is_repo_scaffold(path) {
            return Err(format!("Ruta de scaffold no protegida: {}", path));
        }
        if !client
            .file_exists(&repo.owner, &repo.name, path, &repo.default_branch)
            .await?
        {
            continue; // Falta: se conserva la versión generada.
        }

        match *path {
            "settings.gradle.kts" => {
                // Es el único scaffold que se EDITA: añade el include del
                // módulo nuevo sin borrar los anteriores.
                merge_existing_settings(client, repo, bundle).await?;
            }
            "README.md" => {
                let existing = client
                    .get_file_content(
                        &repo.owner,
                        &repo.name,
                        path,
                        &repo.default_branch,
                    )
                    .await?;
                if !existing
                    .as_deref()
                    .map(|content| is_auto_init_readme(content, &repo.name))
                    .unwrap_or(false)
                {
                    bundle.files.remove(*path);
                }
            }
            _ => {
                // README aparte, el resto es propiedad del usuario: firma,
                // workflow, wrapper y build files nunca se sobrescriben.
                bundle.files.remove(*path);
            }
        }
    }
    Ok(())
}

/// Al reutilizar un repositorio con varias extensiones, añade el `include()`
/// del nuevo módulo sin borrar los que ya existían.
async fn merge_existing_settings(
    client: &GithubApiClient,
    repo: &RepoInfo,
    bundle: &mut crate::engine::ExtensionBundle,
) -> Result<(), String> {
    let existing = client
        .get_file_content(
            &repo.owner,
            &repo.name,
            "settings.gradle.kts",
            &repo.default_branch,
        )
        .await?
        .ok_or_else(|| {
            format!(
                "GitHub informó de settings.gradle.kts en {}/{} pero no devolvió su contenido",
                repo.owner, repo.name
            )
        })?;

    let include = format!("include(\":{}\")", bundle.module_path.replace('/', ":"));
    if existing.contains(&include) {
        bundle.files.remove("settings.gradle.kts");
        return Ok(());
    }

    let mut merged = existing.trim_end().to_string();
    merged.push('\n');
    merged.push_str(&include);
    merged.push('\n');
    bundle
        .files
        .insert("settings.gradle.kts".to_string(), merged.into_bytes());
    Ok(())
}

/// El README de `auto_init` de GitHub contiene solo el título con el nombre
/// del repo y la descripción que AEXT puso al crearlo. Se puede reemplazar
/// por el README mínimo; cualquier otro README se respeta.
fn is_auto_init_readme(content: &str, repo_name: &str) -> bool {
    let lines: Vec<String> = content
        .lines()
        .map(|line| line.trim().to_ascii_lowercase())
        .filter(|line| !line.is_empty())
        .collect();
    let Some(first) = lines.first() else {
        return false;
    };
    if first != &format!("# {}", repo_name.to_ascii_lowercase()) {
        return false;
    }
    lines.len() == 1 || lines.iter().skip(1).any(|line| line.contains("aext"))
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

/// Copia el icono también a `icon/` en la raíz del proyecto. Solo aplica al
/// modo GitHub: el índice de `gh-pages` y Nekoyomi esperan
/// `icon/ic_launcher.png` e `icon/<pkg_id>.png`. En modo local estos archivos
/// se omiten porque no hay repositorio que los publique.
fn write_repo_icons(
    target_dir: &Path,
    pkg_id: &str,
    mipmaps: &std::collections::HashMap<String, Vec<u8>>,
) -> Result<(), String> {
    let Some(icon_bytes) = mipmaps
        .get("mipmap-xxxhdpi")
        .or_else(|| mipmaps.get("mipmap-xxhdpi"))
    else {
        return Ok(());
    };

    let icon_dir = target_dir.join("icon");
    fs::create_dir_all(&icon_dir)
        .map_err(|e| format!("No se pudo crear {}: {}", icon_dir.display(), e))?;
    fs::write(icon_dir.join("ic_launcher.png"), icon_bytes)
        .map_err(|e| format!("No se pudo escribir el icono genérico: {}", e))?;
    fs::write(icon_dir.join(format!("{}.png", pkg_id)), icon_bytes)
        .map_err(|e| format!("No se pudo escribir el icono del paquete: {}", e))?;
    Ok(())
}

/// Archivos que pertenecen al repositorio, NO a una extensión concreta.
///
/// AEXT los crea cuando el repo está vacío, pero en un repo existente jamás
/// deben sobrescribirse: el README (con la URL, el disclaimer y las notas del
/// usuario), la clave de firma (cambiarla rompe las actualizaciones instaladas),
/// el workflow, el wrapper y los build files raíz.
fn is_repo_scaffold(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower == "readme.md"
        || lower == "readme"
        || lower == "build.gradle.kts"
        || lower == "build.gradle"
        || lower == "settings.gradle.kts"
        || lower == "settings.gradle"
        || lower == "gradle.properties"
        || lower == ".gitignore"
        || lower == "local.properties"
        || lower == "gradlew"
        || lower == "gradlew.bat"
        || lower.starts_with(".github/")
        || lower.starts_with("gradle/")
        || lower == "icon/ic_launcher.png"
        || lower.starts_with("signingkey")
        || lower.ends_with(".jks")
        || lower.ends_with(".keystore")
}

#[cfg(test)]
mod scaffold_tests {
    use super::{is_auto_init_readme, is_repo_scaffold, REPO_SCAFFOLD_FILES};

    #[test]
    fn scaffold_list_covers_every_protected_root_file() {
        for path in REPO_SCAFFOLD_FILES {
            assert!(
                is_repo_scaffold(path),
                "el scaffold debe proteger archivos del repo: {}",
                path
            );
        }
    }

    #[test]
    fn detects_auto_init_readme_only() {
        let placeholder = "# nekoyomi-ext-pluto\nRepositorio de extensiones Nekoyomi generado automáticamente por AEXT\n\n";
        assert!(is_auto_init_readme(placeholder, "nekoyomi-ext-pluto"));
        assert!(is_auto_init_readme("# MiRepo\n", "mirepo"));
        assert!(!is_auto_init_readme(
            "# MiRepo\nDocumentación del proyecto\n",
            "mirepo"
        ));
        assert!(!is_auto_init_readme(
            "# Otro repo\nGenerado por AEXT\n",
            "mirepo"
        ));
    }

    #[test]
    fn protects_user_owned_repo_files() {
        for path in [
            "README.md",
            "readme",
            "signingkey.jks",
            "build.gradle.kts",
            "settings.gradle.kts",
            "gradle.properties",
            "gradlew",
            "gradlew.bat",
            "gradle/wrapper/gradle-wrapper.properties",
            "gradle/wrapper/gradle-wrapper.jar",
            ".github/workflows/build_push.yml",
            ".github/scripts/generate-repo-index.py",
            ".gitignore",
            "icon/ic_launcher.png",
        ] {
            assert!(is_repo_scaffold(path), "debe protegerse: {}", path);
        }
    }

    #[test]
    fn allows_extension_owned_files() {
        for path in [
            "src/es/miextension/build.gradle.kts",
            "src/es/miextension/src/main/AndroidManifest.xml",
            "src/es/miextension/src/main/java/eu/kanade/MiExt.kt",
            "src/es/miextension/src/main/res/mipmap-hdpi/ic_launcher.png",
            "icon/eu.kanade.tachiyomi.animeextension.es.miextension.png",
        ] {
            assert!(!is_repo_scaffold(path), "debe poder subirse: {}", path);
        }
    }
}

/// Prepara el bundle para una carpeta local: solo lo necesario para compilar
/// el APK. El workflow de Actions, el script del índice y los iconos de
/// `icon/` solo tienen sentido en un repositorio con `gh-pages`.
fn prepare_bundle_for_local(files: &mut std::collections::HashMap<String, Vec<u8>>) {
    files.retain(|path, _| {
        !(path.starts_with(".github/")
            || path.starts_with("icon/")
            || path == "local.properties")
    });
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

    /// Regresión: los commits que AEXT escribe en el repo destino nunca deben
    /// llevar el package ni el nombre de la extensión.
    #[test]
    fn commit_messages_never_include_extension_identity() {
        for msg in [COMMIT_MSG_GENERATE, COMMIT_MSG_UPDATE] {
            let lower = msg.to_lowercase();
            assert!(
                !lower.contains('.'),
                "el mensaje parece incluir un package: {}",
                msg
            );
            assert!(
                !lower.contains("pluto"),
                "el mensaje incluye un nombre de extensión: {}",
                msg
            );
            assert!(
                lower.starts_with("aext:"),
                "se esperaba un mensaje genérico de AEXT: {}",
                msg
            );
        }
    }
}
