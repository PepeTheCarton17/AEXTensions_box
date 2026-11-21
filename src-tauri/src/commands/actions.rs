use crate::commands::parsers::parse_github_url;
use crate::github::{GithubApiClient, GithubAuthManager};
use crate::models::ExtensionItem;
use crate::state::AppState;
use tauri::State;

/// Comprueba el run de GitHub Actions disparado por `commit_sha` y refleja su
/// resultado REAL. AEXT no afirma que la compilación funcionó solo porque el
/// código se subiera: espera a que GitHub devuelva `conclusion == success`.
pub(crate) async fn verify_github_actions(
    state: &AppState,
    client: &GithubApiClient,
    owner: &str,
    repo: &str,
    commit_sha: &str,
    pkg_id: &str,
) {
    // Un build de extensión tarda ~2-5 min en Actions. 90 s marcaba como
    // «pendiente» una actualización que luego terminaba en success, y el usuario
    // veía que «sólo se actualizaba» la extensión anterior. Esperamos lo suficiente
    // para el caso normal y dejamos el botón «Verificar Actions» para el resto.
    const MAX_WAIT_SECS: u64 = 360;
    const POLL_SECS: u64 = 10;
    const NO_RUN_GRACE_SECS: u64 = 30;
    let started = std::time::Instant::now();
    let short_sha = &commit_sha[..commit_sha.len().min(7)];
    let actions_url = format!("https://github.com/{}/{}/actions", owner, repo);
    let mut saw_run = false;
    let mut last_error = String::new();

    loop {
        match client
            .get_workflow_run_for_sha(owner, repo, commit_sha)
            .await
        {
            Ok(Some(run)) => {
                saw_run = true;
                last_error.clear();
                let finished = run.status == "completed";
                let conclusion = run.conclusion.clone().unwrap_or_default();
                {
                    let mut list = state.extensions.write().await;
                    if let Some(ext) = list
                        .iter_mut()
                        .find(|e| e.pkg_id == pkg_id || e.id == pkg_id)
                    {
                        ext.actions_run_url = Some(run.html_url.clone());
                        if finished {
                            if conclusion == "success" {
                                ext.build_status = Some("success".to_string());
                                ext.build_error = None;
                            } else {
                                ext.build_status = Some("error".to_string());
                                ext.build_error = Some(format!(
                                    "GitHub Actions: {} — {}",
                                    conclusion, run.html_url
                                ));
                                ext.build_apk_path = None;
                            }
                        } else {
                            ext.build_status = Some("building".to_string());
                            ext.build_error = None;
                        }
                    }
                }
                state.persist_extensions().await;

                if finished {
                    let level = if conclusion == "success" {
                        "SUCCESS"
                    } else {
                        "ERROR"
                    };
                    state
                        .add_log(
                            level,
                            &format!(
                                "GitHub Actions para {} ({}): {} — {}",
                                pkg_id, short_sha, conclusion, run.html_url
                            ),
                        )
                        .await;
                    return;
                }
            }
            Ok(None) => {
                let no_run_message = format!(
                    "GitHub Actions no inició ningún run para el commit {} en {}/{} tras {} s. Comprueba que exista `.github/workflows/build_push.yml` y que Actions esté habilitado en el repo; después pulsa «Verificar Actions». Revisa: {}",
                    short_sha, owner, repo, NO_RUN_GRACE_SECS, actions_url
                );
                let timed_out_no_run = !saw_run && started.elapsed().as_secs() >= NO_RUN_GRACE_SECS;
                {
                    let mut list = state.extensions.write().await;
                    if let Some(ext) = list.iter_mut().find(|e| e.pkg_id == pkg_id || e.id == pkg_id) {
                        if ext.actions_run_url.is_none() {
                            ext.actions_run_url = Some(actions_url.clone());
                        }
                        if timed_out_no_run {
                            ext.build_status = Some("error".to_string());
                            ext.build_error = Some(no_run_message.clone());
                            ext.build_apk_path = None;
                        } else {
                            ext.build_status = Some("building".to_string());
                            ext.build_error = None;
                        }
                    }
                }
                state.persist_extensions().await;
                if timed_out_no_run {
                    state.add_log("ERROR", &no_run_message).await;
                    return;
                }
            }
            Err(e) => {
                last_error = e;
                state
                    .add_log(
                        "WARN",
                        &format!("No se pudo consultar Actions ({}): {}", short_sha, last_error),
                    )
                    .await;
            }
        }

        if started.elapsed().as_secs() >= MAX_WAIT_SECS {
            // Nunca dejar «compilando» forever: si hay un run activo sigue en
            // curso; si ni siquiera pudimos consultar, marcamos pendiente con
            // un mensaje accionable y el enlace a Actions.
            let detail = if last_error.is_empty() {
                format!(
                    "El run de GitHub Actions para {} sigue activo tras {} s. AEXT no puede afirmar que haya terminado; pulsa «Verificar Actions» para consultar el resultado final.",
                    short_sha, MAX_WAIT_SECS
                )
            } else {
                format!(
                    "No se pudo confirmar el resultado de GitHub Actions para {} tras {} s ({}). Revisa el repo y pulsa «Verificar Actions».",
                    short_sha, MAX_WAIT_SECS, last_error
                )
            };
            {
                let mut list = state.extensions.write().await;
                if let Some(ext) = list.iter_mut().find(|e| e.pkg_id == pkg_id || e.id == pkg_id) {
                    if ext.actions_run_url.is_none() {
                        ext.actions_run_url = Some(actions_url.clone());
                    }
                    ext.build_status = Some("pending".to_string());
                    ext.build_error = Some(detail.clone());
                    ext.build_apk_path = None;
                }
            }
            state.persist_extensions().await;
            state.add_log("WARN", &detail).await;
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(POLL_SECS)).await;
    }
}

/// Vuelve a consultar el resultado real de GitHub Actions para el último commit
/// que AEXT subió. Útil cuando el run tardó más que la espera inicial.
#[tauri::command]
pub async fn refresh_github_actions(
    pkg_id: String,
    state: State<'_, AppState>,
) -> Result<ExtensionItem, String> {
    let ext = state
        .extensions
        .read()
        .await
        .iter()
        .find(|e| e.pkg_id == pkg_id || e.id == pkg_id)
        .cloned()
        .ok_or_else(|| format!("No se encontró la extensión {}", pkg_id))?;

    let sha = ext.actions_commit_sha.clone().ok_or_else(|| {
        "Esta extensión no tiene un commit de AEXT registrado: genera la extensión de nuevo para poder verificar Actions.".to_string()
    })?;
    let repo_url = ext
        .github_repo_url
        .clone()
        .ok_or_else(|| "Esta extensión no tiene un repositorio de GitHub asociado.".to_string())?;
    let (owner, repo) = parse_github_url(&repo_url)
        .ok_or_else(|| format!("URL de repositorio no válida: {}", repo_url))?;

    let auth = GithubAuthManager::new();
    let token = match auth.get_saved_token() {
        Some(t) => t,
        None => auth
            .extract_and_save_cli_token()
            .await
            .map_err(|_| {
                "Necesitas iniciar sesión en GitHub para consultar Actions.".to_string()
            })?
            .0,
    };
    let client = GithubApiClient::new(token);

    state
        .add_log(
            "INFO",
            &format!("Consultando de nuevo el resultado de Actions para {}…", ext.pkg_id),
        )
        .await;
    verify_github_actions(&state, &client, &owner, &repo, &sha, &ext.pkg_id).await;

    let updated = state
        .extensions
        .read()
        .await
        .iter()
        .find(|e| e.pkg_id == ext.pkg_id || e.id == ext.id)
        .cloned()
        .unwrap_or(ext);
    Ok(updated)
}
