use crate::commands::parsers::parse_github_url;
use crate::models::ExtensionItem;
use crate::state::AppState;
use std::collections::HashSet;
use tauri::State;

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

/// El índice publicado (`gh-pages/index.min.json`) es la fuente de verdad de
/// las variantes que GitHub ya compila. Si AEXT no las adopta, `update_extension`
/// no puede saber que existen y su versión se queda congelada para siempre.
/// Se llama antes de cada actualización para que la lista local se reconcilie
/// sola, sin intervención manual.
pub(crate) async fn sync_published_variants(state: &AppState) -> usize {
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
                if existing.trashed {
                    continue; // La papelera la controla el usuario, no el índice.
                }
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
                trashed: false,
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
