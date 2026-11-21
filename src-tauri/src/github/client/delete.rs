use super::models::GithubApiClient;
use super::request::{base64_encode, encode_path, truncate};

impl GithubApiClient {
    /// Elimina de GitHub únicamente los archivos que viven bajo `module_path`
    /// (p. ej. `src/es/miextension`). Usa la API Git Data para que sea un solo
    /// commit: no toca README, licencia, disclaimer, workflow, wrapper ni los
    /// demás módulos del repositorio.
    ///
    /// Devuelve `None` cuando el módulo ya no existe en GitHub (para que el
    /// llamador pueda seguir con la limpieza local sin inventar un commit).
    pub async fn delete_module(
        &self,
        owner: &str,
        repo: &str,
        module_path: &str,
        message: &str,
        branch: &str,
    ) -> Result<Option<String>, String> {
        let api = format!("https://api.github.com/repos/{}/{}", owner, repo);
        let branch = if branch.is_empty() { "main" } else { branch };

        let ref_url = format!("{}/git/ref/heads/{}", api, encode_path(branch));
        let resp = self.client.get(&ref_url).send().await
            .map_err(|e| format!("Error consultando la rama {}: {}", branch, e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            return Err(format!("No se pudo leer la rama {} (HTTP {}): {}", branch, status, truncate(&err, 200)));
        }
        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let base_commit = json
            .get("object")
            .and_then(|o| o.get("sha"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| "GitHub no devolvió el SHA del commit base".to_string())?
            .to_string();

        let commit_url = format!("{}/git/commits/{}", api, base_commit);
        let resp = self.client.get(&commit_url).send().await
            .map_err(|e| format!("Error consultando el commit base: {}", e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            return Err(format!("No se pudo leer el commit base (HTTP {}): {}", status, truncate(&err, 200)));
        }
        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let base_tree = json
            .get("tree")
            .and_then(|t| t.get("sha"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| "GitHub no devolvió el SHA del árbol base".to_string())?
            .to_string();

        let tree_url = format!("{}/git/trees/{}?recursive=1", api, encode_path(&base_tree));
        let resp = self.client.get(&tree_url).send().await
            .map_err(|e| format!("Error listando el árbol de GitHub: {}", e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            return Err(format!("No se pudo listar el árbol (HTTP {}): {}", status, truncate(&err, 200)));
        }
        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        if json.get("truncated").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Err("GitHub recortó el árbol del repositorio; no es seguro borrar el módulo sin ver todos sus archivos".to_string());
        }

        let prefix = format!("{}/", module_path.trim_end_matches('/'));
        let mut changes = Vec::new();
        if let Some(items) = json.get("tree").and_then(|v| v.as_array()) {
            for item in items {
                if item.get("type").and_then(|v| v.as_str()) != Some("blob") {
                    continue;
                }
                let Some(path) = item.get("path").and_then(|v| v.as_str()) else { continue };
                if path.starts_with(&prefix) || path == module_path {
                    // `sha: null` le dice a GitHub que elimine esa ruta en el
                    // árbol nuevo, conservando el resto gracias a base_tree.
                    changes.push(serde_json::json!({
                        "path": path,
                        "mode": "100644",
                        "type": "blob",
                        "sha": serde_json::Value::Null,
                    }));
                }
            }
        }

        // Dejar el `include()` huérfano rompería el siguiente build de Gradle:
        // quitamos solo esa línea de settings.gradle.kts, sin tocar el resto.
        let include = format!("include(\":{}\")", module_path.replace('/', ":"));
        if let Ok(Some(settings)) = self
            .get_file_content(owner, repo, "settings.gradle.kts", branch)
            .await
        {
            if settings.contains(&include) {
                let cleaned: String = settings
                    .lines()
                    .filter(|line| line.trim() != include)
                    .collect::<Vec<_>>()
                    .join("\n");
                let mut cleaned = cleaned;
                cleaned.push('\n');
                let blob_resp = self.client.post(format!("{}/git/blobs", api))
                    .json(&serde_json::json!({
                        "content": base64_encode(cleaned.as_bytes()),
                        "encoding": "base64",
                    }))
                    .send().await
                    .map_err(|e| format!("Error creando el blob de settings: {}", e))?;
                if !blob_resp.status().is_success() {
                    let status = blob_resp.status();
                    let err = blob_resp.text().await.unwrap_or_default();
                    return Err(format!("Fallo creando el blob de settings (HTTP {}): {}", status, truncate(&err, 200)));
                }
                let blob_json: serde_json::Value = blob_resp.json().await.map_err(|e| e.to_string())?;
                if let Some(blob_sha) = blob_json.get("sha").and_then(|v| v.as_str()) {
                    changes.push(serde_json::json!({
                        "path": "settings.gradle.kts",
                        "mode": "100644",
                        "type": "blob",
                        "sha": blob_sha,
                    }));
                }
            }
        }
        if changes.is_empty() {
            return Ok(None);
        }

        let tree_body = serde_json::json!({
            "base_tree": base_tree,
            "tree": changes,
        });
        let resp = self.client.post(format!("{}/git/trees", api)).json(&tree_body).send().await
            .map_err(|e| format!("Error creando el árbol de borrado: {}", e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            return Err(format!("Fallo creando el árbol de borrado (HTTP {}): {}", status, truncate(&err, 300)));
        }
        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let new_tree = json.get("sha").and_then(|v| v.as_str())
            .ok_or_else(|| "GitHub no devolvió el SHA del árbol de borrado".to_string())?;

        let commit_body = serde_json::json!({
            "message": message,
            "tree": new_tree,
            "parents": [base_commit],
        });
        let resp = self.client.post(format!("{}/git/commits", api)).json(&commit_body).send().await
            .map_err(|e| format!("Error creando el commit de borrado: {}", e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            return Err(format!("Fallo creando el commit de borrado (HTTP {}): {}", status, truncate(&err, 300)));
        }
        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let commit_sha = json.get("sha").and_then(|v| v.as_str())
            .ok_or_else(|| "GitHub no devolvió el SHA del commit de borrado".to_string())?
            .to_string();

        let ref_url = format!("{}/git/refs/heads/{}", api, encode_path(branch));
        let resp = self.client.patch(&ref_url)
            .json(&serde_json::json!({ "sha": commit_sha, "force": false }))
            .send().await
            .map_err(|e| format!("Error actualizando la rama {}: {}", branch, e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            return Err(format!("Fallo actualizando la rama {} (HTTP {}): {}", branch, status, truncate(&err, 300)));
        }

        Ok(Some(commit_sha))
    }
}
