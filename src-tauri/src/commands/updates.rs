use crate::commands::catalog::sync_published_variants;
use crate::commands::update_one::update_extension_inner;
use crate::models::ExtensionItem;
use crate::state::AppState;
use tauri::State;

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
