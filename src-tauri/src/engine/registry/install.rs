//! Instalación de motores desde un repositorio público.
//!
//! El repo publica `aext-engine.json` con licencia y proveniencia. AEXT guarda
//! el manifiesto y los archivos declarados bajo `<storage>/engines/<id>/`; el
//! registro persiste en `aext_engines.json`, fuera del árbol que se borra al
//! actualizar una extensión. No se usa la sesión de GitHub del usuario.

use super::manifest;
use super::model::EngineDescriptor;
use super::store;
use crate::commands::parsers::parse_github_url;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct RepoRef {
    pub owner: String,
    pub repo: String,
    pub branch: String,
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("AEXT-EngineInstaller/1.0")
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| format!("No se pudo crear el cliente HTTP: {}", e))
}

fn raw_url(repo: &RepoRef, path: &str) -> String {
    format!(
        "https://raw.githubusercontent.com/{}/{}/{}/{}",
        repo.owner,
        repo.repo,
        repo.branch,
        path.trim_start_matches('/')
    )
}

async fn fetch_text(http: &reqwest::Client, url: &str) -> Result<String, String> {
    let response = http
        .get(url)
        .send()
        .await
        .map_err(|e| format!("No se pudo descargar {}: {}", url, e))?;
    if !response.status().is_success() {
        return Err(format!("{} devolvió HTTP {}", url, response.status().as_str()));
    }
    response
        .text()
        .await
        .map_err(|e| format!("No se pudo leer {}: {}", url, e))
}

/// Descarga y valida el manifiesto. Prueba `main` y `master`; no acepta una
/// proveniencia que no se pueda verificar.
pub async fn fetch(repo_url: &str) -> Result<(EngineDescriptor, RepoRef), String> {
    let (owner, repo) = parse_github_url(repo_url)
        .ok_or_else(|| "Indica un repositorio de GitHub válido (owner/repo o su URL)".to_string())?;
    let http = client()?;
    let mut last_error = String::new();
    for branch in ["main", "master"] {
        let reference = RepoRef {
            owner: owner.clone(),
            repo: repo.clone(),
            branch: branch.to_string(),
        };
        match fetch_text(&http, &raw_url(&reference, "aext-engine.json")).await {
            Ok(raw) => {
                let mut descriptor = manifest::parse(&raw)?;
                let provenance = format!("https://github.com/{}/{}", owner, repo);
                if descriptor.provenance != provenance && !descriptor.provenance.starts_with("http") {
                    return Err("provenance del manifiesto no verificable".to_string());
                }
                descriptor.provenance = provenance;
                return Ok((descriptor, reference));
            }
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "No se encontró aext-engine.json en el repo (main/master): {}",
        last_error
    ))
}

fn safe_relative(file: &str) -> Result<PathBuf, String> {
    let path = Path::new(file);
    if file.starts_with('/') || path.components().any(|c| c.as_os_str() == "..") {
        return Err(format!("ruta de archivo no segura: {}", file));
    }
    Ok(path.to_path_buf())
}

/// Copia los archivos declarados y el propio manifiesto. Si falla uno, no se
/// registra el motor: evita dejar una instalación a medias.
pub async fn save_files(
    storage_dir: &Path,
    engine: &EngineDescriptor,
    repo: &RepoRef,
) -> Result<PathBuf, String> {
    let http = client()?;
    let dir = store::install_dir(storage_dir, &engine.id);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("No se pudo crear {}: {}", dir.display(), e))?;

    let manifest_json = serde_json::to_string_pretty(engine)
        .map_err(|e| format!("No se pudo serializar el manifiesto: {}", e))?;
    fs::write(dir.join("aext-engine.json"), manifest_json)
        .map_err(|e| format!("No se pudo guardar el manifiesto: {}", e))?;

    for file in &engine.files {
        let relative = safe_relative(file)?;
        let content = fetch_text(&http, &raw_url(repo, file)).await?;
        let target = dir.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("No se pudo crear {}: {}", parent.display(), e))?;
        }
        fs::write(&target, content)
            .map_err(|e| format!("No se pudo guardar {}: {}", target.display(), e))?;
    }
    Ok(dir)
}
