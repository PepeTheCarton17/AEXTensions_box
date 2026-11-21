use super::models::{GithubApiClient, WorkflowRun};
use super::request::truncate;

impl GithubApiClient {
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
