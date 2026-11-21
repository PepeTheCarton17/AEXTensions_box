use crate::commands::parsers::parse_github_url;
use crate::github::{GithubApiClient, GithubAuthManager};
use crate::models::RepoCheckResult;
use crate::state::AppState;
use std::collections::HashMap;
use tauri::State;

/// Deriva la ruta `src/<lang>/<slug>` del package id. Es la misma convención
/// que usa el generador y el borrado remoto; validar los segmentos evita
/// construir rutas arbitrarias a partir de datos del catálogo.
pub(crate) fn extension_module_path(pkg_id: &str) -> Option<String> {
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
