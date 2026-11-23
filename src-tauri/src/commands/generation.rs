use crate::commands::actions::verify_github_actions;
use crate::commands::constants::LOCAL_API_BASE;
use crate::commands::files::{local_extension_dir, prepare_bundle_for_local, write_bundle_to_dir, write_icons_to_dir};
use crate::commands::repo_upload::{upload_bundle_to_github, GithubUploadInfo};
use crate::commands::site_duplicates::{duplicate_message, find_existing_for_site};
use crate::engine::registry::collect_assets;
use crate::engine::{ExtensionGenerator, IconEngine, SiteAnalyzer};
use crate::github::GithubAuthManager;
use crate::models::{ExtensionItem, GenerationMode, GenerationRequest};
use crate::state::AppState;
use std::collections::HashMap;
use std::fs;
use tauri::State;

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
        let mut analysis = analyzer
            .analyze(url)
            .await
            .map_err(|e| format!("Error analizando {}: {}", url, e))?;
        // La generación re-analiza por su cuenta: repetir aquí las sondas
        // evita perder el catálogo verificado por los motores al analizar.
        crate::commands::engines::run_engine_trials(&state, &mut analysis).await;

        // Avisos honestos del análisis: el usuario debe saber qué no se pudo
        // detectar antes de publicar o compilar.
        for warning in &analysis.warnings {
            state
                .add_log("WARN", &format!("{}: {}", analysis.name, warning))
                .await;
        }

        // Un sitio sólo puede tener una ficha. El idioma elegido en la UI no
        // cambia la identidad del paquete, así que generar de nuevo el mismo
        // sitio sólo debe actualizar la ficha existente, nunca crear variantes.
        let duplicate = {
            let list = state.extensions.read().await;
            find_existing_for_site(&list[..], &analysis.url, request.ext_type.as_deref()).cloned()
        };
        if let Some(existing) = duplicate {
            let message = duplicate_message(&existing);
            state.add_log("WARN", &message).await;
            return Err(message);
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
        let engine_assets = {
            let engines = state.engines.read().await;
            collect_assets(&state.storage_dir, &engines[..])
        };
        if !engine_assets.is_empty() {
            state
                .add_log("INFO", &format!("Motores de runtime incluidos: {}", engine_assets.len()))
                .await;
        }
        let mut bundle = ExtensionGenerator::generate_with_engines(
            &analysis,
            request.custom_name.as_deref(),
            request.lang.as_deref(),
            request.ext_type.as_deref(),
            &engine_assets,
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
            trashed: false,
        };

        // Datos del commit subido a GitHub, para verificar el run de Actions
        // después de que el modo GitHub haya terminado de subir el código.
        let mut upload: Option<GithubUploadInfo> = None;

        match request.mode {
            GenerationMode::LocalOnly => {
                write_local_bundle(&state, &mut bundle, &mipmaps, &mut ext_item).await?;
            }
            GenerationMode::GithubRepo => {
                let info = upload_bundle_to_github(
                    &state, &auth, &request, &mut bundle, &mipmaps, &mut ext_item,
                )
                .await?;
                upload = Some(info);
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
        if let Some(info) = upload.take() {
            let pkg_ids = [ext_item.pkg_id.clone()];
            verify_github_actions(
                &state,
                &info.client,
                &info.owner,
                &info.repo,
                &info.sha,
                &pkg_ids,
            )
            .await;
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

/// Escribe en disco la copia local generada en modo `LocalOnly`.
async fn write_local_bundle(
    state: &AppState,
    bundle: &mut crate::engine::ExtensionBundle,
    mipmaps: &HashMap<String, Vec<u8>>,
    ext_item: &mut ExtensionItem,
) -> Result<(), String> {
                let target_dir = local_extension_dir(state, &bundle.pkg_id);
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
    Ok(())
}
