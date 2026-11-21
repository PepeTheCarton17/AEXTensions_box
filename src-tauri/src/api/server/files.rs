use std::fs;
use std::path::{Path as FsPath, PathBuf};

/// Localiza el `build.gradle.kts` del módulo de la extensión. La ruta se
/// deriva del `pkg_id` (`src/<lang>/<slug>`); si no existe, se busca.
pub(super) fn find_module_gradle(dir: &FsPath, pkg_id: &str) -> Option<PathBuf> {
    let parts: Vec<&str> = pkg_id.split('.').filter(|p| !p.is_empty()).collect();
    if parts.len() >= 2 {
        let lang = parts[parts.len() - 2];
        let slug = parts[parts.len() - 1];
        let direct = dir
            .join("src")
            .join(lang)
            .join(slug)
            .join("build.gradle.kts");
        if direct.exists() {
            return Some(direct);
        }
    }

    let mut candidates = Vec::new();
    collect_gradle_files(dir, &mut candidates);
    candidates.sort_by_key(|path| path.components().count());
    candidates.into_iter().find(|path| {
        fs::read_to_string(path)
            .map(|content| content.contains("applicationId"))
            .unwrap_or(false)
    })
}

fn collect_gradle_files(dir: &FsPath, out: &mut Vec<PathBuf>) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_gradle_files(&path, out);
        } else if path.file_name().and_then(|name| name.to_str()) == Some("build.gradle.kts") {
            out.push(path);
        }
    }
}

pub(super) fn collect_files_with_extension(dir: &FsPath, extension: &str, out: &mut Vec<PathBuf>) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files_with_extension(&path, extension, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case(extension))
            .unwrap_or(false)
        {
            out.push(path);
        }
    }
}
