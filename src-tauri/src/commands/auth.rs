use crate::github::{GithubApiClient, GithubAuthManager, RepoSummary};
use crate::models::GithubAccountInfo;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_gh_auth_status(
    state: State<'_, AppState>,
) -> Result<Option<GithubAccountInfo>, String> {
    let auth = GithubAuthManager::new();

    // Primero intentamos token guardado
    if let Some(token) = auth.get_saved_token() {
        if let Ok(info) = auth.verify_token(&token).await {
            let mut gh_state = state.gh_user.write().await;
            *gh_state = Some(info.clone());
            return Ok(Some(info));
        }
    }

    // Si estamos en PC, intentamos detectar CLI
    if let Ok(cli_info) = auth.detect_cli_session().await {
        let mut gh_state = state.gh_user.write().await;
        *gh_state = Some(cli_info.clone());
        return Ok(Some(cli_info));
    }

    Ok(None)
}

#[tauri::command]
pub async fn login_with_token(
    token: String,
    state: State<'_, AppState>,
) -> Result<GithubAccountInfo, String> {
    let auth = GithubAuthManager::new();
    let info = auth.verify_token(&token).await?;
    auth.save_token(&token)?;

    let mut gh_state = state.gh_user.write().await;
    *gh_state = Some(info.clone());
    state
        .add_log("SUCCESS", &format!("Sesión iniciada con GitHub (@{})", info.username))
        .await;

    Ok(info)
}

#[tauri::command]
pub async fn login_with_cli(state: State<'_, AppState>) -> Result<GithubAccountInfo, String> {
    let auth = GithubAuthManager::new();
    let info = auth.detect_cli_session().await?;

    let mut gh_state = state.gh_user.write().await;
    *gh_state = Some(info.clone());
    state
        .add_log("SUCCESS", &format!("Sesión detectada vía GitHub CLI (@{})", info.username))
        .await;

    Ok(info)
}

#[tauri::command]
pub async fn logout_gh(state: State<'_, AppState>) -> Result<(), String> {
    let auth = GithubAuthManager::new();
    let _ = auth.clear_token();

    let mut gh_state = state.gh_user.write().await;
    *gh_state = None;
    state.add_log("INFO", "Sesión de GitHub cerrada").await;

    Ok(())
}

/// Devuelve los repositorios del usuario autenticado para poblar el selector
/// del modal de generación (así no se crea un repo nuevo sin querer).
#[tauri::command]
pub async fn list_github_repos(state: State<'_, AppState>) -> Result<Vec<RepoSummary>, String> {
    let auth = GithubAuthManager::new();
    let token = match auth.get_saved_token() {
        Some(t) => t,
        None => match auth.extract_and_save_cli_token().await {
            Ok((t, info)) => {
                let mut gh_state = state.gh_user.write().await;
                *gh_state = Some(info);
                t
            }
            Err(_) => return Ok(Vec::new()),
        },
    };

    let client = GithubApiClient::new(token);
    client.list_repositories().await
}
