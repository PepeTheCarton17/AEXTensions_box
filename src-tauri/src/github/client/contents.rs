use super::models::GithubApiClient;
use super::request::{base64_decode, base64_encode, encode_path, truncate};

impl GithubApiClient {
    /// Devuelve el contenido de un archivo de texto del repositorio, o `None`
    /// si no existe. Se usa para no pisar `settings.gradle.kts` al añadir una
    /// segunda extensión al mismo repositorio.
    pub async fn get_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        branch: &str,
    ) -> Result<Option<String>, String> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/contents/{}",
            owner,
            repo,
            encode_path(path)
        );
        let mut request = self.client.get(&url);
        if !branch.is_empty() {
            request = request.query(&[("ref", branch)]);
        }

        let resp = request
            .send()
            .await
            .map_err(|e| format!("Error leyendo {} de GitHub: {}", path, e))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Ok(None);
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let encoded = json
            .get("content")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if encoded.is_empty() {
            return Ok(None);
        }

        let cleaned: String = encoded.chars().filter(|c| !c.is_whitespace()).collect();
        let bytes = base64_decode(&cleaned).ok_or_else(|| {
            format!("GitHub devolvió contenido no válido para {}", path)
        })?;
        Ok(Some(String::from_utf8_lossy(&bytes).to_string()))
    }

    /// Indica si un archivo existe en la rama indicada. Se usa para decidir,
    /// archivo por archivo, qué partes del scaffold faltan en un repositorio
    /// existente: las presentes son del usuario y jamás se sobrescriben.
    pub async fn file_exists(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        branch: &str,
    ) -> Result<bool, String> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/contents/{}",
            owner,
            repo,
            encode_path(path)
        );
        let mut request = self.client.get(&url);
        if !branch.is_empty() {
            request = request.query(&[("ref", branch)]);
        }
        let resp = request.send().await.map_err(|e| {
            format!("Error comprobando {} en GitHub: {}", path, e)
        })?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(false);
        }
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "No se pudo comprobar {} en {}/{} (HTTP {}): {}",
                path,
                owner,
                repo,
                status,
                truncate(&body, 200)
            ));
        }
        Ok(true)
    }

    /// Crea o actualiza un archivo. Pasa `branch` vacío para el primer commit
    /// de un repositorio que todavía no tiene ramas.
    pub async fn put_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        content: &[u8],
        commit_message: &str,
        branch: &str,
    ) -> Result<(), String> {
        let encoded_path = encode_path(path);
        let url = format!(
            "https://api.github.com/repos/{}/{}/contents/{}",
            owner, repo, encoded_path
        );

        // Necesitamos el SHA si el archivo ya existe para poder sobrescribirlo.
        let mut sha: Option<String> = None;
        let mut get_req = self.client.get(&url);
        if !branch.is_empty() {
            get_req = get_req.query(&[("ref", branch)]);
        }
        if let Ok(res) = get_req.send().await {
            if res.status().is_success() {
                if let Ok(json) = res.json::<serde_json::Value>().await {
                    if let Some(s) = json.get("sha").and_then(|v| v.as_str()) {
                        sha = Some(s.to_string());
                    }
                }
            }
        }

        let base64_content = base64_encode(content);

        let mut body = serde_json::json!({
            "message": commit_message,
            "content": base64_content,
        });
        if !branch.is_empty() {
            body["branch"] = serde_json::Value::String(branch.to_string());
        }
        if let Some(s) = sha {
            body["sha"] = serde_json::Value::String(s);
        }

        let resp = self
            .client
            .put(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Error al subir archivo {}: {}", path, e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            return Err(format!(
                "Fallo subiendo {} a GitHub (HTTP {}): {}",
                path,
                status,
                truncate(&err, 300)
            ));
        }

        Ok(())
    }
}
