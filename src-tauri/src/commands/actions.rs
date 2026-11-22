use crate::commands::parsers::parse_github_url;
use crate::github::{GithubApiClient, GithubAuthManager};
use crate::models::ExtensionItem;
use crate::state::AppState;
use tauri::State;

/// Espera el run de Actions disparado por `commit_sha` y refleja su resultado
/// REAL en todas las extensiones del lote. Un solo commit comparte un solo run.
const MAX_WAIT_SECS: u64 = 360;
const POLL_SECS: u64 = 10;
/// GitHub tarda en indexar un run recién creado. Antes 30 s bastaban para
/// marcar un error falso ("no inició ningún run") en una compilación válida.
const NO_RUN_GRACE_SECS: u64 = 120;

pub(crate) async fn verify_github_actions(
    state: &AppState,
    client: &GithubApiClient,
    owner: &str,
    repo: &str,
    commit_sha: &str,
    pkg_ids: &[String],
) {
    let started = std::time::Instant::now();
    let short_sha = &commit_sha[..commit_sha.len().min(7)];
    let actions_url = format!("https://github.com/{}/{}/actions", owner, repo);
    let mut saw_run = false;
    let mut last_error = String::new();

    state
        .add_log(
            "INFO",
            &format!(
                "Esperando Actions para {} ({} extensión(es)): {}",
                short_sha,
                pkg_ids.len(),
                actions_url
            ),
        )
        .await;

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
                let (status, error) = if !finished {
                    ("building", None)
                } else if conclusion == "success" {
                    ("success", None)
                } else {
                    (
                        "error",
                        Some(format!("GitHub Actions: {} — {}", conclusion, run.html_url)),
                    )
                };

                {
                    let mut list = state.extensions.write().await;
                    for ext in list.iter_mut() {
                        if !belongs(ext, pkg_ids) {
                            continue;
                        }
                        ext.actions_run_url = Some(run.html_url.clone());
                        ext.build_status = Some(status.to_string());
                        ext.build_error = error.clone();
                        if error.is_some() {
                            ext.build_apk_path = None;
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
                                "GitHub Actions para {} ({}): {} — {} ({} s)",
                                pkg_ids.join(", "),
                                short_sha,
                                conclusion,
                                run.html_url,
                                started.elapsed().as_secs()
                            ),
                        )
                        .await;
                    if conclusion == "success" {
                        report_build_details(state, owner, repo).await;
                    }
                    return;
                }
            }
            Ok(None) => {
                let elapsed = started.elapsed().as_secs();
                if elapsed >= NO_RUN_GRACE_SECS {
                    let detail = format!(
                        "GitHub aún no indexó el run del commit {} tras {} s. El estado queda pendiente (no fallido); pulsa «Verificar Actions» para confirmar. Repo: {}",
                        short_sha, elapsed, actions_url
                    );
                    set_status(state, pkg_ids, "pending", Some(detail.clone()), &actions_url).await;
                    state.add_log("WARN", &detail).await;
                    return;
                }
                set_status(state, pkg_ids, "building", None, &actions_url).await;
            }
            Err(err) => {
                last_error = err;
                state
                    .add_log(
                        "WARN",
                        &format!("No se pudo consultar Actions ({}): {}", short_sha, last_error),
                    )
                    .await;
            }
        }

        let elapsed = started.elapsed().as_secs();
        if elapsed >= MAX_WAIT_SECS {
            let detail = if saw_run {
                format!(
                    "El run de {} sigue activo tras {} s. AEXT no puede afirmar que terminó; pulsa «Verificar Actions».",
                    short_sha, elapsed
                )
            } else {
                format!(
                    "No se pudo confirmar Actions para {} tras {} s ({}). Revisa el repo y pulsa «Verificar Actions».",
                    short_sha, elapsed, last_error
                )
            };
            set_status(state, pkg_ids, "pending", Some(detail.clone()), &actions_url).await;
            state.add_log("WARN", &detail).await;
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(POLL_SECS)).await;
    }
}

/// El workflow publica `build-report.json` en `gh-pages`. Es best-effort: si
/// GitHub aún no propagó la rama, seguimos teniendo el resultado del run.
async fn report_build_details(state: &AppState, owner: &str, repo: &str) {
    let url = format!(
        "https://raw.githubusercontent.com/{}/{}/gh-pages/build-report.json",
        owner, repo
    );
    let http = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(_) => return,
    };
    let report = match http.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => resp.json::<serde_json::Value>().await.ok(),
        _ => None,
    };
    let Some(report) = report else {
        state
            .add_log(
                "WARN",
                &format!(
                    "No se pudo leer el informe de compilación publicado ({}). El resultado de Actions sí está confirmado.",
                    url
                ),
            )
            .await;
        return;
    };

    let num = |value: Option<&serde_json::Value>| -> usize {
        value.and_then(serde_json::Value::as_u64).unwrap_or(0) as usize
    };
    let totals = report.get("totals");
    let modules = num(totals.and_then(|v| v.get("modules")));
    let seconds = num(totals.and_then(|v| v.get("seconds")));
    let failed = num(totals.and_then(|v| v.get("failed")));
    let skipped = num(report.get("skipped_sources"));
    state
        .add_log(
            if failed == 0 { "INFO" } else { "ERROR" },
            &format!(
                "Informe de compilación: {} módulo(s), {} s, {} fallido(s), {} fuente(s) omitida(s).",
                modules, seconds, failed, skipped
            ),
        )
        .await;

    if let Some(rows) = report.get("modules").and_then(serde_json::Value::as_array) {
        for row in rows {
            let text = |key: &str| {
                row.get(key)
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("?")
            };
            state
                .add_log(
                    "INFO",
                    &format!(
                        "  {} v{} (code {}): {} en {} s, {} warning(s), {} error(es).",
                        text("module"),
                        text("version"),
                        text("code"),
                        text("status"),
                        text("seconds"),
                        text("warnings"),
                        text("errors")
                    ),
                )
                .await;
        }
    }
    if let Some(skipped) = report.get("skipped_sources").and_then(serde_json::Value::as_array) {
        for item in skipped {
            let module = item
                .get("module")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("?");
            let detail = item
                .get("detail")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            state
                .add_log("WARN", &format!("  Fuente omitida en {}: {}", module, detail))
                .await;
        }
    }
}

fn belongs(ext: &ExtensionItem, pkg_ids: &[String]) -> bool {
    pkg_ids
        .iter()
        .any(|id| ext.pkg_id == id.as_str() || ext.id == id.as_str())
}

async fn set_status(
    state: &AppState,
    pkg_ids: &[String],
    status: &str,
    error: Option<String>,
    actions_url: &str,
) {
    {
        let mut list = state.extensions.write().await;
        for ext in list.iter_mut() {
            if !belongs(ext, pkg_ids) {
                continue;
            }
            if ext.actions_run_url.is_none() {
                ext.actions_run_url = Some(actions_url.to_string());
            }
            ext.build_status = Some(status.to_string());
            ext.build_error = error.clone();
            if error.is_some() {
                ext.build_apk_path = None;
            }
        }
    }
    state.persist_extensions().await;
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
            .map_err(|_| "Necesitas iniciar sesión en GitHub para consultar Actions.".to_string())?
            .0,
    };
    let client = GithubApiClient::new(token);

    state
        .add_log(
            "INFO",
            &format!("Consultando de nuevo el resultado de Actions para {}…", ext.pkg_id),
        )
        .await;
    let pkg_ids = [ext.pkg_id.clone()];
    verify_github_actions(&state, &client, &owner, &repo, &sha, &pkg_ids).await;

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

#[cfg(test)]
mod tests {
    use super::{belongs, NO_RUN_GRACE_SECS};

    #[test]
    fn batch_wait_does_not_fail_on_indexing_delay() {
        assert!(NO_RUN_GRACE_SECS >= 120);
    }

    #[test]
    fn belongs_matches_any_package_in_the_batch() {
        let ext = crate::models::ExtensionItem {
            id: "a.b.c".to_string(),
            pkg_id: "a.b.c".to_string(),
            name: String::new(),
            class_name: String::new(),
            base_url: String::new(),
            lang: "all".to_string(),
            ext_type: "anime".to_string(),
            version: String::new(),
            version_code: 1,
            cms: String::new(),
            has_sub_dub: false,
            icon_base64: None,
            github_repo_url: None,
            nekoyomi_repo_url: None,
            apk_download_url: None,
            created_at: String::new(),
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
        assert!(belongs(&ext, &["other".to_string(), "a.b.c".to_string()]));
        assert!(!belongs(&ext, &["other".to_string()]));
    }
}
