use crate::state::AppState;
use chrono::Utc;
use tauri::State;

/// Mueve una ficha a «Eliminadas». No borra archivos ni toca GitHub: la
/// eliminación definitiva ocurre solo desde la sección «Eliminadas».
async fn move_to_trash(pkg_id: &str, state: &AppState) -> Result<bool, String> {
    let mut moved = false;
    {
        let mut list = state.extensions.write().await;
        if let Some(ext) = list.iter_mut().find(|e| e.pkg_id == pkg_id || e.id == pkg_id) {
            ext.trashed = true;
            ext.repo_deleted_at = Some(Utc::now().to_rfc3339());
            moved = true;
        }
    }
    if moved {
        state.persist_extensions().await;
        state
            .add_log("INFO", &format!("{} movida a «Eliminadas»", pkg_id))
            .await;
    }
    Ok(moved)
}

/// Icono de basura del catálogo: envía la extensión a «Eliminadas».
#[tauri::command]
pub async fn delete_extension(pkg_id: String, state: State<'_, AppState>) -> Result<bool, String> {
    move_to_trash(&pkg_id, &state).await
}

/// «Borrar de GitHub» también envía la ficha a «Eliminadas». El módulo remoto
/// permanece intacto hasta que el usuario confirme la eliminación definitiva.
#[tauri::command]
pub async fn delete_extension_github(
    pkg_id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    move_to_trash(&pkg_id, &state).await
}
