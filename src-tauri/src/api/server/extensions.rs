use crate::state::AppState;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Json, Response},
};
use std::fs;
use std::path::Path as FsPath;

/// Entrega un ZIP real con el código fuente de la extensión (y su
/// `extension.json`). En modo local no existe un APK compilado en el
/// dispositivo, así que este es el artefacto honesto que podemos ofrecer.
pub(super) async fn download_extension_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let items = state.extensions.read().await;
    let item = items
        .iter()
        .find(|e| e.id == id || e.pkg_id == id)
        .cloned();
    drop(items);

    let Some(ext) = item else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let dir = crate::commands::local_extension_dir(&state, &ext.pkg_id);
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    if dir.is_dir() {
        collect_dir_entries(&dir, &dir, &mut entries);
    }

    // Metadatos legibles por humanos / por Nekoyomi.
    match serde_json::to_vec_pretty(&ext) {
        Ok(bytes) => entries.push(("extension.json".to_string(), bytes)),
        Err(e) => {
            state
                .add_log("WARN", &format!("No se pudo serializar extension.json: {}", e))
                .await;
        }
    }

    if entries.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "No hay archivos locales para esta extensión",
                "pkg_id": ext.pkg_id,
            })),
        )
            .into_response();
    }

    let zip = crate::engine::create_zip(&entries);
    let filename = format!("{}.zip", ext.pkg_id);
    let disposition = format!("attachment; filename=\"{}\"", filename);

    let mut response = Response::new(Body::from(zip));
    *response.status_mut() = StatusCode::OK;
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("application/zip"));
    if let Ok(value) = HeaderValue::from_str(&disposition) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    response
}

/// Directorios que no deben entrar en el ZIP de código: son regenerables
/// (`build`), locales (`.aext-backups`) o temporales (`__pycache__`).
const SKIPPED_ZIP_DIRS: &[&str] = &[
    ".aext-backups",
    ".gradle",
    ".git",
    ".idea",
    "build",
    "__pycache__",
];

fn collect_dir_entries(root: &FsPath, current: &FsPath, out: &mut Vec<(String, Vec<u8>)>) {
    let Ok(read_dir) = fs::read_dir(current) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let dir_name = entry.file_name().to_string_lossy().to_string();
            if SKIPPED_ZIP_DIRS.iter().any(|skip| skip.eq_ignore_ascii_case(&dir_name)) {
                continue;
            }
            collect_dir_entries(root, &path, out);
        } else if path.is_file() {
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            let name = relative.to_string_lossy().replace('\\', "/");
            if let Ok(bytes) = fs::read(&path) {
                out.push((name, bytes));
            }
        }
    }
}
