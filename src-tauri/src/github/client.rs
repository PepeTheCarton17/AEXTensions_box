use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};

pub struct GithubApiClient {
    client: reqwest::Client,
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

impl GithubApiClient {
    pub fn new(token: String) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("AEXT-Extension-Builder/1.0"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/vnd.github.v3+json"));
        if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", token.trim())) {
            headers.insert(AUTHORIZATION, val);
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap_or_default();

        Self { client }
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
        if files.is_empty() {
            return Err("No hay archivos para subir".to_string());
        }
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
                "content": base64_encode(content),
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

    /// Busca el run de Actions disparado por un commit concreto.
    pub async fn get_workflow_run_for_sha(
        &self,
        owner: &str,
        repo: &str,
        head_sha: &str,
    ) -> Result<Option<WorkflowRun>, String> {
        let url = format!("https://api.github.com/repos/{}/{}/actions/runs", owner, repo);
        let resp = self.client.get(&url)
            .query(&[("head_sha", head_sha), ("per_page", "20")])
            .send().await
            .map_err(|e| format!("Error consultando Actions: {}", e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            return Err(format!("Actions HTTP {}: {}", status, truncate(&err, 200)));
        }
        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let Some(runs) = json.get("workflow_runs").and_then(|v| v.as_array()) else {
            return Ok(None);
        };
        // Un repo puede tener otros workflows (p. ej. un health check
        // programado) disparados para el mismo SHA. El run que nos interesa es
        // el provocado por nuestro push, no el de un cron.
        let Some(chosen) = runs
            .iter()
            .find(|r| r.get("event").and_then(|v| v.as_str()) == Some("push"))
            .or_else(|| runs.first())
        else {
            return Ok(None);
        };
        Ok(Some(WorkflowRun {
            id: chosen.get("id").and_then(|v| v.as_u64()).unwrap_or(0),
            status: chosen.get("status").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
            conclusion: chosen.get("conclusion").and_then(|v| v.as_str()).map(|s| s.to_string()),
            html_url: chosen.get("html_url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            head_sha: chosen.get("head_sha").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }))
    }

    pub async fn get_latest_workflow_run(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Option<WorkflowRun>, String> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/actions/runs?per_page=1",
            owner, repo
        );
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Error consultando actions: {}", e))?;

        if !resp.status().is_success() {
            return Ok(None);
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        if let Some(runs) = json.get("workflow_runs").and_then(|v| v.as_array()) {
            if let Some(first) = runs.first() {
                let id = first.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                let status = first
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let conclusion = first
                    .get("conclusion")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let html_url = first
                    .get("html_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let head_sha = first
                    .get("head_sha")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                return Ok(Some(WorkflowRun {
                    id,
                    status,
                    conclusion,
                    html_url,
                    head_sha,
                }));
            }
        }

        Ok(None)
    }
}

fn repo_info_from_json(
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

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let cut: String = value.chars().take(max).collect();
    format!("{}…", cut)
}

/// Escapa cada segmento del path dejando las barras intactas.
fn encode_path(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            let mut out = String::new();
            for byte in segment.as_bytes() {
                match byte {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                        out.push(*byte as char)
                    }
                    _ => out.push_str(&format!("%{:02X}", byte)),
                }
            }
            out
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;

    for ch in input.chars() {
        let value = match ch {
            'A'..='Z' => ch as u32 - 'A' as u32,
            'a'..='z' => ch as u32 - 'a' as u32 + 26,
            '0'..='9' => ch as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            '=' => break,
            _ => continue,
        };
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push(((buffer >> bits) & 0xFF) as u8);
        }
    }

    Some(output)
}

fn base64_encode(data: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        result.push(CHARSET[(b0 >> 2) as usize] as char);
        result.push(CHARSET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARSET[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARSET[(b2 & 0x3f) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}
