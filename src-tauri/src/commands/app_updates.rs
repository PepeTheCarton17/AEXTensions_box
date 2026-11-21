
#[tauri::command]
pub async fn check_aext_updates() -> Result<serde_json::Value, String> {
    let current_version = env!("CARGO_PKG_VERSION");
    let client = reqwest::Client::builder()
        .user_agent("AEXT-Updater/1.0")
        .build()
        .map_err(|e| e.to_string())?;

    let url = "https://api.github.com/repos/PepeTheCarton17/AEXTensions_box/releases/latest";
    match client.get(url).send().await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                let tag_name = json
                    .get("tag_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let clean_tag = tag_name.trim_start_matches('v').trim();
                let release_notes = json
                    .get("body")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let release_url = json
                    .get("html_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("https://github.com/PepeTheCarton17/AEXTensions_box/releases")
                    .to_string();

                let latest_version = if clean_tag.is_empty() {
                    current_version.to_string()
                } else {
                    clean_tag.to_string()
                };
                let update_available = version_is_newer(&latest_version, current_version);

                let notes = if update_available {
                    release_notes
                } else {
                    "Estás ejecutando la versión más reciente.".to_string()
                };

                let assets: Vec<serde_json::Value> = json
                    .get("assets")
                    .and_then(|v| v.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .map(|asset| {
                                serde_json::json!({
                                    "name": asset.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                                    "url": asset.get("browser_download_url").and_then(|v| v.as_str()).unwrap_or(""),
                                    "size": asset.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
                                    "content_type": asset.get("content_type").and_then(|v| v.as_str()).unwrap_or("")
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                return Ok(serde_json::json!({
                    "current_version": current_version,
                    "latest_version": latest_version,
                    "tag_name": if tag_name.is_empty() { format!("v{}", current_version) } else { tag_name.to_string() },
                    "update_available": update_available,
                    "release_notes": notes,
                    "release_url": release_url,
                    "assets": assets
                }));
            }
        }
        Ok(resp) => {
            return Ok(serde_json::json!({
                "current_version": current_version,
                "latest_version": current_version,
                "tag_name": format!("v{}", current_version),
                "update_available": false,
                "release_notes": format!("GitHub respondió HTTP {}. No se pudo comprobar la última release.", resp.status()),
                "release_url": "https://github.com/PepeTheCarton17/AEXTensions_box/releases",
                "error": true
            }));
        }
        Err(e) => {
            return Ok(serde_json::json!({
                "current_version": current_version,
                "latest_version": current_version,
                "tag_name": format!("v{}", current_version),
                "update_available": false,
                "release_notes": format!("No se pudo conectar con GitHub: {}", e),
                "release_url": "https://github.com/PepeTheCarton17/AEXTensions_box/releases",
                "error": true
            }));
        }
    }

    Ok(serde_json::json!({
        "current_version": current_version,
        "latest_version": current_version,
        "tag_name": format!("v{}", current_version),
        "update_available": false,
        "release_notes": "No se pudo leer la respuesta de GitHub.",
        "release_url": "https://github.com/PepeTheCarton17/AEXTensions_box/releases",
        "error": true
    }))
}

fn version_is_newer(latest: &str, current: &str) -> bool {
    let latest_parts = parse_version(latest);
    let current_parts = parse_version(current);
    let len = latest_parts.len().max(current_parts.len());
    for i in 0..len {
        let l = *latest_parts.get(i).unwrap_or(&0);
        let c = *current_parts.get(i).unwrap_or(&0);
        if l > c {
            return true;
        }
        if l < c {
            return false;
        }
    }
    false
}

fn parse_version(value: &str) -> Vec<u64> {
    value
        .trim()
        .trim_start_matches('v')
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::version_is_newer;

    #[test]
    fn compares_versions() {
        assert!(version_is_newer("1.2.0", "1.1.9"));
        assert!(!version_is_newer("1.0.0", "1.0.0"));
        assert!(!version_is_newer("1.0.0", "1.0.1"));
    }
}
