//! Tipos compartidos por las fases de preparación y publicación de updates.
//! Una actualización se prepara por completo antes de tocar GitHub: así todas
//! las extensiones de un repo pueden subirse en un único commit.

use crate::github::{GithubApiClient, RepoInfo};
use crate::models::ExtensionItem;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Repo destino de un grupo de extensiones. El cliente no es `Clone`, por eso
/// se mueve junto con la ficha preparada hasta la fase de publicación.
pub(crate) struct RepoTarget {
    pub client: GithubApiClient,
    pub repo: RepoInfo,
}

impl RepoTarget {
    pub(crate) fn key(&self) -> String {
        format!(
            "{}/{}",
            self.repo.owner.to_ascii_lowercase(),
            self.repo.name.to_ascii_lowercase()
        )
    }
}

/// Resultado de regenerar una extensión, todavía sin commit ni verificación.
pub(crate) struct PreparedUpdate {
    pub item: ExtensionItem,
    pub pkg_id: String,
    pub previous_pkg_id: String,
    pub module_path: String,
    pub files: HashMap<String, Vec<u8>>,
    pub repo: Option<RepoTarget>,
    pub warnings: Vec<String>,
    pub elapsed_ms: u128,
}

/// Una sola operación de actualización (individual, lote o masiva) a la vez.
/// El `commit_lock` de GitHub evita el 422, pero no evita varios runs seguidos.
pub(crate) fn update_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

#[cfg(test)]
mod tests {
    use super::RepoTarget;

    #[test]
    fn repo_key_is_generic_and_case_insensitive() {
        // La clave no debe incluir el package ni el nombre de la extensión.
        let repo = crate::github::RepoInfo {
            owner: "Owner".to_string(),
            name: "Repo".to_string(),
            full_name: "Owner/Repo".to_string(),
            html_url: "https://github.com/Owner/Repo".to_string(),
            default_branch: "main".to_string(),
            empty: false,
            created: false,
        };
        let key = RepoTarget {
            // El cliente no se usa en la clave; se construye con el default.
            client: crate::github::GithubApiClient::new(String::new()),
            repo,
        }
        .key();
        assert_eq!(key, "owner/repo");
        assert!(!key.contains('.'));
    }
}
