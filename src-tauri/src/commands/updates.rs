//! Comandos Tauri de actualización. Todos comparten el mismo pipeline y un
//! bloqueo global: nunca dos operaciones de actualización a la vez.

use crate::commands::catalog::sync_published_variants;
use crate::commands::update_prepare::prepare_update;
use crate::commands::update_publish::publish_updates;
use crate::commands::update_types::update_lock;
use crate::models::ExtensionItem;
use crate::state::AppState;
use tauri::State;

fn is_updatable(ext: &ExtensionItem, repo_url: Option<&str>) -> bool {
    if ext.trashed || (ext.repo_state.as_deref() == Some("missing") && !ext.repo_ignore_missing) {
        return false;
    }
    match repo_url {
        None => true,
        Some(selected) => ext
            .github_repo_url
            .as_deref()
            .map(|url| url.trim_end_matches('/') == selected.trim_end_matches('/'))
            .unwrap_or(false),
    }
}

fn skip_reason(ext: &ExtensionItem) -> &'static str {
    if ext.trashed {
        "en «Eliminadas»"
    } else {
        "módulo ausente en el repositorio"
    }
}

/// Prepara todos los destinos y publica un solo commit por repositorio.
async fn run_batch(
    pkg_ids: Vec<String>,
    lang: Option<String>,
    state: &AppState,
) -> Result<Vec<ExtensionItem>, String> {
    let mut unique = Vec::new();
    for id in pkg_ids {
        if !unique.contains(&id) {
            unique.push(id);
        }
    }
    if unique.is_empty() {
        return Err("No hay extensiones que actualizar".to_string());
    }

    state
        .add_log(
            "INFO",
            &format!("Preparando {} extensión(es) para actualizar", unique.len()),
        )
        .await;

    let mut prepared = Vec::new();
    let mut failures = Vec::new();
    for id in unique {
        match prepare_update(&id, lang.as_deref(), state).await {
            Ok(item) => prepared.push(item),
            Err(err) => {
                state
                    .add_log("ERROR", &format!("No se pudo actualizar {}: {}", id, err))
                    .await;
                failures.push(format!("{}: {}", id, err));
            }
        }
    }
    if prepared.is_empty() {
        return Err(failures.join("; "));
    }

    let results = publish_updates(prepared, state).await;
    if !failures.is_empty() {
        state
            .add_log(
                "WARN",
                &format!(
                    "{} extensión(es) omitidas del lote: {}",
                    failures.len(),
                    failures.join("; ")
                ),
            )
            .await;
    }
    Ok(results)
}

#[tauri::command]
pub async fn update_extension(
    pkg_id: String,
    lang: Option<String>,
    state: State<'_, AppState>,
) -> Result<ExtensionItem, String> {
    let _guard = update_lock().lock().await;
    sync_published_variants(&state).await;
    let mut results = run_batch(vec![pkg_id.clone()], lang, &state).await?;
    results
        .pop()
        .ok_or_else(|| format!("No se pudo confirmar la actualización de {}", pkg_id))
}

/// Actualiza todas las extensiones visibles o, si se indica un repo, todas las
/// que apuntan a ese mismo repositorio. Un solo commit por repo, sin runs
/// intermedios que se cancelen entre sí.
#[tauri::command]
pub async fn update_all_extensions(
    repo_url: Option<String>,
    lang: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ExtensionItem>, String> {
    let _guard = update_lock().lock().await;
    sync_published_variants(&state).await;

    let (targets, skipped) = {
        let list = state.extensions.read().await;
        let selected = repo_url.as_deref().map(|url| url.trim_end_matches('/'));
        let targets: Vec<String> = list
            .iter()
            .filter(|ext| is_updatable(ext, selected))
            .map(|ext| ext.pkg_id.clone())
            .collect();
        let skipped: Vec<String> = list
            .iter()
            .filter(|ext| !is_updatable(ext, selected))
            .map(|ext| format!("{}: {}", ext.pkg_id, skip_reason(ext)))
            .collect();
        (targets, skipped)
    };

    if targets.is_empty() {
        return Err("No hay extensiones que coincidan con ese repositorio".to_string());
    }
    for reason in &skipped {
        state.add_log("WARN", &format!("Omitida {}", reason)).await;
    }
    run_batch(targets, lang, &state).await
}

/// Lote explícito de paquetes: lo usa la ventana de agrupación de la UI.
#[tauri::command]
pub async fn update_extensions_batch(
    pkg_ids: Vec<String>,
    lang: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ExtensionItem>, String> {
    let _guard = update_lock().lock().await;
    sync_published_variants(&state).await;
    run_batch(pkg_ids, lang, &state).await
}
