use crate::commands::catalog_check::extension_module_path;
use crate::commands::constants::COMMIT_MSG_DELETE;
use crate::commands::files::{local_extension_dir, remove_dir_with_retries};
use crate::commands::parsers::parse_github_url;
use crate::github::{GithubApiClient, GithubAuthManager};
use crate::state::AppState;
use tauri::State;

async fn remove_remote_module(ext: &crate::models::ExtensionItem, state: &AppState) -> Result<(), String> {
    let Some(repo_url) = ext.github_repo_url.as_deref() else {
        return Ok(());
    };
    let (owner, name) = parse_github_url(repo_url)
        .ok_or_else(|| format!("No se pudo interpretar la URL de GitHub: {}", repo_url))?;
    let module_path = extension_module_path(&ext.pkg_id)
        .ok_or_else(|| "La ruta del módulo calculada no es segura; no se borró nada".to_string())?;

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
    let repo = client.get_repository(&owner, &name).await?;
    client
        .delete_module(
            &repo.owner,
            &repo.name,
            &module_path,
            COMMIT_MSG_DELETE,
            &repo.default_branch,
        )
        .await
        .map_err(|e| format!("No se pudo borrar el módulo de GitHub: {}", e))?;
    Ok(())
}

/// Eliminación definitiva desde «Eliminadas»: borra el módulo remoto (si
/// existe), la carpeta local y la ficha del catálogo. No se puede deshacer.
#[tauri::command]
pub async fn delete_extension_permanently(
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

    remove_remote_module(&ext, &state).await?;

    let dir = local_extension_dir(&state, &ext.pkg_id);
    if dir.is_dir() {
        let _ = remove_dir_with_retries(&dir.join("build"));
        let _ = remove_dir_with_retries(&dir.join(".gradle"));
        remove_dir_with_retries(&dir).map_err(|e| {
            format!(
                "No se pudo borrar la carpeta de {} ({}). Ciérrala en el explorador y vuelve a intentarlo.",
                ext.pkg_id, e
            )
        })?;
    }

    {
        let mut list = state.extensions.write().await;
        list.retain(|e| e.pkg_id != ext.pkg_id && e.id != ext.pkg_id);
    }
    state.persist_extensions().await;
    state
        .add_log(
            "SUCCESS",
            &format!("{} eliminada definitivamente (local y repositorio)", ext.pkg_id),
        )
        .await;
    Ok(true)
}
