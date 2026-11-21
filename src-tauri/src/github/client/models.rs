use serde::{Deserialize, Serialize};

pub struct GithubApiClient {
    /// `pub(super)` para que los submódulos hermanos (`repos`, `contents`, ...)
    /// usen el mismo `reqwest::Client` con sus headers por defecto.
    pub(super) client: reqwest::Client,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateRepoParams {
    pub name: String,
    pub description: Option<String>,
    pub private: bool,
    pub auto_init: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub owner: String,
    pub name: String,
    pub full_name: String,
    pub html_url: String,
    pub default_branch: String,
    /// `true` cuando el repositorio acaba de crearse o no tiene commits todavía.
    pub empty: bool,
    /// `true` si AEXT tuvo que crear el repositorio.
    pub created: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSummary {
    pub owner: String,
    pub name: String,
    pub full_name: String,
    pub html_url: String,
    pub default_branch: String,
    pub private: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub id: u64,
    pub status: String,
    pub conclusion: Option<String>,
    pub html_url: String,
    pub head_sha: String,
}

pub(super) fn repo_info_from_json(
    data: &serde_json::Value,
    fallback_owner: &str,
    fallback_name: &str,
    created: bool,
) -> RepoInfo {
    let owner = data
        .get("owner")
        .and_then(|o| o.get("login"))
        .and_then(|v| v.as_str())
        .unwrap_or(fallback_owner)
        .to_string();
    let name = data
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(fallback_name)
        .to_string();
    let full_name = data
        .get("full_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}/{}", owner, name));
    let html_url = data
        .get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let default_branch = data
        .get("default_branch")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("main")
        .to_string();
    let empty = data
        .get("size")
        .and_then(|v| v.as_u64())
        .map(|size| size == 0)
        .unwrap_or(false);

    RepoInfo {
        owner,
        name,
        full_name,
        html_url,
        default_branch,
        empty,
        created,
    }
}
