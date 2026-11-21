use super::models::{
    repo_info_from_json, CreateRepoParams, GithubApiClient, RepoInfo, RepoSummary,
};
use super::request::{encode_path, truncate};

impl GithubApiClient {
    /// Consulta un repositorio existente sin crear nada. La usa el borrado para
    /// saber la rama por defecto y no asumir que siempre es `main`.
    pub async fn get_repository(&self, owner: &str, repo_name: &str) -> Result<RepoInfo, String> {
        let url = format!(
            "https://api.github.com/repos/{}/{}",
            encode_path(owner),
            encode_path(repo_name)
        );
        let resp = self.client.get(&url).send().await
            .map_err(|e| format!("Error consultando el repositorio: {}", e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            return Err(format!(
                "No se pudo acceder a {}/{} (HTTP {}): {}",
                owner,
                repo_name,
                status,
                truncate(&err, 200)
            ));
        }
        let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        Ok(repo_info_from_json(&data, owner, repo_name, false))
    }

    /// Obtiene un repositorio existente o lo crea si no existe.
    ///
    /// `authenticated_user` es el login del usuario dueño del token: si el
    /// repositorio pedido pertenece a otra persona y no existe, devolvemos un
    /// error claro en lugar de intentar crear un repo con el mismo nombre en
    /// la cuenta equivocada (comportamiento anterior que duplicaba repos).
    pub async fn ensure_repository(
        &self,
        owner: &str,
        repo_name: &str,
        private: bool,
        authenticated_user: &str,
    ) -> Result<RepoInfo, String> {
        let check_url = format!("https://api.github.com/repos/{}/{}", owner, repo_name);
        let resp = self
            .client
            .get(&check_url)
            .send()
            .await
            .map_err(|e| format!("Error verificando repo en GitHub: {}", e))?;

        if resp.status().is_success() {
            let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            let mut info = repo_info_from_json(&data, owner, repo_name, false);
            // `size == 0` también ocurre en repos con un commit mínimo: solo
            // son "vacíos" si la API de commits confirma que no hay ninguno.
            if info.empty {
                info.empty = !self.has_commits(owner, repo_name).await;
            }
            return Ok(info);
        }

        if resp.status() != reqwest::StatusCode::NOT_FOUND {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "No se pudo acceder al repositorio {}/{} (HTTP {}): {}",
                owner,
                repo_name,
                status,
                truncate(&body, 300)
            ));
        }

        // El repo no existe. Solo podemos crearlo en la cuenta autenticada.
        if !owner.eq_ignore_ascii_case(authenticated_user) {
            return Err(format!(
                "El repositorio {}/{} no existe o tu token no tiene acceso. \
                 Si quieres crear uno nuevo, deja el campo vacío o usa tu propio usuario (@{}).",
                owner, repo_name, authenticated_user
            ));
        }

        let create_url = "https://api.github.com/user/repos";
        let params = CreateRepoParams {
            name: repo_name.to_string(),
            description: Some(
                "Repositorio de extensiones Nekoyomi generado automáticamente por AEXT".to_string(),
            ),
            private,
            auto_init: true,
        };

        let create_resp = self
            .client
            .post(create_url)
            .json(&params)
            .send()
            .await
            .map_err(|e| format!("Error creando repositorio en GitHub: {}", e))?;

        if !create_resp.status().is_success() {
            let status = create_resp.status();
            let err_body = create_resp.text().await.unwrap_or_default();
            return Err(format!(
                "Error al crear el repositorio {} (HTTP {}): {}",
                repo_name,
                status,
                truncate(&err_body, 300)
            ));
        }

        let created_data: serde_json::Value =
            create_resp.json().await.map_err(|e| e.to_string())?;
        let mut info = repo_info_from_json(&created_data, owner, repo_name, true);
        info.created = true;
        info.empty = false; // auto_init crea el commit inicial
        Ok(info)
    }

    /// Comprueba si el repositorio tiene al menos un commit.
    async fn has_commits(&self, owner: &str, repo_name: &str) -> bool {
        let url = format!(
            "https://api.github.com/repos/{}/{}/commits?per_page=1",
            owner, repo_name
        );
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => resp
                .json::<serde_json::Value>()
                .await
                .ok()
                .and_then(|value| value.as_array().map(|items| !items.is_empty()))
                .unwrap_or(false),
            _ => false,
        }
    }

    /// Lista los repositorios a los que el token tiene acceso.
    pub async fn list_repositories(&self) -> Result<Vec<RepoSummary>, String> {
        let url = "https://api.github.com/user/repos?per_page=100&sort=updated&affiliation=owner,collaborator,organization_member";
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Error listando repositorios: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "GitHub devolvió HTTP {} al listar repositorios: {}",
                status,
                truncate(&body, 300)
            ));
        }

        let data: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;
        Ok(data
            .into_iter()
            .filter_map(|repo| {
                let full_name = repo.get("full_name")?.as_str()?.to_string();
                let name = repo.get("name")?.as_str()?.to_string();
                let owner = repo
                    .get("owner")
                    .and_then(|o| o.get("login"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let html_url = repo
                    .get("html_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let default_branch = repo
                    .get("default_branch")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("main")
                    .to_string();
                let private = repo.get("private").and_then(|v| v.as_bool()).unwrap_or(false);
                Some(RepoSummary {
                    owner,
                    name,
                    full_name,
                    html_url,
                    default_branch,
                    private,
                })
            })
            .collect())
    }
}
