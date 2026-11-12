use crate::models::GithubAccountInfo;
use std::fs;
use std::path::PathBuf;

pub struct GithubAuthManager {
    config_path: PathBuf,
}

impl GithubAuthManager {
    pub fn new() -> Self {
        let mut path = dirs_next().unwrap_or_else(|| PathBuf::from("."));
        path.push("aext_github_auth.json");
        Self { config_path: path }
    }

    pub fn get_saved_token(&self) -> Option<String> {
        if self.config_path.exists() {
            if let Ok(content) = fs::read_to_string(&self.config_path) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(t) = val.get("token").and_then(|v| v.as_str()) {
                        if !t.is_empty() {
                            return Some(t.to_string());
                        }
                    }
                }
            }
        }
        None
    }

    pub fn save_token(&self, token: &str) -> Result<(), String> {
        let val = serde_json::json!({
            "token": token,
            "saved_at": chrono::Utc::now().to_rfc3339()
        });
        fs::write(&self.config_path, val.to_string())
            .map_err(|e| format!("Error guardando token: {}", e))
    }

    pub fn clear_token(&self) -> Result<(), String> {
        if self.config_path.exists() {
            let _ = fs::remove_file(&self.config_path);
        }
        Ok(())
    }

    pub async fn verify_token(&self, token: &str) -> Result<GithubAccountInfo, String> {
        let client = reqwest::Client::builder()
            .user_agent("AEXT-Extension-Builder/1.0")
            .build()
            .map_err(|e| e.to_string())?;

        let res = client
            .get("https://api.github.com/user")
            .header("Authorization", format!("Bearer {}", token.trim()))
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .map_err(|e| format!("Fallo de conexión con GitHub: {}", e))?;

        if !res.status().is_success() {
            return Err(format!("Token de GitHub inválido o expirado (HTTP {})", res.status()));
        }

        let scopes = res.headers()
            .get("x-oauth-scopes")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
            .unwrap_or_default();

        let json: serde_json::Value = res.json().await
            .map_err(|e| format!("Error procesando respuesta de GitHub: {}", e))?;

        let username = json.get("login").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let display_name = json.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
        let avatar_url = json.get("avatar_url").and_then(|v| v.as_str()).unwrap_or("").to_string();

        Ok(GithubAccountInfo {
            username,
            display_name,
            avatar_url,
            auth_type: "pat".to_string(),
            scopes,
        })
    }

    #[cfg(target_os = "windows")]
    pub async fn detect_cli_session(&self) -> Result<GithubAccountInfo, String> {
        // En Windows intentar leer token de GitHub CLI
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        let cli_hosts = PathBuf::from(&appdata).join("GitHub CLI").join("hosts.yml");
        if cli_hosts.exists() {
            if let Ok(content) = fs::read_to_string(&cli_hosts) {
                // Buscamos oauth_token en github.com
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("oauth_token:") {
                        let token = trimmed.trim_start_matches("oauth_token:").trim();
                        if !token.is_empty() {
                            let mut info = self.verify_token(token).await?;
                            info.auth_type = "cli".to_string();
                            return Ok(info);
                        }
                    }
                }
            }
        }

        // Intento con comando gh auth token si está en PATH
        if let Ok(output) = std::process::Command::new("gh").arg("auth").arg("token").output() {
            if output.status.success() {
                let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !token.is_empty() {
                    let mut info = self.verify_token(&token).await?;
                    info.auth_type = "cli".to_string();
                    return Ok(info);
                }
            }
        }

        Err("No se detectó sesión activa de GitHub CLI".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    pub async fn detect_cli_session(&self) -> Result<GithubAccountInfo, String> {
        // En Linux/macOS intentar ~/.config/gh/hosts.yml o gh auth token
        let home = std::env::var("HOME").unwrap_or_default();
        let cli_hosts = PathBuf::from(&home).join(".config").join("gh").join("hosts.yml");
        if cli_hosts.exists() {
            if let Ok(content) = fs::read_to_string(&cli_hosts) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("oauth_token:") {
                        let token = trimmed.trim_start_matches("oauth_token:").trim();
                        if !token.is_empty() {
                            let mut info = self.verify_token(token).await?;
                            info.auth_type = "cli".to_string();
                            return Ok(info);
                        }
                    }
                }
            }
        }

        if let Ok(output) = std::process::Command::new("gh").arg("auth").arg("token").output() {
            if output.status.success() {
                let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !token.is_empty() {
                    let mut info = self.verify_token(&token).await?;
                    info.auth_type = "cli".to_string();
                    return Ok(info);
                }
            }
        }

        Err("No se detectó sesión de GitHub CLI".to_string())
    }
}

fn dirs_next() -> Option<PathBuf> {
    if let Ok(appdata) = std::env::var("APPDATA") {
        let p = PathBuf::from(appdata).join("AEXT");
        let _ = fs::create_dir_all(&p);
        return Some(p);
    }
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home).join(".aext");
        let _ = fs::create_dir_all(&p);
        return Some(p);
    }
    std::env::current_dir().ok()
}
