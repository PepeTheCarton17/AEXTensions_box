//! Persistencia del registro de motores.
//!
//! Vive junto a `aext_extensions.json`, como hermano de `extensions/`. La
//! preparación de actualizaciones sólo borra `<storage>/extensions/<pkg_id>`,
//! así que los motores instalados sobreviven a cualquier regeneración.

use super::builtin::builtin_engines;
use super::model::EngineDescriptor;
use std::fs;
use std::path::{Path, PathBuf};

pub const ENGINES_FILE: &str = "aext_engines.json";
pub const ENGINES_DIR: &str = "engines";

pub fn engines_path(dir: &Path) -> PathBuf {
    dir.join(ENGINES_FILE)
}

pub fn install_dir(dir: &Path, id: &str) -> PathBuf {
    dir.join(ENGINES_DIR).join(id)
}

pub fn load(dir: &Path) -> Vec<EngineDescriptor> {
    let mut installed = Vec::new();
    if let Ok(raw) = fs::read_to_string(engines_path(dir)) {
        if let Ok(list) = serde_json::from_str::<Vec<EngineDescriptor>>(&raw) {
            installed = list;
        }
    }
    for builtin in builtin_engines() {
        if !installed.iter().any(|e| e.id == builtin.id) {
            installed.push(builtin);
        }
    }
    installed
}

pub fn persist(dir: &Path, engines: &[EngineDescriptor]) {
    if let Ok(json) = serde_json::to_string_pretty(engines) {
        let _ = fs::write(engines_path(dir), json);
    }
}

pub fn upsert(engines: &mut Vec<EngineDescriptor>, descriptor: EngineDescriptor) {
    if let Some(current) = engines.iter_mut().find(|e| e.id == descriptor.id) {
        *current = descriptor;
    } else {
        engines.push(descriptor);
    }
}

/// Los motores incluidos no se pueden desinstalar: sólo se desactivan.
pub fn disable(engines: &mut [EngineDescriptor], id: &str, disabled: bool) -> bool {
    if let Some(engine) = engines.iter_mut().find(|e| e.id == id) {
        engine.enabled = !disabled;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_present_and_repo_engines_survive_a_reload() {
        let mut list = load(Path::new("C:/definitely-missing-aext-dir"));
        assert_eq!(list.len(), 2);
        let mut extra = EngineDescriptor::default();
        extra.id = "repo-engine".to_string();
        upsert(&mut list, extra);
        assert!(list.iter().any(|e| e.id == "repo-engine"));
        assert!(list.iter().any(|e| e.id == "builtin-public-api"));
    }
}
