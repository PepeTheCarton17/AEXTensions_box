use crate::models::AiPatchRequest;
use crate::state::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use std::fs;
use std::path::Path as FsPath;

use super::files::{collect_files_with_extension, find_module_gradle};

pub(super) async fn apply_ai_patch_handler(
    State(state): State<AppState>,
    Json(patch): Json<AiPatchRequest>,
) -> Response {
    state
        .add_log(
            "INFO",
            &format!("Aplicando parche manual a la extensión: {}", patch.pkg_id),
        )
        .await;

    let mut extensions = state.extensions.write().await;
    let Some(ext) = extensions
        .iter_mut()
        .find(|e| e.pkg_id == patch.pkg_id || e.id == patch.pkg_id)
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Extensión no encontrada" })),
        )
            .into_response();
    };

    let next_code = ext.version_code + 1;
    let next_version = format!("16.{}", next_code);
    ext.version_code = next_code;
    ext.version = next_version.clone();

    let dir = crate::commands::local_extension_dir(&state, &ext.pkg_id);
    // Copia de seguridad antes de tocar nada: permite volver al estado
    // original desde "Herramientas → Versiones del código".
    let backup = if dir.is_dir() {
        crate::engine::backup::create_backup(&dir, "before-patch")
            .ok()
            .map(|info| info.name)
    } else {
        None
    };
    let persisted = apply_patch_to_files(&dir, &patch, next_code, &next_version);

    drop(extensions);
    state.persist_extensions().await;

    match &persisted {
        Ok(true) => state
            .add_log(
                "SUCCESS",
                &format!(
                    "Extensión {} actualizada con parche manual a v{} (archivos locales reescritos)",
                    patch.pkg_id, next_version
                ),
            )
            .await,
        Ok(false) => state
            .add_log(
                "WARN",
                &format!(
                    "Extensión {} actualizada a v{} solo en el catálogo: no hay copia local de los archivos para parchear",
                    patch.pkg_id, next_version
                ),
            )
            .await,
        Err(e) => state
            .add_log(
                "WARN",
                &format!("Parche aplicado en catálogo, pero falló la escritura de archivos: {}", e),
            )
            .await,
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "patched",
            "version": next_version,
            "files_patched": persisted.unwrap_or(false),
            "backup": backup
        })),
    )
        .into_response()
}

/// Reescribe `build.gradle.kts` (versión) y los selectores del Kotlin si la
/// extensión tiene copia local. Devuelve `false` si no existe esa copia.
fn apply_patch_to_files(
    dir: &FsPath,
    patch: &AiPatchRequest,
    version_code: i32,
    version_name: &str,
) -> Result<bool, String> {
    if !dir.is_dir() {
        return Ok(false);
    }

    let gradle_path = find_module_gradle(dir, &patch.pkg_id);
    if let Some(gradle_path) = gradle_path {
        let content = fs::read_to_string(&gradle_path)
            .map_err(|e| format!("No se pudo leer {}: {}", gradle_path.display(), e))?;
        let re_code = regex::Regex::new(r"versionCode\s*=\s*\d+")
            .map_err(|e| e.to_string())?;
        let re_name = regex::Regex::new(r#"versionName\s*=\s*"[^"]*""#)
            .map_err(|e| e.to_string())?;
        let content = re_code
            .replace(&content, format!("versionCode = {}", version_code).as_str())
            .to_string();
        let content = re_name
            .replace(&content, format!("versionName = \"{}\"", version_name).as_str())
            .to_string();
        fs::write(&gradle_path, content)
            .map_err(|e| format!("No se pudo escribir {}: {}", gradle_path.display(), e))?;
    }

    let mut kotlin_files = Vec::new();
    collect_files_with_extension(dir, "kt", &mut kotlin_files);
    for kt in kotlin_files {
        let Ok(content) = fs::read_to_string(&kt) else {
            continue;
        };
        let original = content.clone();
        let mut patched = content;

        if let Some(selector) = patch.custom_card_selector.as_deref() {
            if !selector.trim().is_empty() {
                let escaped = escape_kotlin_string(selector);
                for func in [
                    "popularAnimeSelector",
                    "latestUpdatesSelector",
                    "searchAnimeSelector",
                ] {
                    patched = replace_selector_function(&patched, func, &escaped);
                }
            }
        }

        if let Some(selector) = patch.custom_next_selector.as_deref() {
            if !selector.trim().is_empty() {
                let escaped = escape_kotlin_string(selector);
                for func in [
                    "popularAnimeNextPageSelector",
                    "latestUpdatesNextPageSelector",
                    "searchAnimeNextPageSelector",
                ] {
                    patched = replace_selector_function(&patched, func, &escaped);
                }
            }
        }

        if let Some(selector) = patch.custom_title_selector.as_deref() {
            if !selector.trim().is_empty() {
                let escaped = escape_kotlin_string(selector);
                if let Ok(re) = regex::Regex::new(r#"title\s*=\s*element\.selectFirst\("[^"]*"\)"#) {
                    patched = re
                        .replace(&patched, |_: &regex::Captures| {
                            format!("title = element.selectFirst(\"{}\")", escaped)
                        })
                        .to_string();
                }
            }
        }

        if let Some(code) = patch.custom_video_code.as_deref() {
            if !code.trim().is_empty() {
                let marker = "\n// ============================== AEXT MANUAL PATCH ==============================\n";
                if !patched.contains("AEXT MANUAL PATCH") {
                    patched.push_str(marker);
                    patched.push_str(code);
                    patched.push('\n');
                }
            }
        }

        if patched != original {
            fs::write(&kt, patched)
                .map_err(|e| format!("No se pudo escribir {}: {}", kt.display(), e))?;
        }
    }

    Ok(true)
}

fn replace_selector_function(content: &str, func: &str, escaped_selector: &str) -> String {
    let pattern = format!(r#"({}\(\)\s*:\s*String\s*=\s*")[^"]*(")"#, regex::escape(func));
    match regex::Regex::new(&pattern) {
        Ok(re) => re
            .replace(content, |caps: &regex::Captures| {
                format!(
                    "{}{}{}",
                    caps.get(1).map(|m| m.as_str()).unwrap_or(""),
                    escaped_selector,
                    caps.get(2).map(|m| m.as_str()).unwrap_or("")
                )
            })
            .to_string(),
        Err(_) => content.to_string(),
    }
}

fn escape_kotlin_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
