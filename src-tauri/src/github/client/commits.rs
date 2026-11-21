use std::collections::HashMap;
use std::sync::OnceLock;

use super::models::GithubApiClient;
use super::request::{base64_encode, encode_path, truncate};

/// Dos actualizaciones en paralelo contra el mismo repo leían el mismo commit
/// base y luego competían por mover la rama; GitHub respondía 422/409. Se
/// serializan TODOS los commits que salen de esta instancia de AEXT.
fn commit_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// El generador emite `icon/<pkg>.png` e `icon/ic_launcher.png` una vez por
/// cada densidad. Un árbol con la misma ruta repetida produce 422
/// (`duplicate_entries`) en la API Git Data. Se conserva la última versión.
fn dedup_files(files: &[(String, Vec<u8>)]) -> Vec<(String, Vec<u8>)> {
    let mut content: HashMap<String, Vec<u8>> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for (path, bytes) in files {
        if !content.contains_key(path) {
            order.push(path.clone());
        }
        content.insert(path.clone(), bytes.clone());
    }
    order
        .into_iter()
        .filter_map(|path| {
            content
                .remove(&path)
                .map(|bytes| (path, bytes))
        })
        .collect()
}

impl GithubApiClient {
    /// Sube TODOS los archivos en un único commit mediante la API Git Data.
    ///
    /// El anterior `put_file_content` por archivo generaba un commit (y por tanto
    /// un run de Actions) por cada archivo: decenas de runs cancelados y mucho
    /// ruido. Aquí creamos blobs + un árbol + un commit + un ref en una sola
    /// operación lógica. Devuelve el SHA del commit para poder localizar el run.
    pub async fn commit_files(
        &self,
        owner: &str,
        repo: &str,
        files: &[(String, Vec<u8>)],
        message: &str,
        branch: &str,
    ) -> Result<String, String> {
        let files = dedup_files(files);
        if files.is_empty() {
            return Err("No hay archivos para subir".to_string());
        }
        // Se mantiene el candado durante todo el flujo blob → tree → commit →
        // ref para que dos extensiones del mismo repo no se pisen el SHA base.
        let _guard = commit_lock().lock().await;
        let api = format!("https://api.github.com/repos/{}/{}", owner, repo);
        let branch = if branch.is_empty() { "main" } else { branch };

        // Commit base: si no existe (repo vacío) creamos el árbol sin base_tree
        // y el commit sin padres, y luego la rama con createRef.
        let mut base_commit: Option<String> = None;
        let ref_url = format!("{}/git/ref/heads/{}", api, encode_path(branch));
        if let Ok(resp) = self.client.get(&ref_url).send().await {
            if resp.status().is_success() {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    base_commit = json
                        .get("object")
                        .and_then(|o| o.get("sha"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
            }
        }

        // 1) Un blob por archivo.
        let mut tree = Vec::with_capacity(files.len());
        for (path, content) in files {
            let url = format!("{}/git/blobs", api);
            let body = serde_json::json!({
                "content": base64_encode(&content),
                "encoding": "base64",
            });
            let resp = self.client.post(&url).json(&body).send().await
                .map_err(|e| format!("Error creando blob para {}: {}", path, e))?;
            if !resp.status().is_success() {
                let status = resp.status();
                let err = resp.text().await.unwrap_or_default();
                return Err(format!("Fallo creando blob para {} (HTTP {}): {}", path, status, truncate(&err, 200)));
            }
            let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            let sha = json.get("sha").and_then(|v| v.as_str())
                .ok_or_else(|| format!("GitHub no devolvió SHA para {}", path))?;
            tree.push(serde_json::json!({
                "path": path,
                "mode": "100644",
                "type": "blob",
                "sha": sha,
            }));
        }

        // 2) Árbol: encima del árbol del commit base para no borrar lo demás.
        let mut tree_body = serde_json::json!({ "tree": tree });
        if let Some(ref commit_sha) = base_commit {
            let commit_url = format!("{}/git/commits/{}", api, commit_sha);
            if let Ok(resp) = self.client.get(&commit_url).send().await {
                if resp.status().is_success() {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if let Some(tree_sha) = json.get("tree").and_then(|v| v.get("sha")).and_then(|v| v.as_str()) {
                            tree_body["base_tree"] = serde_json::Value::String(tree_sha.to_string());
                        }
                    }
                }
            }
        }
        let resp = self.client.post(format!("{}/git/trees", api)).json(&tree_body).send().await
            .map_err(|e| format!("Error creando árbol: {}", e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            return Err(format!("Fallo creando árbol (HTTP {}): {}", status, truncate(&err, 300)));
        }
        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let tree_sha = json.get("sha").and_then(|v| v.as_str())
            .ok_or_else(|| "GitHub no devolvió SHA del árbol".to_string())?.to_string();

        // 3) Commit con los blobs anteriores.
        let mut commit_body = serde_json::json!({
            "message": message,
            "tree": tree_sha,
        });
        if let Some(ref parent) = base_commit {
            commit_body["parents"] = serde_json::json!([parent]);
        }
        let resp = self.client.post(format!("{}/git/commits", api)).json(&commit_body).send().await
            .map_err(|e| format!("Error creando commit: {}", e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            return Err(format!("Fallo creando commit (HTTP {}): {}", status, truncate(&err, 300)));
        }
        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let commit_sha = json.get("sha").and_then(|v| v.as_str())
            .ok_or_else(|| "GitHub no devolvió SHA del commit".to_string())?.to_string();

        // 4) Mover la rama (o crearla si el repo estaba vacío).
        let (url, ref_body) = if base_commit.is_some() {
            (
                format!("{}/git/refs/heads/{}", api, encode_path(branch)),
                serde_json::json!({ "sha": commit_sha, "force": false }),
            )
        } else {
            (
                format!("{}/git/refs", api),
                serde_json::json!({
                    "ref": format!("refs/heads/{}", branch),
                    "sha": commit_sha,
                }),
            )
        };
        let resp = if base_commit.is_some() {
            self.client.patch(&url).json(&ref_body).send().await
        } else {
            self.client.post(&url).json(&ref_body).send().await
        }.map_err(|e| format!("Error actualizando la rama: {}", e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            return Err(format!("Fallo actualizando la rama {} (HTTP {}): {}", branch, status, truncate(&err, 300)));
        }

        Ok(commit_sha)
    }
}

#[cfg(test)]
mod github_client_tests {
    use super::dedup_files;

    #[test]
    fn repeated_icon_paths_are_collapsed_before_building_a_tree() {
        let files = vec![
            ("src/module/App.kt".to_string(), vec![1]),
            ("icon/ic_launcher.png".to_string(), vec![2]),
            ("icon/all.frame.png".to_string(), vec![3]),
            ("icon/ic_launcher.png".to_string(), vec![4]),
            ("icon/all.frame.png".to_string(), vec![5]),
        ];
        let deduped = dedup_files(&files);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0].0, "src/module/App.kt");
        assert_eq!(deduped[1].1, vec![4], "se conserva el último contenido");
        assert_eq!(deduped[2].1, vec![5]);
        let paths: Vec<&str> = deduped.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(paths.len(), paths.iter().collect::<std::collections::HashSet<_>>().len());
    }
}
