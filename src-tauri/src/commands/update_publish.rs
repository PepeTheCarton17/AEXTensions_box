//! Fase de publicación: agrupa las actualizaciones preparadas por repositorio
//! y hace un único commit (y por tanto un único run de Actions) por grupo.

use crate::commands::actions::verify_github_actions;
use crate::commands::constants::COMMIT_MSG_UPDATE;
use crate::commands::repo::{reconcile_batch_scaffold, settings_for_modules};
use crate::commands::update_types::PreparedUpdate;
use crate::models::ExtensionItem;
use crate::state::AppState;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Pausa breve entre repos para no encadenar pushes contra la API de GitHub.
const PAUSE_BETWEEN_REPOS_SECS: u64 = 3;

/// Publica todas las actualizaciones preparadas. Nunca falla en bloque: un
/// error de un repo se refleja en las fichas de ese repo y se registro en logs.
pub(crate) async fn publish_updates(
    prepared: Vec<PreparedUpdate>,
    state: &AppState,
) -> Vec<ExtensionItem> {
    let started = Instant::now();
    let mut groups: HashMap<String, Vec<PreparedUpdate>> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for update in prepared {
        let key = update
            .repo
            .as_ref()
            .map(|target| target.key())
            .unwrap_or_else(|| "local".to_string());
        if !groups.contains_key(&key) {
            order.push(key.clone());
        }
        groups.entry(key).or_default().push(update);
    }

    let group_count = order.len();
    let mut results = Vec::new();
    for (index, key) in order.into_iter().enumerate() {
        let group = groups.remove(&key).unwrap_or_default();
        results.extend(publish_group(group, state).await);
        if index + 1 < group_count {
            tokio::time::sleep(Duration::from_secs(PAUSE_BETWEEN_REPOS_SECS)).await;
        }
    }

    state
        .add_log(
            "INFO",
            &format!(
                "Lote publicado: {} extensión(es), {} grupo(s), {} s",
                results.len(),
                group_count,
                started.elapsed().as_secs()
            ),
        )
        .await;
    results
}

async fn publish_group(mut items: Vec<PreparedUpdate>, state: &AppState) -> Vec<ExtensionItem> {
    if items.is_empty() {
        return Vec::new();
    }
    if items.first().and_then(|item| item.repo.as_ref()).is_none() {
        return publish_local(&items, state).await;
    }

    let mut target = items[0].repo.take().expect("repo del grupo");
    for item in items.iter_mut().skip(1) {
        item.repo = None;
    }
    let repo = target.repo.clone();
    let client = &target.client;
    let modules: Vec<String> = items.iter().map(|item| item.module_path.clone()).collect();
    let pkg_ids: Vec<String> = items.iter().map(|item| item.pkg_id.clone()).collect();
    let warnings: usize = items.iter().map(|item| item.warnings.len()).sum();
    let prep_ms: u128 = items.iter().map(|item| item.elapsed_ms).sum();

    // Un solo mapa de archivos para todo el repo: así un mismo scaffold no se
    // sube una vez por extensión y no hay daños por deduplicación de rutas.
    let mut files: HashMap<String, Vec<u8>> = HashMap::new();
    let mut generated_settings = None;
    for item in &items {
        for (path, bytes) in &item.files {
            if path == "settings.gradle.kts" {
                if generated_settings.is_none() {
                    generated_settings = Some(bytes.clone());
                }
                continue;
            }
            files.insert(path.clone(), bytes.clone());
        }
    }

    let mut error = None;
    if let Err(err) = reconcile_batch_scaffold(client, &repo, &mut files).await {
        error = Some(err);
    }
    if error.is_none() {
        match settings_for_modules(client, &repo, &modules, generated_settings).await {
            Ok(Some(bytes)) => {
                files.insert("settings.gradle.kts".to_string(), bytes);
            }
            Ok(None) => {}
            Err(err) => error = Some(err),
        }
    }
    if let Some(err) = error {
        return mark_error(items, state, format!("No se pudo preparar el repo: {}", err)).await;
    }

    let file_count = files.len();
    let all_files: Vec<(String, Vec<u8>)> = files.into_iter().collect();
    let commit_sha = match client
        .commit_files(
            &repo.owner,
            &repo.name,
            &all_files,
            COMMIT_MSG_UPDATE,
            &repo.default_branch,
        )
        .await
    {
        Ok(sha) => sha,
        Err(err) => {
            return mark_error(items, state, format!("No se pudo actualizar GitHub: {}", err))
                .await;
        }
    };

    for item in &mut items {
        item.item.actions_commit_sha = Some(commit_sha.clone());
        item.item.actions_run_url = Some(format!(
            "https://github.com/{}/{}/actions",
            repo.owner, repo.name
        ));
    }
    persist_group(&items, state).await;

    let short_sha = &commit_sha[..commit_sha.len().min(7)];
    state
        .add_log(
            "INFO",
            &format!(
                "Lote subido en un commit ({}) a {}/{}: {} módulo(s), {} archivo(s), fuentes omitidas {}, preparación {} ms",
                short_sha,
                repo.owner,
                repo.name,
                modules.len(),
                file_count,
                warnings,
                prep_ms
            ),
        )
        .await;

    // Un solo sondeo para todo el grupo: el run compila todos los módulos.
    verify_github_actions(state, client, &repo.owner, &repo.name, &commit_sha, &pkg_ids).await;
    let final_items = collect_results(&pkg_ids, state).await;
    for item in &final_items {
        let level = if item.build_status.as_deref() == Some("error") {
            "ERROR"
        } else {
            "INFO"
        };
        state
            .add_log(
                level,
                &format!(
                    "{} v{}: {} — fuentes omitidas: {}",
                    item.pkg_id,
                    item.version,
                    item.build_status.as_deref().unwrap_or("pendiente"),
                    warnings
                ),
            )
            .await;
    }
    final_items
}

async fn publish_local(items: &[PreparedUpdate], state: &AppState) -> Vec<ExtensionItem> {
    persist_group(items, state).await;
    state
        .add_log(
            "INFO",
            &format!(
                "Actualización local completada: {} extensión(es)",
                items.len()
            ),
        )
        .await;
    items.iter().map(|item| item.item.clone()).collect()
}

async fn mark_error(
    mut items: Vec<PreparedUpdate>,
    state: &AppState,
    error: String,
) -> Vec<ExtensionItem> {
    state.add_log("ERROR", &format!("Publicación fallida: {}", error)).await;
    for item in &mut items {
        item.item.build_status = Some("error".to_string());
        item.item.build_error = Some(error.clone());
    }
    persist_group(&items, state).await;
    items.into_iter().map(|item| item.item).collect()
}

async fn persist_group(items: &[PreparedUpdate], state: &AppState) {
    let ids: Vec<String> = items
        .iter()
        .flat_map(|item| [item.pkg_id.clone(), item.previous_pkg_id.clone()])
        .collect();
    {
        let mut list = state.extensions.write().await;
        list.retain(|ext| !ids.contains(&ext.pkg_id) && !ids.contains(&ext.id));
        for item in items {
            list.push(item.item.clone());
        }
    }
    state.persist_extensions().await;
}

async fn collect_results(pkg_ids: &[String], state: &AppState) -> Vec<ExtensionItem> {
    let list = state.extensions.read().await;
    pkg_ids
        .iter()
        .filter_map(|id| {
            list.iter()
                .find(|ext| ext.pkg_id == id.as_str() || ext.id == id.as_str())
                .cloned()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::COMMIT_MSG_UPDATE;

    #[test]
    fn batch_commit_message_stays_generic() {
        assert!(COMMIT_MSG_UPDATE.to_ascii_lowercase().starts_with("aext:"));
        assert!(!COMMIT_MSG_UPDATE.contains('.'));
    }
}
