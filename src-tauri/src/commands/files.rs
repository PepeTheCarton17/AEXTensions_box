use crate::state::AppState;
use std::fs;
use std::path::Path;
use tauri::State;

pub fn local_extension_dir(state: &AppState, pkg_id: &str) -> std::path::PathBuf {
    state.storage_dir.join("extensions").join(pkg_id)
}

/// Borra un directorio reintentando un par de veces: en Windows los archivos
/// de `build/` pueden seguir abiertos justo después de Gradle.
pub(crate) fn remove_dir_with_retries(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let mut last = String::new();
    for attempt in 0..3 {
        match fs::remove_dir_all(path) {
            Ok(()) if !path.exists() => return Ok(()),
            Ok(()) => last = "el directorio sigue existiendo".to_string(),
            Err(e) => last = e.to_string(),
        }
        std::thread::sleep(std::time::Duration::from_millis(200 * (attempt + 1)));
    }
    Err(last)
}

pub(crate) fn write_bundle_to_dir(
    target_dir: &Path,
    files: &std::collections::HashMap<String, Vec<u8>>,
) -> Result<(), String> {
    fs::create_dir_all(target_dir)
        .map_err(|e| format!("No se pudo crear {}: {}", target_dir.display(), e))?;
    for (rel_path, content) in files {
        let file_path = target_dir.join(rel_path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("No se pudo crear {}: {}", parent.display(), e))?;
        }
        fs::write(&file_path, content)
            .map_err(|e| format!("No se pudo escribir {}: {}", file_path.display(), e))?;
    }
    Ok(())
}

pub(crate) fn write_icons_to_dir(
    target_dir: &Path,
    module_path: &str,
    mipmaps: &std::collections::HashMap<String, Vec<u8>>,
) -> Result<(), String> {
    for (res_dir, icon_bytes) in mipmaps {
        let icon_path = target_dir
            .join(module_path)
            .join("src")
            .join("main")
            .join("res")
            .join(res_dir)
            .join("ic_launcher.png");
        if let Some(parent) = icon_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("No se pudo crear {}: {}", parent.display(), e))?;
        }
        fs::write(&icon_path, icon_bytes)
            .map_err(|e| format!("No se pudo escribir {}: {}", icon_path.display(), e))?;
    }
    Ok(())
}

/// Copia el icono también a `icon/` en la raíz del proyecto. Solo aplica al
/// modo GitHub: el índice de `gh-pages` y Nekoyomi esperan
/// `icon/ic_launcher.png` e `icon/<pkg_id>.png`. En modo local estos archivos
/// se omiten porque no hay repositorio que los publique.
pub(crate) fn write_repo_icons(
    target_dir: &Path,
    pkg_id: &str,
    mipmaps: &std::collections::HashMap<String, Vec<u8>>,
) -> Result<(), String> {
    let Some(icon_bytes) = mipmaps
        .get("mipmap-xxxhdpi")
        .or_else(|| mipmaps.get("mipmap-xxhdpi"))
    else {
        return Ok(());
    };

    let icon_dir = target_dir.join("icon");
    fs::create_dir_all(&icon_dir)
        .map_err(|e| format!("No se pudo crear {}: {}", icon_dir.display(), e))?;
    fs::write(icon_dir.join("ic_launcher.png"), icon_bytes)
        .map_err(|e| format!("No se pudo escribir el icono genérico: {}", e))?;
    fs::write(icon_dir.join(format!("{}.png", pkg_id)), icon_bytes)
        .map_err(|e| format!("No se pudo escribir el icono del paquete: {}", e))?;
    Ok(())
}

/// Prepara el bundle para una carpeta local: solo lo necesario para compilar
/// el APK. El workflow de Actions, el script del índice y los iconos de
/// `icon/` solo tienen sentido en un repositorio con `gh-pages`.
pub(crate) fn prepare_bundle_for_local(files: &mut std::collections::HashMap<String, Vec<u8>>) {
    files.retain(|path, _| {
        !(path.starts_with(".github/")
            || path.starts_with("icon/")
            || path == "local.properties")
    });
}

/// Lee `versionCode` y `versionName` del primer módulo Gradle de la extensión.
fn read_extension_versions(dir: &std::path::Path) -> (Option<i32>, Option<String>) {
    let Ok(langs) = std::fs::read_dir(dir.join("src")) else {
        return (None, None);
    };
    for lang in langs.flatten() {
        let Ok(modules) = std::fs::read_dir(lang.path()) else {
            continue;
        };
        for module in modules.flatten() {
            let gradle = module.path().join("build.gradle.kts");
            let Ok(content) = std::fs::read_to_string(&gradle) else {
                continue;
            };
            let code = regex::Regex::new(r"versionCode\s*=\s*(\d+)")
                .ok()
                .and_then(|re| re.captures(&content))
                .and_then(|caps| caps.get(1))
                .and_then(|m| m.as_str().parse::<i32>().ok());
            let name = regex::Regex::new(r#"versionName\s*=\s*"([^"]*)""#)
                .ok()
                .and_then(|re| re.captures(&content))
                .and_then(|caps| caps.get(1))
                .map(|m| m.as_str().to_string());
            if code.is_some() || name.is_some() {
                return (code, name);
            }
        }
    }
    (None, None)
}

/// Devuelve la ruta local del código de una extensión (para mostrarla aunque
/// el sistema no pueda abrir el explorador, p. ej. en Android).
#[tauri::command]
pub async fn open_extension_folder(
    state: State<'_, AppState>,
    pkg_id: String,
) -> Result<serde_json::Value, String> {
    let dir = local_extension_dir(&state, &pkg_id);
    if !dir.is_dir() {
        return Err(format!(
            "No hay copia local de esta extensión en {}",
            dir.display()
        ));
    }

    let path = dir.display().to_string();
    let opened = tauri_plugin_opener::open_path(&dir, None::<&str>).is_ok();

    Ok(serde_json::json!({ "path": path, "opened": opened }))
}

#[tauri::command]
pub async fn list_extension_backups(
    state: State<'_, AppState>,
    pkg_id: String,
) -> Result<Vec<crate::engine::backup::BackupInfo>, String> {
    let dir = local_extension_dir(&state, &pkg_id);
    Ok(crate::engine::backup::list_backups(&dir))
}

/// Restaura una copia de seguridad concreta ("versión original" o un parche
/// anterior). Es reversible: nunca borra archivos, solo los sobrescribe.
#[tauri::command]
pub async fn restore_extension_backup(
    state: State<'_, AppState>,
    pkg_id: String,
    backup: String,
) -> Result<serde_json::Value, String> {
    let dir = local_extension_dir(&state, &pkg_id);
    let count = crate::engine::backup::restore_backup(&dir, &backup)?;

    // La copia también revierte versionCode/versionName del build.gradle.kts:
    // sincronizamos la ficha de la extensión para que la UI no muestre una
    // versión que ya no existe en el código restaurado.
    let (restored_code, restored_name) = read_extension_versions(&dir);
    if restored_code.is_some() || restored_name.is_some() {
        let mut extensions = state.extensions.write().await;
        if let Some(ext) = extensions
            .iter_mut()
            .find(|e| e.pkg_id == pkg_id || e.id == pkg_id)
        {
            if let Some(code) = restored_code {
                ext.version_code = code;
            }
            if let Some(name) = &restored_name {
                ext.version = name.clone();
            }
        }
        drop(extensions);
        state.persist_extensions().await;
    }

    state
        .add_log(
            "SUCCESS",
            &format!(
                "Copia '{}' restaurada en {} ({} archivos)",
                backup, pkg_id, count
            ),
        )
        .await;
    Ok(serde_json::json!({
        "restored_files": count,
        "backup": backup,
        "version": restored_name,
        "version_code": restored_code,
    }))
}
