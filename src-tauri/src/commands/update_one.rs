use crate::commands::actions::verify_github_actions;
use crate::commands::constants::{COMMIT_MSG_UPDATE, LOCAL_API_BASE};
use crate::commands::files::{
    local_extension_dir, prepare_bundle_for_local, write_bundle_to_dir, write_icons_to_dir,
    write_repo_icons,
};
use crate::commands::parsers::parse_github_url;
use crate::commands::repo::reconcile_repo_files;
use crate::engine::generator::generate_repo_readme;
use crate::engine::{ExtensionGenerator, IconEngine, SiteAnalyzer};
use crate::github::{GithubApiClient, GithubAuthManager, RepoInfo};
use crate::models::ExtensionItem;
use crate::state::AppState;
use std::fs;

/// Idioma de los textos visibles: el elegido en AEXT prevalece; si no se envió
/// (o vino vacío), se mantienen los textos del idioma de la extensión.
pub(crate) fn resolve_update_lang<'a>(requested: Option<&'a str>, existing: &'a str) -> &'a str {
    requested
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(existing)
}

/// Regenera una extensión existente (re-analiza su sitio, sube la versión y
/// reescribe los archivos locales y, si aplica, el repositorio de GitHub).
#[tauri::command]
pub(crate) async fn update_extension_inner(
    pkg_id: String,
    requested_lang: Option<String>,
    state: &AppState,
) -> Result<ExtensionItem, String> {
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
    let analysis = analyzer
        .analyze(&existing.base_url)
        .await
        .map_err(|e| format!("No se pudo re-analizar {}: {}", existing.base_url, e))?;

    let next_code = existing.version_code + 1;
    let next_version = format!("16.{}", next_code);

    let (mipmaps, icon_base64) = IconEngine::fetch_and_generate_icons(
        analysis.favicon_url.as_deref(),
        &existing.name,
    )
    .await
    .unwrap_or_else(|_| (std::collections::HashMap::new(), String::new()));

    // El idioma elegido sólo traduce textos. La identidad del paquete se
    // mantiene: una actualización nunca debe crear otro módulo con otro lang.
    let labels_lang = resolve_update_lang(requested_lang.as_deref(), existing.lang.as_str());

    let mut bundle = ExtensionGenerator::generate_with_version(
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
        updated.icon_base64 = Some(icon_base64.clone());
    }

    // 1. Reescribir siempre la copia local (sirve también como fuente del ZIP).
    let target_dir = local_extension_dir(&state, &bundle.pkg_id);
    let _ = fs::remove_dir_all(&target_dir);
    let has_repo = existing.github_repo_url.is_some();
    if !has_repo {
        // Sin repositorio no hay rama `gh-pages`: el bundle local solo lleva
        // lo necesario para compilar (sin .github/, icon/ ni README con
        // placeholders de repo).
        prepare_bundle_for_local(&mut bundle.files);
    }
    write_bundle_to_dir(&target_dir, &bundle.files)?;
    write_icons_to_dir(&target_dir, &bundle.module_path, &mipmaps)?;
    if has_repo {
        write_repo_icons(&target_dir, &bundle.pkg_id, &mipmaps)?;
    }
    if existing.pkg_id != bundle.pkg_id {
        // El paquete cambió de id: borrar la carpeta local antigua.
        let _ = fs::remove_dir_all(local_extension_dir(&state, &existing.pkg_id));
    }
    if updated.github_repo_url.is_none() {
        updated.apk_download_url = Some(format!(
            "{}/api/download/{}",
            LOCAL_API_BASE, bundle.pkg_id
        ));
    }

    // 2. Si vive en GitHub, subir la nueva versión al repo existente.
    let mut update_actions: Option<(GithubApiClient, String, String, String)> = None;
    if let Some(repo_url) = existing.github_repo_url.clone() {
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

        let (owner, repo_name) = parse_github_url(&repo_url)
            .ok_or_else(|| format!("URL de repositorio no válida: {}", repo_url))?;
        let repo: RepoInfo = client
            .ensure_repository(&owner, &repo_name, false, &gh_user.username)
            .await?;

        // Igual que al generar: completar el scaffold solo donde falte y
        // respetar README, firma, workflow y build files que ya existan.
        bundle.files.insert(
            "README.md".to_string(),
            generate_repo_readme(&repo.owner, &repo.name, &repo.html_url).into_bytes(),
        );
        reconcile_repo_files(&client, &repo, &mut bundle).await?;

        let mut all_files: Vec<(String, Vec<u8>)> = bundle
            .files
            .iter()
            .map(|(path, content)| (path.clone(), content.clone()))
            .collect();
        for (res_dir, icon_bytes) in &mipmaps {
            all_files.push((
                format!(
                    "{}/src/main/res/{}/ic_launcher.png",
                    bundle.module_path, res_dir
                ),
                icon_bytes.clone(),
            ));
            if res_dir == "mipmap-xxxhdpi" || res_dir == "mipmap-xxhdpi" {
                all_files.push((format!("icon/{}.png", bundle.pkg_id), icon_bytes.clone()));
                all_files.push(("icon/ic_launcher.png".to_string(), icon_bytes.clone()));
            }
        }
        let branch = repo.default_branch.clone();
        let commit_sha = client
            .commit_files(
                &repo.owner,
                &repo.name,
                &all_files,
                COMMIT_MSG_UPDATE,
                &branch,
            )
            .await
            .map_err(|e| format!("No se pudo actualizar GitHub: {}", e))?;
        updated.actions_commit_sha = Some(commit_sha.clone());
        update_actions = Some((
            client,
            repo.owner.clone(),
            repo.name.clone(),
            commit_sha.clone(),
        ));
        state
            .add_log(
                "INFO",
                &format!(
                    "Actualización subida en un solo commit ({}) a {}/{}. Verificando Actions…",
                    &commit_sha[..commit_sha.len().min(7)],
                    repo.owner,
                    repo.name
                ),
            )
            .await;

        updated.github_repo_url = Some(repo.html_url.clone());
        updated.nekoyomi_repo_url = Some(format!(
            "https://raw.githubusercontent.com/{}/gh-pages/index.min.json",
            repo.full_name
        ));
        updated.apk_download_url = None;
    }

    {
        let mut list = state.extensions.write().await;
        list.retain(|x| {
            x.pkg_id != updated.pkg_id
                && x.id != updated.id
                && x.pkg_id != existing.pkg_id
                && x.id != existing.id
        });
        list.push(updated.clone());
    }
    state.persist_extensions().await;

    // El estado final lo decide GitHub Actions, no la subida del código.
    if let Some((gh_client, owner, repo_name, sha)) = update_actions {
        verify_github_actions(&state, &gh_client, &owner, &repo_name, &sha, &updated.pkg_id)
            .await;
        if let Some(latest) = state
            .extensions
            .read()
            .await
            .iter()
            .find(|e| e.pkg_id == updated.pkg_id || e.id == updated.id)
            .cloned()
        {
            updated = latest;
        }
    }

    state
        .add_log(
            "SUCCESS",
            &format!("Extensión {} actualizada a v{}", updated.name, updated.version),
        )
        .await;

    Ok(updated)
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
