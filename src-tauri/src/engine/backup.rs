//! Copias de seguridad locales de las extensiones generadas.
//!
//! Antes de aplicar un parche (manual o automático) se guarda una copia del
//! árbol de fuentes en `<extensión>/.aext-backups/<fecha>/`. Así el usuario
//! puede volver al estado "original" sin depender de Git ni de GitHub.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const BACKUP_DIR: &str = ".aext-backups";
const MANIFEST: &str = "manifest.json";

/// Directorios que nunca se copian: son regenerables y pueden ser enormes.
const SKIPPED_DIRS: &[&str] = &[BACKUP_DIR, "build", ".gradle", ".git", ".idea"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    pub name: String,
    pub created_at: String,
    pub reason: String,
    pub files: usize,
    pub bytes: u64,
}

fn backups_root(dir: &Path) -> PathBuf {
    dir.join(BACKUP_DIR)
}

/// Normaliza una fecha local como nombre de carpeta ordenable.
fn timestamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 96
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Crea una copia de seguridad del árbol de la extensión y devuelve su ficha.
pub fn create_backup(dir: &Path, reason: &str) -> Result<BackupInfo, String> {
    if !dir.is_dir() {
        return Err(format!(
            "No existe la carpeta local de la extensión: {}",
            dir.display()
        ));
    }

    let root = backups_root(dir);
    fs::create_dir_all(&root)
        .map_err(|e| format!("No se pudo crear {}: {}", root.display(), e))?;

    let stamp = timestamp();
    let mut name = stamp.clone();
    let mut attempt = 1;
    while root.join(&name).exists() {
        name = format!("{}-{}", stamp, attempt);
        attempt += 1;
    }

    let target = root.join(&name);
    fs::create_dir_all(&target)
        .map_err(|e| format!("No se pudo crear {}: {}", target.display(), e))?;

    let mut files = 0usize;
    let mut bytes = 0u64;
    copy_tree(dir, dir, &target, &mut files, &mut bytes)?;

    let info = BackupInfo {
        name,
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        reason: reason.to_string(),
        files,
        bytes,
    };

    let json = serde_json::to_string_pretty(&info)
        .map_err(|e| format!("No se pudo serializar el manifiesto: {}", e))?;
    fs::write(target.join(MANIFEST), json)
        .map_err(|e| format!("No se pudo escribir el manifiesto: {}", e))?;

    Ok(info)
}

fn copy_tree(
    root: &Path,
    current: &Path,
    target_root: &Path,
    files: &mut usize,
    bytes: &mut u64,
) -> Result<(), String> {
    let entries = fs::read_dir(current)
        .map_err(|e| format!("No se pudo leer {}: {}", current.display(), e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if entry.file_type().map(|t| t.is_symlink()).unwrap_or(false) {
            continue;
        }

        if path.is_dir() {
            if SKIPPED_DIRS.contains(&name.as_str()) {
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_path_buf();
            let destination = target_root.join(&relative);
            fs::create_dir_all(&destination)
                .map_err(|e| format!("No se pudo crear {}: {}", destination.display(), e))?;
            copy_tree(root, &path, target_root, files, bytes)?;
            continue;
        }

        if !path.is_file() {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_path_buf();
        let destination = target_root.join(&relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("No se pudo crear {}: {}", parent.display(), e))?;
        }
        let size = fs::copy(&path, &destination)
            .map_err(|e| format!("No se pudo copiar {}: {}", path.display(), e))?;
        *files += 1;
        *bytes += size;
    }

    Ok(())
}

/// Lista las copias disponibles, de la más reciente a la más antigua.
pub fn list_backups(dir: &Path) -> Vec<BackupInfo> {
    let root = backups_root(dir);
    let Ok(entries) = fs::read_dir(&root) else {
        return Vec::new();
    };

    let mut items: Vec<BackupInfo> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if !is_valid_name(&name) {
                return None;
            }
            if let Ok(content) = fs::read_to_string(entry.path().join(MANIFEST)) {
                if let Ok(info) = serde_json::from_str::<BackupInfo>(&content) {
                    return Some(info);
                }
            }
            Some(BackupInfo {
                name,
                created_at: String::new(),
                reason: String::new(),
                files: 0,
                bytes: 0,
            })
        })
        .collect();

    items.sort_by(|a, b| b.name.cmp(&a.name));
    items
}

/// Restaura una copia sobre el árbol actual (sobrescribe, no borra sobrantes).
/// Devuelve cuántos archivos se restauraron.
pub fn restore_backup(dir: &Path, name: &str) -> Result<usize, String> {
    if !is_valid_name(name) {
        return Err("Nombre de copia inválido".to_string());
    }

    let source = backups_root(dir).join(name);
    if !source.is_dir() {
        return Err(format!("No existe la copia de seguridad '{}'", name));
    }

    let mut restored = 0usize;
    restore_tree(&source, &source, dir, &mut restored)?;
    Ok(restored)
}

fn restore_tree(
    source_root: &Path,
    current: &Path,
    destination_root: &Path,
    restored: &mut usize,
) -> Result<(), String> {
    let entries = fs::read_dir(current)
        .map_err(|e| format!("No se pudo leer {}: {}", current.display(), e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            if SKIPPED_DIRS.contains(&name.as_str()) {
                continue;
            }
            restore_tree(source_root, &path, destination_root, restored)?;
            continue;
        }

        // El manifiesto pertenece a la copia, no al proyecto.
        if name == MANIFEST && current == source_root {
            continue;
        }

        let relative = path
            .strip_prefix(source_root)
            .map_err(|e| e.to_string())?
            .to_path_buf();
        let destination = destination_root.join(&relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("No se pudo crear {}: {}", parent.display(), e))?;
        }
        fs::copy(&path, &destination)
            .map_err(|e| format!("No se pudo restaurar {}: {}", destination.display(), e))?;
        *restored += 1;
    }

    Ok(())
}
