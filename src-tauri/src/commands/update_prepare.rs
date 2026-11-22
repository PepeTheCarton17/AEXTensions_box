//! Fase de preparación de una actualización: re-analiza, regenera, escribe la
//! copia local y resuelve el repo. No sube nada ni cambia el estado publicado.

use crate::commands::constants::LOCAL_API_BASE;
use crate::commands::files::{
    local_extension_dir, prepare_bundle_for_local, write_bundle_to_dir, write_icons_to_dir,
    write_repo_icons,
};
use crate::commands::parsers::parse_github_url;
use crate::commands::update_types::{PreparedUpdate, RepoTarget};
use crate::engine::generator::generate_repo_readme;
use crate::engine::{ExtensionGenerator, IconEngine, SiteAnalyzer};
use crate::github::{GithubApiClient, GithubAuthManager, RepoInfo};
use crate::models::ExtensionItem;
use crate::state::AppState;
use std::collections::HashMap;
use std::fs;
use std::time::Instant;

/// Idioma de los textos visibles: el elegido en AEXT prevalece; si no se envió
/// (o vino vacío), se mantienen los textos del idioma de la extensión.
pub(crate) fn resolve_update_lang<'a>(requested: Option<&'a str>, existing: &'a str) -> &'a str {
    requested
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(existing)
}

/// Regenera una extensión existente y deja todo listo para publicarla.
pub(crate) async fn prepare_update(
    pkg_id: &str,
    requested_lang: Option<&str>,
    state: &AppState,
) -> Result<PreparedUpdate, String> {
    let started = Instant::now();
    let existing = {
        let list = state.extensions.read().await;
        list.iter()
            .find(|e| e.pkg_id == pkg_id || e.id == pkg_id)
            .cloned()
            .ok_or_else(|| format!("No se encontró la extensión {}", pkg_id))?
    };

    if existing.trashed
        || (existing.repo_state.as_deref() == Some("missing") && !existing.repo_ignore_missing)
    {
        return Err(
            "Esta extensión está en «Eliminadas». Restaura su ficha antes de actualizarla.".to_string(),
        );
    }

    state
        .add_log(
            "INFO",
            &format!("Actualizando extensión {} desde {}", existing.name, existing.base_url),
        )
        .await;

    let analyzer = SiteAnalyzer::new();
    let mut analysis = analyzer
        .analyze(&existing.base_url)
        .await
        .map_err(|e| format!("No se pudo re-analizar {}: {}", existing.base_url, e))?;
    // Re-analizar también repite las sondas anónimas: el catálogo verificado
    // por los motores debe sobrevivir a la actualización de la extensión.
    crate::commands::engines::run_engine_trials(state, &mut analysis).await;

    // Los avisos del análisis son fuentes que NO se pudieron verificar. Se
    // guardan en el bundle para que el workflow los incluya en su reporte.
    let warnings = analysis.warnings.clone();
    for warning in &warnings {
        state
            .add_log("WARN", &format!("Fuentes omitidas en {}: {}", existing.pkg_id, warning))
            .await;
    }

    let next_code = existing.version_code + 1;
    let next_version = format!("16.{}", next_code);
    let (mipmaps, icon_base64) = IconEngine::fetch_and_generate_icons(
        analysis.favicon_url.as_deref(),
        &existing.name,
    )
    .await
    .unwrap_or_else(|_| (HashMap::new(), String::new()));

    // El idioma elegido sólo traduce textos. La identidad del paquete se
    // mantiene: una actualización nunca debe crear otro módulo con otro lang.
    let labels_lang = resolve_update_lang(requested_lang, existing.lang.as_str());
    let bundle = ExtensionGenerator::generate_with_version(
        &analysis,
        Some(existing.name.as_str()),
        Some(labels_lang),
        Some(existing.ext_type.as_str()),
        next_code,
        &next_version,
        Some(existing.lang.as_str()),
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
        updated.icon_base64 = Some(icon_base64);
    }

    let mut bundle_files = bundle.files.clone();
    let target_dir = local_extension_dir(state, &bundle.pkg_id);
    let _ = fs::remove_dir_all(&target_dir);
    let has_repo = existing.github_repo_url.is_some();
    if !has_repo {
        prepare_bundle_for_local(&mut bundle_files);
    }
    write_bundle_to_dir(&target_dir, &bundle_files)?;
    write_icons_to_dir(&target_dir, &bundle.module_path, &mipmaps)?;
    if has_repo {
        write_repo_icons(&target_dir, &bundle.pkg_id, &mipmaps)?;
    }
    if existing.pkg_id != bundle.pkg_id {
        let _ = fs::remove_dir_all(local_extension_dir(state, &existing.pkg_id));
    }
    if updated.github_repo_url.is_none() {
        updated.apk_download_url = Some(format!(
            "{}/api/download/{}",
            LOCAL_API_BASE, bundle.pkg_id
        ));
    }

    let mut repo_target = None;
    if let Some(repo_url) = existing.github_repo_url.as_deref() {
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
        let (owner, repo_name) = parse_github_url(repo_url)
            .ok_or_else(|| format!("URL de repositorio no válida: {}", repo_url))?;
        let repo: RepoInfo = client
            .ensure_repository(&owner, &repo_name, false, &gh_user.username)
            .await?;
        bundle_files.insert(
            "README.md".to_string(),
            generate_repo_readme(&repo.owner, &repo.name, &repo.html_url).into_bytes(),
        );

        let report = serde_json::json!({
            "pkg_id": bundle.pkg_id,
            "version": bundle.version,
            "version_code": bundle.version_code,
            "lang": bundle.lang,
            "warnings": warnings,
        });
        if let Ok(bytes) = serde_json::to_vec_pretty(&report) {
            bundle_files.insert(format!("{}/.aext-report.json", bundle.module_path), bytes);
        }
        repo_target = Some(RepoTarget { client, repo });
    }

    for (res_dir, icon_bytes) in &mipmaps {
        bundle_files.insert(
            format!("{}/src/main/res/{}/ic_launcher.png", bundle.module_path, res_dir),
            icon_bytes.clone(),
        );
        if res_dir == "mipmap-xxxhdpi" || res_dir == "mipmap-xxhdpi" {
            bundle_files.insert(format!("icon/{}.png", bundle.pkg_id), icon_bytes.clone());
            bundle_files.insert("icon/ic_launcher.png".to_string(), icon_bytes.clone());
        }
    }
    if let Some(target) = &repo_target {
        updated.github_repo_url = Some(target.repo.html_url.clone());
        updated.nekoyomi_repo_url = Some(format!(
            "https://raw.githubusercontent.com/{}/gh-pages/index.min.json",
            target.repo.full_name
        ));
        updated.apk_download_url = None;
    }

    let elapsed_ms = started.elapsed().as_millis();
    state
        .add_log(
            "INFO",
            &format!(
                "Preparada {} v{} en {} ms (fuentes omitidas: {})",
                updated.pkg_id,
                updated.version,
                elapsed_ms,
                warnings.len()
            ),
        )
        .await;

    Ok(PreparedUpdate {
        item: updated,
        pkg_id: bundle.pkg_id,
        previous_pkg_id: existing.pkg_id,
        module_path: bundle.module_path,
        files: bundle_files,
        repo: repo_target,
        warnings,
        elapsed_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::resolve_update_lang;

    #[test]
    fn chosen_language_only_affects_labels_on_update() {
        assert_eq!(resolve_update_lang(Some("es"), "all"), "es");
        assert_eq!(resolve_update_lang(Some("  pt-br "), "es"), "pt-br");
        assert_eq!(resolve_update_lang(Some("   "), "es"), "es");
        assert_eq!(resolve_update_lang(None, "en"), "en");
    }
}
