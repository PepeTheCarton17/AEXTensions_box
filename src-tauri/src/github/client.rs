use serde::{Deserialize, Serialize};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT, ACCEPT};

pub struct GithubApiClient {
    client: reqwest::Client,
    #[allow(dead_code)]
    token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateRepoParams {
    pub name: String,
    pub description: Option<String>,
    pub private: bool,
    pub auto_init: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub id: u64,
    pub status: String,
    pub conclusion: Option<String>,
    pub html_url: String,
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

        Self { client, token: token.clone() }
    }

    pub async fn ensure_repository(&self, owner: &str, repo_name: &str, private: bool) -> Result<String, String> {
        let check_url = format!("https://api.github.com/repos/{}/{}", owner, repo_name);
        let resp = self.client.get(&check_url).send().await
            .map_err(|e| format!("Error verificando repo en GitHub: {}", e))?;

        if resp.status().is_success() {
            let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            return Ok(data.get("html_url").and_then(|v| v.as_str()).unwrap_or("").to_string());
        }

        // Si no existe (404), lo creamos
        let create_url = "https://api.github.com/user/repos";
        let params = CreateRepoParams {
            name: repo_name.to_string(),
            description: Some("Repositorio de Extensiones Nekoyomi generado automáticamente por AEXT".to_string()),
            private,
            auto_init: true,
        };

        let create_resp = self.client.post(create_url)
            .json(&params)
            .send()
            .await
            .map_err(|e| format!("Error creando repositorio en GitHub: {}", e))?;

        if !create_resp.status().is_success() {
            let err_body = create_resp.text().await.unwrap_or_default();
            return Err(format!("Error al crear repositorio {}: {}", repo_name, err_body));
        }

        let created_data: serde_json::Value = create_resp.json().await.map_err(|e| e.to_string())?;
        Ok(created_data.get("html_url").and_then(|v| v.as_str()).unwrap_or("").to_string())
    }

    pub async fn put_file_content(&self, owner: &str, repo: &str, path: &str, content: &[u8], commit_message: &str) -> Result<(), String> {
        let url = format!("https://api.github.com/repos/{}/{}/contents/{}", owner, repo, path);

        // Verificamos si el archivo ya existe para obtener su SHA (requerido por GitHub API para sobreescribir)
        let mut sha: Option<String> = None;
        if let Ok(res) = self.client.get(&url).send().await {
            if res.status().is_success() {
                if let Ok(json) = res.json::<serde_json::Value>().await {
                    if let Some(s) = json.get("sha").and_then(|v| v.as_str()) {
                        sha = Some(s.to_string());
                    }
                }
            }
        }

        // Convertir contenido a base64
        let base64_content = base64_encode(content);

        let mut body = serde_json::json!({
            "message": commit_message,
            "content": base64_content,
            "branch": "main"
        });

        if let Some(s) = sha {
            body["sha"] = serde_json::Value::String(s);
        }

        let resp = self.client.put(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Error al subir archivo {}: {}", path, e))?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            return Err(format!("Fallo subiendo {} a GitHub: {}", path, err));
        }

        Ok(())
    }

    pub async fn get_latest_workflow_run(&self, owner: &str, repo: &str) -> Result<Option<WorkflowRun>, String> {
        let url = format!("https://api.github.com/repos/{}/{}/actions/runs?per_page=1", owner, repo);
        let resp = self.client.get(&url).send().await
            .map_err(|e| format!("Error consultando actions: {}", e))?;

        if !resp.status().is_success() {
            return Ok(None);
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        if let Some(runs) = json.get("workflow_runs").and_then(|v| v.as_array()) {
            if let Some(first) = runs.first() {
                let id = first.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                let status = first.get("status").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                let conclusion = first.get("conclusion").and_then(|v| v.as_str()).map(|s| s.to_string());
                let html_url = first.get("html_url").and_then(|v| v.as_str()).unwrap_or("").to_string();

                return Ok(Some(WorkflowRun { id, status, conclusion, html_url }));
            }
        }

        Ok(None)
    }
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
