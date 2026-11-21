use crate::commands::catalog_check::extension_module_path;
use crate::commands::constants::COMMIT_MSG_DELETE;
use crate::commands::files::{local_extension_dir, remove_dir_with_retries};
use crate::commands::parsers::parse_github_url;
use crate::github::{GithubApiClient, GithubAuthManager};
use crate::state::AppState;
use tauri::State;

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
