use crate::commands::constants::COMMIT_MSG_GENERATE;
use crate::commands::files::{local_extension_dir, write_bundle_to_dir, write_icons_to_dir, write_repo_icons};
use crate::commands::parsers::{parse_repo_target, sanitize_repo_name};
use crate::commands::repo::reconcile_repo_files;
use crate::engine::generator::generate_repo_readme;
use crate::engine::ExtensionBundle;
use crate::github::{GithubApiClient, GithubAuthManager};
use crate::models::{ExtensionItem, GenerationRequest};
use crate::state::AppState;
use std::collections::HashMap;
use std::fs;

pub(crate) struct GithubUploadInfo {
    pub(crate) client: GithubApiClient,
    pub(crate) owner: String,
    pub(crate) repo: String,
    pub(crate) sha: String,
}

/// Sube el bundle a GitHub y devuelve los datos del commit para verificar Actions.
pub(crate) async fn upload_bundle_to_github(
    state: &AppState,
    auth: &GithubAuthManager,
    request: &GenerationRequest,
    bundle: &mut ExtensionBundle,
    mipmaps: &HashMap<String, Vec<u8>>,
    ext_item: &mut ExtensionItem,
) -> Result<GithubUploadInfo, String> {
                let token = match auth.get_saved_token() {
                    Some(t) => t,
                    None => match auth.extract_and_save_cli_token().await {
                        Ok((t, info)) => {
                            let mut gh_state = state.gh_user.write().await;
                            *gh_state = Some(info);
                            state
                                .add_log(
                                    "INFO",
                                    "Token de GitHub CLI detectado y guardado automáticamente",
                                )
                                .await;
                            t
                        }
                        Err(_) => {
                            return Err("Se requiere iniciar sesión en GitHub. Ve a la sección de autenticación e ingresa tu Personal Access Token (PAT).".to_string());
                        }
                    },
                };

                let gh_user = auth.verify_token(&token).await?;
                let client = GithubApiClient::new(token);

                let raw_repo = request.repo_name.as_deref().unwrap_or("").trim();
                let (owner_opt, parsed_repo) = parse_repo_target(raw_repo);
                let repo_name = if parsed_repo.trim().is_empty() {
                    format!("nekoyomi-ext-{}", bundle.class_name.to_lowercase())
                } else {
                    sanitize_repo_name(&parsed_repo)
                };
                let owner = owner_opt.unwrap_or_else(|| gh_user.username.clone());

                state
                    .add_log(
                        "INFO",
                        &format!(
                            "Buscando / creando repositorio en GitHub: {}/{}",
                            owner, repo_name
                        ),
                    )
                    .await;
                let repo = client
                    .ensure_repository(
                        &owner,
                        &repo_name,
                        request.private_repo.unwrap_or(false),
                        &gh_user.username,
                    )
                    .await?;

                // El README genérico solo se usa si el repo no tiene uno propio
                // (o tiene únicamente el placeholder de `auto_init`).
                bundle.files.insert(
                    "README.md".to_string(),
                    generate_repo_readme(&repo.owner, &repo.name, &repo.html_url).into_bytes(),
                );
                // Comprobar los archivos del scaffold uno por uno: un repo
                // recién creado con `auto_init` NO está vacío, pero sí necesita
                // workflow, wrapper, keystore y build files. En cambio, los que
                // ya existen pertenecen al usuario y no se tocan.
                reconcile_repo_files(&client, &repo, bundle).await?;

                state
                    .add_log(
                        "INFO",
                        if repo.created {
                            "Repositorio creado. Subiendo archivos de código fuente..."
                        } else {
                            "Repositorio existente detectado. Actualizando archivos..."
                        },
                    )
                    .await;

                // Un único commit con todos los archivos e iconos. Antes cada
                // archivo producía su propio commit y, con él, un run de Actions
                // distinto: decenas de runs (casi todos cancelados) y mucho ruido.
                let mut all_files: Vec<(String, Vec<u8>)> = bundle
                    .files
                    .iter()
                    .map(|(path, content)| (path.clone(), content.clone()))
                    .collect();
                for (res_dir, icon_bytes) in mipmaps {
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
                        COMMIT_MSG_GENERATE,
                        &branch,
                    )
                    .await
                    .map_err(|e| format!("No se pudo subir la extensión a GitHub: {}", e))?;
                ext_item.actions_commit_sha = Some(commit_sha.clone());
                let upload = GithubUploadInfo {
                    client,
                    owner: repo.owner.clone(),
                    repo: repo.name.clone(),
                    sha: commit_sha.clone(),
                };
                state
                    .add_log(
                        "INFO",
                        &format!(
                            "Subidos {} archivos en un solo commit ({}) a {}/{}",
                            all_files.len(),
                            &commit_sha[..commit_sha.len().min(7)],
                            repo.owner,
                            repo.name
                        ),
                    )
                    .await;

                // Copia local del proyecto: es la fuente del ZIP de código y
                // permite ver los archivos sin clonar el repositorio.
                let target_dir = local_extension_dir(state, &ext_item.pkg_id);
                let _ = fs::remove_dir_all(&target_dir);
                write_bundle_to_dir(&target_dir, &bundle.files)?;
                write_icons_to_dir(&target_dir, &bundle.module_path, &mipmaps)?;
                write_repo_icons(&target_dir, &ext_item.pkg_id, &mipmaps)?;

                ext_item.github_repo_url = Some(repo.html_url.clone());
                ext_item.nekoyomi_repo_url = Some(format!(
                    "https://raw.githubusercontent.com/{}/gh-pages/index.min.json",
                    repo.full_name
                ));
                // En modo GitHub no existe un APK todavía: la compilación la
                // hace GitHub Actions. No inventamos una URL de APK.
                ext_item.apk_download_url = None;

                state
                    .add_log(
                        "INFO",
                        &format!(
                            "Código de {} subido a GitHub: {}. Verificando el resultado de Actions…",
                            ext_item.pkg_id, repo.html_url
                        ),
                    )
                    .await;
    Ok(upload)
}
